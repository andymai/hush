//! How Hush types on the session's keyboard: with keys from the active XKB
//! layout, with the built-in US table, or not at all, so every take pastes.
//!
//! The rules:
//! - With one layout, or when the compositor names the active one, Hush
//!   compiles the keymap and types through it.
//! - With several layouts and no word on the active one, Hush pastes,
//!   because a wrong guess types the wrong letters.
//! - When the keyboard can have either of two option lists, as on X11, Hush
//!   types only the characters whose keys are the same with both.
//! - When libxkbcommon or the keymap is not available, Hush types with the
//!   US table on plain US and pastes on every other layout.
//! - With Caps Lock on, Hush finds the keys with the lock held, so each
//!   character comes out as written. When Hush cannot tell what the lock
//!   holds, it types only the characters whose keys agree with each
//!   possible lock. When it cannot read the lock at all, it types as if the
//!   lock is off.

use super::keymap::{ChordKeys, KeyTable};
use super::layout::{self, plain_us, Active, LockSource, Session, FIX_SEVERAL};
use super::xkb::{self, Locked, Names, Request};
use input_linux::Key;
use std::fmt;
use std::sync::Arc;
use tracing::debug;

/// XKB keymaps hold at most four layouts.
const MAX_GROUPS: usize = 4;
/// Tables to keep, so that Caps Lock on and off do not compile again.
const CACHED_TABLES: usize = 2;

/// Reads the Caps Lock LED of Hush's own keyboard. `None` when the LED
/// cannot be read.
pub trait CapsLed {
    fn caps_lock(&mut self) -> Option<bool>;
}

/// For a process without Hush's keyboard, such as `hush doctor`.
pub struct NoLeds;

impl CapsLed for NoLeds {
    fn caps_lock(&mut self) -> Option<bool> {
        None
    }
}

/// Whether Caps Lock is on for the keys Hush types, or `None` when Hush
/// cannot tell.
fn caps_on(session: Option<&Session>, leds: &mut dyn CapsLed) -> Option<bool> {
    match session.map_or(LockSource::Leds, |session| session.locks) {
        LockSource::Leds => leds.caps_lock(),
        LockSource::Reported(on) => Some(on),
        LockSource::Unknown => None,
    }
}

/// Whether the Caps Lock LED can show Shift Lock: with `caps:shiftlock`, or
/// with options that Hush does not know. For a take without a key table.
fn shift_lock_possible(session: Option<&Session>) -> bool {
    session
        .and_then(|session| session.options.as_deref())
        .is_none_or(|options| {
            options
                .split(',')
                .any(|option| option.trim() == "caps:shiftlock")
        })
}

/// The locked modifiers to build the tables with, one table for each.
/// With known XKB options, the keymap's Caps Lock indicator says what the
/// LED shows. With unknown options (Sway), the LED can show Lock, Shift
/// with `caps:shiftlock`, or a layout other than the first with
/// `grp_led:caps`, so Hush keeps only the keys that agree with each.
fn locks_for(session: Option<&Session>, group: u32, on: Option<bool>) -> Vec<Locked> {
    let options_known = session.is_some_and(|session| session.options.is_some());
    match on {
        Some(true) if options_known => vec![Locked::CapsIndicator],
        Some(true) => {
            let mut locks = vec![Locked::Named(xkb::LOCK_MOD), Locked::Named(xkb::SHIFT_MOD)];
            if group != 0 {
                locks.push(Locked::None);
            }
            locks
        },
        Some(false) => vec![Locked::None],
        None => {
            debug!("Caps Lock state unknown; typing as if it is off");
            vec![Locked::None]
        },
    }
}

/// How the session's text goes in.
#[derive(Debug, Clone)]
pub enum Typing {
    /// Keys from the active layout's XKB keymap.
    Keys {
        table: Arc<KeyTable>,
        layout: String,
    },
    /// The built-in US table, because libxkbcommon or the keymap is not
    /// available and the active layout is plain US.
    UsTable { layout: String, why: String },
    /// No key table fits, so every take pastes.
    Paste { why: String, fix: &'static str },
}

impl Typing {
    pub fn table(&self) -> Option<&KeyTable> {
        match self {
            Self::Keys { table, .. } => Some(table),
            Self::UsTable { .. } => Some(KeyTable::us()),
            Self::Paste { .. } => None,
        }
    }
}

impl fmt::Display for Typing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Keys { table, layout } => match table.dead_key_chars() {
                0 => write!(f, "types key by key on {}", layout),
                dead => write!(
                    f,
                    "types key by key on {}, {} characters through dead keys",
                    layout, dead
                ),
            },
            Self::UsTable { layout, why } => {
                write!(f, "types with the US key table on {} ({})", layout, why)
            },
            Self::Paste { why, .. } => write!(f, "pastes every take: {}", why),
        }
    }
}

