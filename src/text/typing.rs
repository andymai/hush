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

use super::keymap::KeyTable;
use super::layout::{self, plain_us, Active, Session, FIX_SEVERAL};
use super::xkb::{self, Names, Request};
use std::fmt;
use std::sync::Arc;
use tracing::debug;

/// XKB keymaps hold at most four layouts.
const MAX_GROUPS: usize = 4;

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

/// Finds the typing for each take, and keeps the last table so an
/// unchanged layout does not compile again.
#[derive(Default)]
pub struct Resolver {
    reserved: Vec<u16>,
    cached: Option<(Vec<Request>, Arc<KeyTable>)>,
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

    pub fn resolve(&mut self) -> Typing {
        let session = layout::session();
        debug!("Keyboard session: {:?}", session);
        self.resolve_for(session.as_ref(), xkb::available())
    }

    fn resolve_for(&mut self, session: Option<&Session>, library: bool) -> Typing {
        match plan(session, library) {
            Plan::Paste { why, fix } => Typing::Paste { why, fix },
            Plan::UsTable { layout, why } => Typing::UsTable { layout, why },
            Plan::Compile {
                names,
                other_options,
                group,
                level3,
                layout,
                plain_us,
            } => {
                let request = Request {
                    names,
                    group,
                    level3,
                    reserved: self.reserved.clone(),
                    locale: xkb::session_locale(),
                };
                let mut requests = vec![request.clone()];
                if let Some(options) = other_options {
                    let mut other = request;
                    other.names.options = options;
                    requests.push(other);
                }
                if let Some((cached, table)) = &self.cached {
                    if *cached == requests {
                        return Typing::Keys {
                            table: Arc::clone(table),
                            layout,
                        };
                    }
                }
                match build_common(&requests) {
                    Ok(table) => {
                        let table = Arc::new(table);
                        self.cached = Some((requests, Arc::clone(&table)));
                        Typing::Keys { table, layout }
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