/// What a session calls for, before any keymap compiles.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Plan {
    Compile {
        names: Names,
        /// A second option list the keyboard can have instead.
        other_options: Option<String>,
        group: u32,
        level3: bool,
        layout: String,
        plain_us: bool,
    },
    UsTable {
        layout: String,
        why: String,
    },
    Paste {
        why: String,
        fix: &'static str,
    },
}

const FIX_NAMES: &str = "to type, set the layout by its XKB name";
const FIX_CAPS: &str = "to type with Caps Lock on, install libxkbcommon (libxkbcommon0 on Debian \
     and Ubuntu); with Caps Lock off, Hush types";
const WHY_CAPS: &str = "Caps Lock is on, and the built-in US key table types every letter in the \
     wrong case with it";
const FIX_LIBRARY: &str =
    "install libxkbcommon (libxkbcommon0 on Debian and Ubuntu) to type on this layout";

fn plan(session: Option<&Session>, library: bool) -> Plan {
    let Some(session) = session else {
        let layout = "us (assumed, no source names the layout)".to_string();
        return if library {
            Plan::Compile {
                names: Names {
                    layout: "us".into(),
                    ..Names::default()
                },
                other_options: None,
                group: 0,
                level3: false,
                layout,
                plain_us: true,
            }
        } else {
            Plan::UsTable {
                layout,
                why: "libxkbcommon is not installed".into(),
            }
        };
    };
    let Some(entries) = session
        .layout
        .entries()
        .filter(|entries| !entries.is_empty())
    else {
        return Plan::Paste {
            why: format!(
                "Hush cannot map the layout '{}' to XKB names",
                session.layout
            ),
            fix: FIX_NAMES,
        };
    };
    let index = match &session.active {
        Active::Index(index) if *index < entries.len() => *index,
        Active::Index(index) => {
            return Plan::Paste {
                why: format!(
                    "the compositor names layout {} of {}",
                    index + 1,
                    entries.len()
                ),
                fix: FIX_SEVERAL,
            };
        },
        Active::Unknown if entries.len() == 1 => 0,
        Active::Unknown => {
            return Plan::Paste {
                why: format!(
                    "the session has {} layouts ({}), and Hush cannot tell which one is active",
                    entries.len(),
                    session.layout
                ),
                fix: FIX_SEVERAL,
            };
        },
        Active::Unclear { why, fix } => {
            return Plan::Paste {
                why: why.clone(),
                fix,
            };
        },
    };
    let (layout, variant) = &entries[index];
    let shown = match (layout.as_str(), variant.as_str()) {
        ("", _) => "the default layout".to_string(),
        (layout, "") => layout.to_string(),
        (layout, variant) => format!("{}({})", layout, variant),
    };
    let plain_us = plain_us(layout, variant);
    if !library {
        return if plain_us {
            Plan::UsTable {
                layout: shown,
                why: "libxkbcommon is not installed".into(),
            }
        } else {
            Plan::Paste {
                why: format!(
                    "libxkbcommon is not installed, and {} is not plain US",
                    shown
                ),
                fix: FIX_LIBRARY,
            }
        };
    }
    let (chosen, group) = if entries.len() > MAX_GROUPS {
        (&entries[index..=index], 0)
    } else {
        (&entries[..], index)
    };
    let list =
        |pick: fn(&(String, String)) -> &str| chosen.iter().map(pick).collect::<Vec<_>>().join(",");
    Plan::Compile {
        names: Names {
            rules: session.rules.clone(),
            model: session.model.clone(),
            layout: list(|(layout, _)| layout),
            variant: list(|(_, variant)| variant),
            options: session.options.clone().unwrap_or_default(),
        },
        other_options: session.other_options.clone(),
        group: group as u32,
        level3: session.options.is_some(),
        layout: match session.other_options {
            Some(_) => format!("{} (only keys that two option lists agree on)", shown),
            None => shown,
        },
        plain_us,
    }
}

/// The XKB names that decide which keys hold Control and Shift, or why
/// Hush cannot know them. The XKB options move these modifiers, and the
/// layouts do not, so a session whose active layout is unknown, or whose
/// layout is an input method, still gives names.
fn modifier_names(session: Option<&Session>) -> Result<Vec<Names>, String> {
    let session = session.ok_or("no source names the keyboard layout and its XKB options")?;
    let options = session.options.clone().ok_or(
        "the compositor does not report its XKB options (Sway does not), and an option can \
         move Ctrl",
    )?;
    let entries = session
        .layout
        .entries()
        .filter(|entries| !entries.is_empty())
        .unwrap_or_else(|| vec![("us".to_string(), String::new())]);
    let entries = &entries[..entries.len().min(MAX_GROUPS)];
    let list = |pick: fn(&(String, String)) -> &str| {
        entries.iter().map(pick).collect::<Vec<_>>().join(",")
    };
    let names = Names {
        rules: session.rules.clone(),
        model: session.model.clone(),
        layout: list(|(layout, _)| layout),
        variant: list(|(_, variant)| variant),
        options,
    };
    let mut all = vec![names.clone()];
    all.extend(
        session
            .other_options
            .clone()
            .map(|options| Names { options, ..names }),
    );
    Ok(all)
}

fn holders_for(names: &Result<Vec<Names>, String>) -> Result<xkb::Holders, String> {
    let names = names.as_ref().map_err(String::clone)?;
    if !xkb::available() {
        return Err("libxkbcommon is not installed".to_string());
    }
    xkb::holders(names, 0).map_err(|e| format!("{:#}", e))
}

/// The keys for a paste chord. A paste must go out, so a modifier that Hush
/// cannot place keeps its key from a keymap without options.
fn paste_keys(holders: &Result<xkb::Holders, String>) -> ChordKeys {
    let defaults = ChordKeys::default();
    match holders {
        Ok(holders) => ChordKeys {
            control: holders.control.unwrap_or(defaults.control),
            shift: holders.shift.unwrap_or(defaults.shift),
        },
        Err(why) => {
            debug!("Paste chords use Left Ctrl and Left Shift: {}", why);
            defaults
        },
    }
}

fn control_key_for(session: Option<&Session>) -> Result<Key, String> {
    holders_for(&modifier_names(session))?
        .control
        .ok_or_else(|| "no key holds Ctrl with every XKB option list of the keyboard".to_string())
}

/// The keymap of the keyboard that the user types on, which decides what
/// its modifier keys type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserKeymap {
    pub names: Names,
    /// The active layout, when the session reports it.
    pub group: Option<u32>,
}

/// The user's keymap, or why Hush cannot know it.
pub fn user_keymap() -> Result<UserKeymap, String> {
    user_keymap_for(layout::keyboard().as_ref())
}

fn user_keymap_for(session: Option<&Session>) -> Result<UserKeymap, String> {
    let names = modifier_names(session)?
        .into_iter()
        .next()
        .ok_or_else(|| "the session names no keymap".to_string())?;
    let layouts = names.layout.split(',').count();
    let group = match session.map(|session| &session.active) {
        Some(Active::Index(index)) if *index < layouts => u32::try_from(*index).ok(),
        _ => None,
    };
    Ok(UserKeymap { names, group })
}

/// The key that holds Control on the session's keyboard, for a tap that
/// must not guess: with `ctrl:swapcaps`, a Left Ctrl tap toggles Caps Lock.
/// The error says why Hush cannot know the key.
pub fn control_key() -> Result<Key, String> {
    control_key_for(layout::session().as_ref())
}

/// Finds the typing for each take, and keeps the last table so an
/// unchanged layout does not compile again.
#[derive(Default)]
pub struct Resolver {
    reserved: Vec<u16>,
    /// The newest first.
    cached: Vec<(Vec<Request>, Arc<KeyTable>)>,
    chords: Option<(Result<Vec<Names>, String>, ChordKeys)>,
    /// Whether Shift can be locked for the last take.
    shift_locked: bool,
}

impl Resolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// evdev codes Hush must never press, such as the key of an X11 grab,
    /// which would take the press away from the window.
    pub fn reserve(&mut self, keys: Vec<u16>) {
        self.reserved = keys;
    }

    /// `leds` reads the Caps Lock LED of Hush's own keyboard.
    pub fn resolve(&mut self, leds: &mut dyn CapsLed) -> Typing {
        let session = layout::session();
        debug!("Keyboard session: {:?}", session);
        self.update_chords(session.as_ref());
        self.resolve_with(session.as_ref(), xkb::available(), leds)
    }

    /// Whether Shift can be locked (Shift Lock) as of the last
    /// [`Self::resolve`]. Every key Hush sends then gets Shift, so a paste
    /// needs Shift+Insert, and Enter does not send a chat message.
    pub fn shift_locked(&self) -> bool {
        self.shift_locked
    }

    /// The keys that hold Control and Shift for a paste chord, as of the
    /// last [`Self::resolve`].
    pub fn chords(&self) -> ChordKeys {
        self.chords
            .as_ref()
            .map(|(_, keys)| *keys)
            .unwrap_or_default()
    }

    fn update_chords(&mut self, session: Option<&Session>) {
        let names = modifier_names(session);
        if self
            .chords
            .as_ref()
            .is_some_and(|(cached, _)| *cached == names)
        {
            return;
        }
        let keys = paste_keys(&holders_for(&names));
        if keys != ChordKeys::default() {
            debug!(
                "The keymap moves a modifier: Ctrl is {:?}, Shift is {:?}",
                keys.control, keys.shift
            );
        }
        self.chords = Some((names, keys));
    }

    fn resolve_with(
        &mut self,
        session: Option<&Session>,
        library: bool,
        leds: &mut dyn CapsLed,
    ) -> Typing {
        let on = caps_on(session, leds);
        let typing = self.typing_for(session, library, on);
        self.shift_locked = match &typing {
            Typing::Keys { table, .. } => table.locks().shift_locked,
            _ => on == Some(true) && shift_lock_possible(session),
        };
        typing
    }

    fn typing_for(&mut self, session: Option<&Session>, library: bool, on: Option<bool>) -> Typing {
        match plan(session, library) {
            Plan::Paste { why, fix } => Typing::Paste { why, fix },
            Plan::UsTable { .. } if on == Some(true) => Typing::Paste {
                why: WHY_CAPS.to_string(),
                fix: FIX_CAPS,
            },
            Plan::UsTable { layout, why } => Typing::UsTable { layout, why },
            Plan::Compile {
                names,
                other_options,
                group,
                level3,
                layout,
                plain_us,
            } => {
                let locks = locks_for(session, group, on);
                let layout = if locks == [Locked::None] {
                    layout
                } else {
                    format!("{} with Caps Lock on", layout)
                };
                let request = Request {
                    names,
                    group,
                    level3,
                    reserved: self.reserved.clone(),
                    locale: xkb::session_locale(),
                    locked: Locked::None,
                };
                let mut option_lists = vec![request.names.options.clone()];
                option_lists.extend(other_options);
                let requests: Vec<Request> = option_lists
                    .iter()
                    .flat_map(|options| {
                        locks.iter().map(|&locked| {
                            let mut each = request.clone();
                            each.names.options = options.clone();
                            each.locked = locked;
                            each
                        })
                    })
                    .collect();
                if let Some((_, table)) = self.cached.iter().find(|(key, _)| *key == requests) {
                    return Typing::Keys {
                        table: Arc::clone(table),
                        layout,
                    };
                }
                match build_common(&requests) {
                    Ok(table) => {
                        let table = Arc::new(table);
                        self.cached.insert(0, (requests, Arc::clone(&table)));
                        self.cached.truncate(CACHED_TABLES);
                        Typing::Keys { table, layout }
                    },
                    Err(e) if plain_us && on == Some(true) => Typing::Paste {
                        why: format!("{} ({:#})", WHY_CAPS, e),
                        fix: FIX_NAMES,
                    },
                    Err(e) if plain_us => Typing::UsTable {
                        layout,
                        why: format!("{:#}", e),
                    },
                    Err(e) => Typing::Paste {
                        why: format!("{:#}", e),
                        fix: FIX_NAMES,
                    },
                }
            },
        }
    }
}

/// The keys that type the same characters with every request, for a
/// keyboard that can have any of their keymaps.
fn build_common(requests: &[Request]) -> anyhow::Result<KeyTable> {
    let mut tables = requests.iter().map(xkb::build);
    let first = tables
        .next()
        .ok_or_else(|| anyhow::anyhow!("no keymap to build"))??;
    tables.try_fold(first, |common, table| Ok(common.common(&table?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::layout::Layout;
    use crate::text::xkb::tests::xkb_ready;

    impl Resolver {
        fn resolve_for(&mut self, session: Option<&Session>, library: bool) -> Typing {
            self.resolve_with(session, library, &mut NoLeds)
        }
    }

    fn session(layout: &str, variant: &str, active: Active) -> Session {
        Session {
            layout: Layout::Xkb {
                layout: layout.into(),
                variant: variant.into(),
            },
            rules: String::new(),
            model: "pc105".into(),
            options: Some("grp:alt_shift_toggle".into()),
            other_options: None,
            active,
            locks: LockSource::Leds,
        }
    }

    fn compiles(plan: &Plan) -> (&Names, u32, bool) {
        match plan {
            Plan::Compile {
                names,
                group,
                level3,
                ..
            } => (names, *group, *level3),
            other => panic!("expected a keymap, got {other:?}"),
        }
    }

    fn pastes(plan: &Plan) -> &str {
        match plan {
            Plan::Paste { why, .. } => why,
            other => panic!("expected a paste, got {other:?}"),
        }
    }

    #[test]
    fn the_chord_keys_follow_the_options_whatever_the_active_layout() {
        let mut swapped = session("us,de", ",nodeadkeys", Active::Unknown);
        swapped.options = Some("ctrl:swap_lwin_lctl,ctrl:swap_rwin_rctl".into());
        let names = modifier_names(Some(&swapped)).unwrap();
        assert_eq!(names.len(), 1);
        assert_eq!(names[0].layout, "us,de");
        assert_eq!(names[0].options, "ctrl:swap_lwin_lctl,ctrl:swap_rwin_rctl");
        swapped.other_options = Some(String::new());
        assert_eq!(
            modifier_names(Some(&swapped)).unwrap()[1].options,
            "",
            "a keyboard that can have either option list"
        );
        swapped.other_options = None;

        let ibus = Session {
            layout: Layout::Described("ibus mozc-jp".into()),
            ..session("us", "", Active::Unknown)
        };
        let names = modifier_names(Some(&ibus)).unwrap();
        assert_eq!(
            (names[0].layout.as_str(), names[0].options.as_str()),
            ("us", "grp:alt_shift_toggle"),
            "an input method still gives its XKB options"
        );

        if !xkb_ready() {
            return;
        }
        let mut resolver = Resolver::new();
        resolver.update_chords(Some(&swapped));
        assert_eq!(
            resolver.chords().control,
            Key::LeftMeta,
            "Left Win types Control_L, so Ctrl+V is Left Win+V"
        );
        resolver.update_chords(Some(&session("us", "", Active::Unknown)));
        assert_eq!(resolver.chords(), ChordKeys::default());
        assert_eq!(control_key_for(Some(&swapped)), Ok(Key::LeftMeta));
        let mut swapcaps = ibus.clone();
        swapcaps.options = Some("ctrl:swapcaps".into());
        assert_eq!(control_key_for(Some(&swapcaps)), Ok(Key::RightCtrl));
    }

    #[test]
    fn the_user_keymap_has_the_session_options_and_the_active_layout() {
        let mut x11 = session("de,us", "", Active::Index(1));
        x11.options = Some("ctrl:swapcaps".into());
        x11.other_options = Some(String::new());
        let keymap = user_keymap_for(Some(&x11)).unwrap();
        assert_eq!(
            (keymap.names.layout.as_str(), keymap.names.options.as_str()),
            ("de,us", "ctrl:swapcaps"),
            "the user's keyboard has the session options, not the second list"
        );
        assert_eq!(keymap.group, Some(1));
        x11.active = Active::Unknown;
        assert_eq!(user_keymap_for(Some(&x11)).unwrap().group, None);
        x11.active = Active::Index(4);
        assert_eq!(
            user_keymap_for(Some(&x11)).unwrap().group,
            None,
            "an index past the list is unknown"
        );
        let sway = Session {
            options: None,
            ..session("us", "", Active::Index(0))
        };
        assert!(user_keymap_for(Some(&sway)).is_err());
        assert!(user_keymap_for(None).is_err());
    }

    #[test]
    fn the_mask_key_is_unknown_when_the_options_are() {
        let sway = Session {
            options: None,
            ..session("us", "", Active::Index(0))
        };
        let why = modifier_names(Some(&sway)).unwrap_err();
        assert!(why.contains("Sway"), "{why}");
        assert!(control_key_for(Some(&sway)).is_err());
        assert!(control_key_for(None).is_err(), "no session at all");

        let mut resolver = Resolver::new();
        resolver.update_chords(Some(&sway));
        assert_eq!(
            resolver.chords(),
            ChordKeys::default(),
            "a paste still goes out with the usual keys"
        );

        if !xkb_ready() {
            return;
        }
        let mut two_lists = session("us", "", Active::Unknown);
        two_lists.options = Some("ctrl:swap_lwin_lctl".into());
        two_lists.other_options = Some("ctrl:swap_rwin_rctl".into());
        let why = control_key_for(Some(&two_lists)).unwrap_err();
        assert!(why.contains("every XKB option list"), "{why}");
        resolver.update_chords(Some(&two_lists));
        assert_eq!(resolver.chords(), ChordKeys::default());
    }

    #[test]
    fn one_layout_compiles_whatever_the_active_index() {
        let plan = plan(Some(&session("de", "nodeadkeys", Active::Unknown)), true);
        let (names, group, level3) = compiles(&plan);
        assert_eq!(names.layout, "de");
        assert_eq!(names.variant, "nodeadkeys");
        assert_eq!(names.model, "pc105");
        assert_eq!(names.options, "grp:alt_shift_toggle");
        assert_eq!(group, 0);
        assert!(level3);
    }

    #[test]
    fn several_layouts_need_the_active_one() {
        let known = plan(
            Some(&session("us,de", ",nodeadkeys", Active::Index(1))),
            true,
        );
        let (names, group, _) = compiles(&known);
        assert_eq!(
            (names.layout.as_str(), names.variant.as_str()),
            ("us,de", ",nodeadkeys")
        );
        assert_eq!(group, 1);

        let unknown = plan(Some(&session("us,de", "", Active::Unknown)), true);
        assert!(pastes(&unknown).contains("cannot tell which one is active"));
        let unclear = plan(
            Some(&session(
                "us,de",
                "",
                Active::Unclear {
                    why: "KWin disagrees".into(),
                    fix: "ask KWin",
                },
            )),
            true,
        );
        assert_eq!(pastes(&unclear), "KWin disagrees");
        assert!(
            matches!(
                unclear,
                Plan::Paste {
                    fix: "ask KWin",
                    ..
                }
            ),
            "the reason brings its own fix"
        );
        let out_of_range = plan(Some(&session("us,de", "", Active::Index(2))), true);
        pastes(&out_of_range);
    }

    #[test]
    fn inline_variants_become_the_variant_list() {
        let plan = plan(
            Some(&session("us(intl),us+dvorak", "", Active::Index(0))),
            true,
        );
        let (names, _, _) = compiles(&plan);
        assert_eq!(names.layout, "us,us");
        assert_eq!(names.variant, "intl,dvorak");
    }

    #[test]
    fn unknown_options_leave_out_the_third_level() {
        let sway = Session {
            options: None,
            ..session("us,de", "", Active::Index(1))
        };
        let plan = plan(Some(&sway), true);
        let (names, _, level3) = compiles(&plan);
        assert!(!level3, "an unknown option can move AltGr");
        assert_eq!(names.options, "");
    }

    #[test]
    fn more_than_four_layouts_compile_the_active_one_alone() {
        let plan = plan(
            Some(&session("us,de,fr,es,it", ",,,,", Active::Index(3))),
            true,
        );
        let (names, group, _) = compiles(&plan);
        assert_eq!((names.layout.as_str(), group), ("es", 0));
    }

    #[test]
    fn an_empty_layout_compiles_the_libxkbcommon_default() {
        let kwin = session("", "", Active::Index(0));
        let plan = plan(Some(&kwin), true);
        let (names, group, _) = compiles(&plan);
        assert_eq!((names.layout.as_str(), group), ("", 0));
        assert!(matches!(
            &plan,
            Plan::Compile { layout, plain_us: true, .. } if layout == "the default layout"
        ));
        if !xkb_ready() {
            return;
        }
        let typing = Resolver::new().resolve_for(Some(&kwin), true);
        assert_eq!(typing.to_string(), "types key by key on the default layout");
        assert!(typing
            .table()
            .is_some_and(|table| table.covers("Hello, world!")));
    }

    #[test]
    fn a_second_option_list_goes_to_the_plan() {
        let x11 = Session {
            other_options: Some("compose:102".into()),
            ..session("de", "", Active::Unknown)
        };
        let plan = plan(Some(&x11), true);
        assert!(
            matches!(
                &plan,
                Plan::Compile { other_options: Some(other), level3: true, layout, .. }
                    if other == "compose:102" && layout.contains("two option lists")
            ),
            "{plan:?}"
        );
    }

    #[test]
    fn two_option_lists_type_only_the_keys_they_agree_on() {
        if !xkb_ready() {
            return;
        }
        let mut resolver = Resolver::new();
        let x11 = Session {
            options: Some("compose:102,caps:escape".into()),
            other_options: Some("compose:102".into()),
            ..session("de", "", Active::Unknown)
        };
        let both = resolver.resolve_for(Some(&x11), true);
        let table = both.table().expect("German types");
        assert!(table.covers("Größe: 3 € @ café"), "{both}");
        assert!(!table.covers("<"), "the ISO key is Compose in both lists");

        let escape = Session {
            options: Some("caps:escape".into()),
            ..x11.clone()
        };
        let table = resolver
            .resolve_for(Some(&escape), true)
            .table()
            .cloned()
            .expect("German types");
        assert!(
            !table.covers("a < b"),
            "with the default options the ISO key is Compose, so < pastes"
        );
        assert!(table.covers("Größe @"));
        let alone = Session {
            other_options: None,
            ..escape
        };
        assert!(resolver
            .resolve_for(Some(&alone), true)
            .table()
            .is_some_and(|table| table.covers("a < b")));

        let moved = Session {
            options: Some("lv3:ralt_alt,lv3:caps_switch".into()),
            other_options: Some(String::new()),
            ..session("de", "", Active::Unknown)
        };
        let table = resolver
            .resolve_for(Some(&moved), true)
            .table()
            .cloned()
            .expect("German types");
        assert!(!table.covers("@"), "AltGr is on another key in each list");
        assert!(table.covers("Größe café"));
    }

    #[test]
    fn a_described_layout_pastes() {
        let ime = Session {
            layout: Layout::Described("ibus mozc-jp".into()),
            ..session("us", "", Active::Index(0))
        };
        assert!(pastes(&plan(Some(&ime), true)).contains("ibus mozc-jp"));
    }

    #[test]
    fn without_libxkbcommon_only_plain_us_types() {
        assert!(matches!(
            plan(Some(&session("us", "", Active::Unknown)), false),
            Plan::UsTable { .. }
        ));
        assert!(matches!(
            plan(Some(&session("us,de", "", Active::Index(0))), false),
            Plan::UsTable { .. }
        ));
        assert!(
            pastes(&plan(Some(&session("us,de", "", Active::Index(1))), false))
                .contains("libxkbcommon")
        );
        assert!(
            pastes(&plan(Some(&session("us", "intl", Active::Unknown)), false))
                .contains("not plain US")
        );
        assert!(matches!(plan(None, false), Plan::UsTable { .. }));
    }

    #[test]
    fn an_unknown_session_compiles_us() {
        let plan = plan(None, true);
        let (names, group, level3) = compiles(&plan);
        assert_eq!(names.layout, "us");
        assert_eq!(group, 0);
        assert!(!level3);
    }

    #[test]
    fn the_resolver_types_through_the_keymap_and_falls_back() {
        if !xkb_ready() {
            return;
        }
        let mut resolver = Resolver::new();
        let de = resolver.resolve_for(Some(&session("us,de", "", Active::Index(1))), true);
        let table = de.table().expect("German types");
        assert!(table.covers("Größe"));
        assert!(de.to_string().starts_with("types key by key on de"), "{de}");
        let again = resolver.resolve_for(Some(&session("us,de", "", Active::Index(1))), true);
        assert!(
            matches!(
                (&de, &again),
                (Typing::Keys { table: a, .. }, Typing::Keys { table: b, .. }) if Arc::ptr_eq(a, b)
            ),
            "an unchanged layout reuses the table"
        );

        let missing = resolver.resolve_for(Some(&session("xx", "", Active::Unknown)), true);
        assert!(missing.table().is_none(), "{missing}");
        let bad_us = Session {
            rules: "no-such-rules".into(),
            ..session("us", "", Active::Unknown)
        };
        let fallback = resolver.resolve_for(Some(&bad_us), true);
        assert!(
            matches!(fallback, Typing::UsTable { .. }),
            "plain US keeps the US table: {fallback}"
        );
        assert!(resolver
            .resolve_for(Some(&session("us,de", "", Active::Unknown)), true)
            .table()
            .is_none());
    }

    /// A fake Caps Lock LED that counts its reads.
    struct FakeLed {
        on: Option<bool>,
        reads: usize,
    }

    impl FakeLed {
        fn new(on: Option<bool>) -> Self {
            Self { on, reads: 0 }
        }
    }

    impl CapsLed for FakeLed {
        fn caps_lock(&mut self) -> Option<bool> {
            self.reads += 1;
            self.on
        }
    }

    fn locks(session: &Session, group: u32, led: &mut FakeLed) -> Vec<Locked> {
        locks_for(Some(session), group, caps_on(Some(session), led))
    }

    #[test]
    fn the_led_and_the_options_decide_the_locks() {
        let known = session("de", "", Active::Unknown);
        let mut led = FakeLed::new(Some(true));
        assert_eq!(locks(&known, 0, &mut led), [Locked::CapsIndicator]);
        assert_eq!(led.reads, 1);
        for off in [Some(false), None] {
            assert_eq!(
                locks(&known, 0, &mut FakeLed::new(off)),
                [Locked::None],
                "{off:?}: an LED that is off or unreadable types as before"
            );
        }

        let sway = Session {
            options: None,
            ..known.clone()
        };
        assert_eq!(
            locks(&sway, 0, &mut FakeLed::new(Some(true))),
            [Locked::Named(xkb::LOCK_MOD), Locked::Named(xkb::SHIFT_MOD)],
            "unknown options: the LED shows Lock, or Shift with caps:shiftlock"
        );
        assert_eq!(
            locks(&sway, 1, &mut FakeLed::new(Some(true))),
            [
                Locked::Named(xkb::LOCK_MOD),
                Locked::Named(xkb::SHIFT_MOD),
                Locked::None
            ],
            "on a second layout, the LED can show the layout"
        );

        let hyprland = Session {
            locks: LockSource::Reported(true),
            ..known.clone()
        };
        let mut led = FakeLed::new(Some(false));
        assert_eq!(locks(&hyprland, 0, &mut led), [Locked::CapsIndicator]);
        assert_eq!(led.reads, 0, "Hyprland's LEDs come from other keyboards");
        let unknown = Session {
            locks: LockSource::Unknown,
            ..known
        };
        let mut led = FakeLed::new(Some(true));
        assert_eq!(locks(&unknown, 0, &mut led), [Locked::None]);
        assert_eq!(led.reads, 0);
    }

    #[test]
    fn the_us_table_pastes_with_caps_lock_on() {
        let us = session("us", "", Active::Unknown);
        let mut resolver = Resolver::new();
        let on = resolver.resolve_with(Some(&us), false, &mut FakeLed::new(Some(true)));
        assert!(on.table().is_none(), "{on}");
        assert!(on.to_string().contains("Caps Lock is on"), "{on}");
        assert!(!resolver.shift_locked(), "Lock, not Shift Lock");
        let off = resolver.resolve_with(Some(&us), false, &mut FakeLed::new(Some(false)));
        assert!(matches!(off, Typing::UsTable { .. }), "{off}");
        let unknown = resolver.resolve_with(Some(&us), false, &mut FakeLed::new(None));
        assert!(matches!(unknown, Typing::UsTable { .. }), "{unknown}");

        let shift_lock = Session {
            options: Some("caps:shiftlock".into()),
            ..us.clone()
        };
        resolver.resolve_with(Some(&shift_lock), false, &mut FakeLed::new(Some(true)));
        assert!(resolver.shift_locked());
        resolver.resolve_with(Some(&shift_lock), false, &mut FakeLed::new(Some(false)));
        assert!(!resolver.shift_locked());

        if !xkb_ready() {
            return;
        }
        let bad_us = Session {
            rules: "no-such-rules".into(),
            ..us
        };
        let failed = resolver.resolve_with(Some(&bad_us), true, &mut FakeLed::new(Some(true)));
        assert!(
            failed.table().is_none(),
            "a failed keymap on US with Caps Lock on pastes: {failed}"
        );
        let failed_off = resolver.resolve_with(Some(&bad_us), true, &mut FakeLed::new(Some(false)));
        assert!(matches!(failed_off, Typing::UsTable { .. }), "{failed_off}");
    }

    #[test]
    fn shift_lock_is_known_for_each_take() {
        if !xkb_ready() {
            return;
        }
        let mut resolver = Resolver::new();
        let shift_lock = Session {
            options: Some("caps:shiftlock".into()),
            ..session("us", "", Active::Unknown)
        };
        let typing = resolver.resolve_with(Some(&shift_lock), true, &mut FakeLed::new(Some(true)));
        assert!(resolver.shift_locked(), "{typing}");
        let table = typing.table().expect("US types");
        assert!(table.covers("HELLO"));
        assert!(
            !table.covers("HELLO\nWORLD"),
            "no key holds Shift, so a line break pastes"
        );
        resolver.resolve_with(Some(&shift_lock), true, &mut FakeLed::new(Some(false)));
        assert!(!resolver.shift_locked());

        let caps = session("us", "", Active::Unknown);
        let typing = resolver.resolve_with(Some(&caps), true, &mut FakeLed::new(Some(true)));
        assert!(!resolver.shift_locked());
        assert!(typing
            .table()
            .is_some_and(|table| table.covers("Hi\nthere")));

        let sway = Session {
            options: None,
            ..caps
        };
        resolver.resolve_with(Some(&sway), true, &mut FakeLed::new(Some(true)));
        assert!(resolver.shift_locked(), "Sway can have caps:shiftlock");
    }

    #[test]
    fn caps_lock_types_each_character_as_written() {
        if !xkb_ready() {
            return;
        }
        let us = session("us", "", Active::Unknown);
        let mut resolver = Resolver::new();
        let on = resolver.resolve_with(Some(&us), true, &mut FakeLed::new(Some(true)));
        assert_eq!(on.to_string(), "types key by key on us with Caps Lock on");
        let table = on.table().expect("US types");
        assert_eq!(
            table.taps('h'),
            Some(&[crate::text::keymap::Tap::shifted(Key::H)][..])
        );
        assert!(table.covers("Hello World"));

        let off = resolver.resolve_with(Some(&us), true, &mut FakeLed::new(Some(false)));
        assert_eq!(off.to_string(), "types key by key on us");
        let again = resolver.resolve_with(Some(&us), true, &mut FakeLed::new(Some(true)));
        assert!(
            matches!(
                (&on, &again),
                (Typing::Keys { table: a, .. }, Typing::Keys { table: b, .. }) if Arc::ptr_eq(a, b)
            ),
            "Caps Lock on and off each keep a table"
        );

        let sway = Session {
            options: None,
            ..us
        };
        let unsure = resolver.resolve_with(Some(&sway), true, &mut FakeLed::new(Some(true)));
        let table = unsure.table().expect("US types");
        assert!(table.covers("HELLO WORLD"));
        assert!(
            !table.covers("Hello"),
            "Lock and Shift Lock disagree on lower case, so it pastes"
        );
        assert!(!table.covers("42"), "and on digits");
    }

    #[test]
    fn a_reserved_altgr_key_is_never_pressed() {
        if !xkb_ready() {
            return;
        }
        let mut resolver = Resolver::new();
        resolver.reserve(vec![input_linux::Key::RightAlt.code()]);
        let typing = resolver.resolve_for(Some(&session("de", "", Active::Unknown)), true);
        let table = typing.table().expect("German types");
        assert!(table.covers("Größe"));
        assert!(!table.covers("@"), "AltGr is reserved");
    }
}
