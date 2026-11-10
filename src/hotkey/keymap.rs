//! What the modifier keys type. A modifier name in a chord (Ctrl, Shift,
//! Alt, or Super) means each key that types that modifier in the session
//! keymap, as on the X11 backend and in the paste chords. An XKB option can
//! move a modifier: with `ctrl:swap_lwin_lctl`, the Win key types Ctrl, so
//! `Ctrl+RightAlt` fires with the Win key. A key alone, such as `RightAlt`
//! or `RightCtrl`, is the physical key.
//!
//! When Hush cannot read the keymap options, as on Sway, the names keep
//! their physical keys: Ctrl is the Left Ctrl and Right Ctrl keys.

use super::combination::{KeyCombination, Modifiers};
use crate::text::layout::Layout;
use crate::text::typing::{user_keymap, UserKeymap};
use crate::text::xkb::{self, KeyClasses};
use evdev::KeyCode;
use tracing::{debug, info};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModifierKeys {
    ctrl: Vec<KeyCode>,
    shift: Vec<KeyCode>,
    alt: Vec<KeyCode>,
    super_keys: Vec<KeyCode>,
    /// The keys that hold another modifier, such as AltGr. A held one
    /// blocks a chord, as a held Ctrl blocks a bare key.
    blocking: Vec<KeyCode>,
    /// The keys that type AltGr, or `None` when Hush cannot read the keymap.
    altgr: Option<Vec<KeyCode>>,
}

impl Default for ModifierKeys {
    fn default() -> Self {
        Self::physical()
    }
}

impl ModifierKeys {
    /// The keys by their names, for when Hush cannot read the keymap.
    pub fn physical() -> Self {
        Self {
            ctrl: vec![KeyCode::KEY_LEFTCTRL, KeyCode::KEY_RIGHTCTRL],
            shift: vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_RIGHTSHIFT],
            alt: vec![KeyCode::KEY_LEFTALT, KeyCode::KEY_RIGHTALT],
            super_keys: vec![KeyCode::KEY_LEFTMETA, KeyCode::KEY_RIGHTMETA],
            blocking: Vec::new(),
            altgr: None,
        }
    }

    pub fn from_classes(classes: &KeyClasses) -> Self {
        let codes = |keys: &[input_linux::Key]| -> Vec<KeyCode> {
            keys.iter().map(|key| KeyCode::new(key.code())).collect()
        };
        let altgr = codes(&classes.altgr);
        Self {
            ctrl: codes(&classes.control),
            shift: codes(&classes.shift),
            alt: codes(&classes.alt),
            super_keys: codes(&classes.super_keys),
            blocking: altgr.iter().copied().chain(codes(&classes.other)).collect(),
            altgr: Some(altgr),
        }
    }

    /// The keys of a keymap, or why Hush cannot compile it.
    pub fn of_keymap(keymap: &UserKeymap) -> Result<Self, String> {
        if !xkb::available() {
            return Err("libxkbcommon is not installed".to_string());
        }
        xkb::key_classes(&keymap.names, keymap.group)
            .map(|classes| Self::from_classes(&classes))
            .map_err(|e| format!("{:#}", e))
    }

    /// The keys of the user's keymap, or the physical keys when Hush cannot
    /// read it.
    pub fn session() -> Self {
        user_keymap()
            .and_then(|keymap| Self::of_keymap(&keymap))
            .unwrap_or_else(|_| Self::physical())
    }

    /// Whether the keys come from the keymap, not from their names.
    pub fn from_keymap(&self) -> bool {
        self.altgr.is_some()
    }

    /// The modifiers that the held keys hold, or `None` when a held key
    /// holds another modifier, such as AltGr. Other keys count for nothing.
    pub fn held(&self, keys: impl IntoIterator<Item = KeyCode>) -> Option<Modifiers> {
        let mut modifiers = Modifiers::default();
        for key in keys {
            if self.blocking.contains(&key) {
                return None;
            }
            self.add_key(&mut modifiers, key);
        }
        Some(modifiers)
    }

    /// Add the modifier that a key types; other keys leave it unchanged.
    pub fn add_key(&self, modifiers: &mut Modifiers, key: KeyCode) {
        modifiers.ctrl |= self.ctrl.contains(&key);
        modifiers.shift |= self.shift.contains(&key);
        modifiers.alt |= self.alt.contains(&key);
        modifiers.super_key |= self.super_keys.contains(&key);
    }

    /// Whether each key of the chord types Alt or Super, such as a bare key
    /// that types Super. When such a chord goes up with no other key in
    /// between, many applications open their menu bar and many desktops
    /// open their launcher.
    pub fn only_alt_or_super(&self, chord: &KeyCombination) -> bool {
        let key = chord.evdev_key();
        let main = self.alt.contains(&key) || self.super_keys.contains(&key);
        main && !chord.modifiers.ctrl && !chord.modifiers.shift
    }

    /// The keys that type AltGr. Without the keymap, Right Alt is AltGr on
    /// every layout but plain US, and an unknown layout counts as US.
    pub fn altgr_keys(&self, layout: Option<&Layout>) -> Vec<KeyCode> {
        match &self.altgr {
            Some(keys) => keys.clone(),
            None if layout.is_some_and(|layout| !layout.is_plain_us()) => {
                vec![KeyCode::KEY_RIGHTALT]
            },
            None => Vec::new(),
        }
    }

    /// A key that types Ctrl, to use alone in place of AltGr: Right Ctrl when
    /// it types Ctrl, else another key on the right that does, else any.
    pub fn ctrl_key(&self) -> Option<KeyCode> {
        let right = [
            KeyCode::KEY_RIGHTCTRL,
            KeyCode::KEY_RIGHTMETA,
            KeyCode::KEY_RIGHTALT,
            KeyCode::KEY_COMPOSE,
        ];
        right
            .into_iter()
            .find(|key| self.ctrl.contains(key))
            .or_else(|| self.ctrl.first().copied())
    }

    /// One line for the log: the keys that type each modifier.
    pub fn describe(&self) -> String {
        let names = |keys: &[KeyCode]| {
            let names: Vec<String> = keys
                .iter()
                .filter_map(|key| KeyCombination::from_evdev(*key, Modifiers::default()))
                .map(|chord| chord.to_string())
                .collect();
            match names.as_slice() {
                [] => "no key".to_string(),
                names => names.join(" and "),
            }
        };
        format!(
            "Ctrl is {}, Shift is {}, Alt is {}, Super is {}",
            names(&self.ctrl),
            names(&self.shift),
            names(&self.alt),
            names(&self.super_keys)
        )
    }
}

/// Reads the user's keymap again on each call, and changes the modifier
/// keys only on a steady change. A read that fails keeps the keys, and new
/// names count only when two reads in a row give them: a one-off failure of
/// hyprctl, gsettings, or D-Bus can fall back to other names, and those
/// would swap Ctrl and Super until the next read.
#[derive(Default)]
pub struct KeymapWatch {
    /// The keymap in use: `None` before the first read, and `Some(None)`
    /// for the physical keys.
    applied: Option<Option<UserKeymap>>,
    /// New names from the last read, which the next read must give again.
    pending: Option<UserKeymap>,
}

impl KeymapWatch {
    pub fn new() -> Self {
        Self::default()
    }

    /// The modifier keys when they change. The first call always gives
    /// keys, and the log says once why Hush keeps the physical keys.
    pub fn changed(&mut self) -> Option<ModifierKeys> {
        self.changed_to(user_keymap())
    }

    fn changed_to(&mut self, read: Result<UserKeymap, String>) -> Option<ModifierKeys> {
        let Some(applied) = &self.applied else {
            let compiled = read.and_then(|keymap| Ok((ModifierKeys::of_keymap(&keymap)?, keymap)));
            let (keys, keymap) = match compiled {
                Ok((keys, keymap)) => {
                    info!(
                        "Modifier names in hotkeys follow the keymap: {}",
                        keys.describe()
                    );
                    (keys, Some(keymap))
                },
                Err(why) => {
                    info!(
                        "Modifier names in hotkeys mean the keys with those names: {}",
                        why
                    );
                    (ModifierKeys::physical(), None)
                },
            };
            self.applied = Some(keymap);
            return Some(keys);
        };
        let keymap = match read {
            Ok(keymap) => keymap,
            Err(why) => {
                debug!(
                    "The keymap read failed ({}); the hotkeys keep their keys",
                    why
                );
                self.pending = None;
                return None;
            },
        };
        if applied.as_ref() == Some(&keymap) {
            self.pending = None;
            return None;
        }
        if self.pending.as_ref() != Some(&keymap) {
            debug!("The keymap names changed; the next read must give them again");
            self.pending = Some(keymap);
            return None;
        }
        self.pending = None;
        match ModifierKeys::of_keymap(&keymap) {
            Ok(keys) => {
                info!(
                    "Modifier names in hotkeys follow the new keymap: {}",
                    keys.describe()
                );
                self.applied = Some(Some(keymap));
                Some(keys)
            },
            Err(why) => {
                debug!(
                    "The new keymap does not compile ({}); the hotkeys keep their keys",
                    why
                );
                None
            },
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use input_linux::Key;

    /// The keys of `us` with `ctrl:swap_lwin_lctl,ctrl:swap_rwin_rctl`: the
    /// Win keys type Ctrl, and the Ctrl keys type Super.
    pub(crate) fn swapped() -> ModifierKeys {
        ModifierKeys::from_classes(&KeyClasses {
            control: vec![Key::LeftMeta, Key::RightMeta],
            shift: vec![Key::LeftShift, Key::RightShift],
            alt: vec![Key::LeftAlt, Key::RightAlt],
            super_keys: vec![Key::LeftCtrl, Key::RightCtrl],
            altgr: Vec::new(),
            other: Vec::new(),
        })
    }

    /// The keys of `de`: Right Alt is AltGr.
    pub(crate) fn german() -> ModifierKeys {
        ModifierKeys::from_classes(&KeyClasses {
            control: vec![Key::LeftCtrl, Key::RightCtrl],
            shift: vec![Key::LeftShift, Key::RightShift],
            alt: vec![Key::LeftAlt],
            super_keys: vec![Key::LeftMeta, Key::RightMeta],
            altgr: vec![Key::RightAlt],
            other: Vec::new(),
        })
    }

    fn chord(text: &str) -> KeyCombination {
        KeyCombination::parse(text).unwrap()
    }

    #[test]
    fn modifier_names_follow_what_the_keys_type() {
        let keys = swapped();
        assert_eq!(
            keys.held([KeyCode::KEY_LEFTMETA]),
            Some(chord("Ctrl+A").modifiers),
            "the Win key types Ctrl"
        );
        assert_eq!(
            keys.held([KeyCode::KEY_RIGHTCTRL, KeyCode::KEY_A]),
            Some(chord("Super+A").modifiers),
            "a letter counts for nothing"
        );
        let physical = ModifierKeys::physical();
        assert_eq!(
            physical.held([KeyCode::KEY_LEFTMETA]),
            Some(chord("Super+A").modifiers)
        );
        assert_eq!(
            german().held([KeyCode::KEY_RIGHTALT]),
            None,
            "a held AltGr blocks every chord"
        );
        assert_eq!(
            german().held([KeyCode::KEY_LEFTALT]),
            Some(chord("Alt+A").modifiers)
        );
    }

    #[test]
    fn the_mask_follows_what_a_bare_key_types() {
        let keys = swapped();
        assert!(keys.only_alt_or_super(&chord("LeftCtrl")), "it types Super");
        assert!(
            !keys.only_alt_or_super(&chord("LeftSuper")),
            "it types Ctrl"
        );
        assert!(keys.only_alt_or_super(&chord("RightAlt")));
        assert!(!keys.only_alt_or_super(&chord("Ctrl+RightAlt")));
        assert!(
            !german().only_alt_or_super(&chord("RightAlt")),
            "AltGr opens no menu"
        );
        let physical = ModifierKeys::physical();
        for text in [
            "RightAlt",
            "AltGr",
            "LeftAlt",
            "LeftSuper",
            "RightMeta",
            "Super+RightAlt",
            "Alt+LeftMeta",
            "Alt+Super+RightAlt",
        ] {
            assert!(physical.only_alt_or_super(&chord(text)), "{text}");
        }
        for text in [
            "Ctrl+RightAlt",
            "Shift+RightAlt",
            "Ctrl+LeftSuper",
            "LeftCtrl",
            "F13",
            "Mouse4",
            "Alt+Space",
            "Super+F",
        ] {
            assert!(!physical.only_alt_or_super(&chord(text)), "{text}");
        }
    }

    #[test]
    fn altgr_and_the_key_to_use_instead_come_from_the_keymap() {
        let us = Layout::Xkb {
            layout: "us".into(),
            variant: String::new(),
        };
        let de = Layout::Xkb {
            layout: "de".into(),
            variant: String::new(),
        };
        assert_eq!(german().altgr_keys(Some(&us)), [KeyCode::KEY_RIGHTALT]);
        assert!(
            swapped().altgr_keys(Some(&de)).is_empty(),
            "the keymap decides"
        );
        let physical = ModifierKeys::physical();
        assert_eq!(physical.altgr_keys(Some(&de)), [KeyCode::KEY_RIGHTALT]);
        assert!(physical.altgr_keys(Some(&us)).is_empty());
        assert!(physical.altgr_keys(None).is_empty());

        assert_eq!(german().ctrl_key(), Some(KeyCode::KEY_RIGHTCTRL));
        assert_eq!(
            swapped().ctrl_key(),
            Some(KeyCode::KEY_RIGHTMETA),
            "Right Ctrl types Super there"
        );
        assert_eq!(physical.ctrl_key(), Some(KeyCode::KEY_RIGHTCTRL));
    }

    #[test]
    fn unknown_options_keep_the_physical_keys() {
        let mut watch = KeymapWatch::new();
        let sway = Err("the compositor does not report its XKB options".to_string());
        assert_eq!(
            watch.changed_to(sway.clone()),
            Some(ModifierKeys::physical())
        );
        assert_eq!(watch.changed_to(sway), None, "nothing changes");
    }

    fn keymap(layout: &str, options: &str, group: Option<u32>) -> Result<UserKeymap, String> {
        Ok(UserKeymap {
            names: crate::text::xkb::Names {
                layout: layout.into(),
                options: options.into(),
                ..Default::default()
            },
            group,
        })
    }

    const SWAP: &str = "ctrl:swap_lwin_lctl,ctrl:swap_rwin_rctl";

    #[test]
    fn the_watch_follows_the_options_it_is_given() {
        if !crate::text::xkb::tests::xkb_ready() {
            return;
        }
        let mut watch = KeymapWatch::new();
        assert_eq!(
            watch.changed_to(keymap("us", SWAP, Some(0))),
            Some(swapped()),
            "the first read applies at once"
        );
        assert_eq!(watch.changed_to(keymap("us", SWAP, Some(0))), None);
        assert_eq!(
            watch.changed_to(keymap("us", "ctrl:swapcaps", Some(0))),
            None
        );
        let swapcaps = watch
            .changed_to(keymap("us", "ctrl:swapcaps", Some(0)))
            .expect("the second read in a row applies");
        assert_eq!(
            swapcaps.held([KeyCode::KEY_CAPSLOCK]),
            Some(chord("Ctrl+A").modifiers)
        );
        assert_eq!(
            swapcaps.held([KeyCode::KEY_LEFTCTRL]),
            Some(Modifiers::default()),
            "Left Ctrl types Caps Lock"
        );
        let alt_win = KeymapWatch::new()
            .changed_to(keymap("us", "altwin:swap_alt_win", None))
            .unwrap();
        assert_eq!(
            alt_win.held([KeyCode::KEY_LEFTMETA]),
            Some(chord("Alt+A").modifiers)
        );
        assert!(
            alt_win.only_alt_or_super(&chord("RightAlt")),
            "it types Super"
        );
    }

    #[test]
    fn a_failed_or_odd_read_keeps_the_keys() {
        if !crate::text::xkb::tests::xkb_ready() {
            return;
        }
        let mut watch = KeymapWatch::new();
        watch.changed_to(keymap("us", SWAP, Some(0)));
        assert_eq!(
            watch.changed_to(Err("hyprctl did not answer".to_string())),
            None,
            "a failed read keeps the keymap's keys"
        );
        assert_eq!(
            watch.changed_to(keymap("us", "", None)),
            None,
            "names from a fallback source count only twice in a row"
        );
        assert_eq!(watch.changed_to(keymap("us", SWAP, Some(0))), None);
        assert_eq!(
            watch.changed_to(keymap("us", "", None)),
            None,
            "the last read gave the keymap in use, so this one starts over"
        );
        assert_eq!(watch.changed_to(Err("D-Bus timed out".to_string())), None);
        assert_eq!(
            watch.changed_to(keymap("us", "", None)),
            None,
            "a failed read between them breaks the run"
        );

        let mut watch = KeymapWatch::new();
        assert_eq!(
            watch.changed_to(Err("gsettings failed".to_string())),
            Some(ModifierKeys::physical())
        );
        assert_eq!(watch.changed_to(keymap("us", SWAP, Some(0))), None);
        assert_eq!(
            watch.changed_to(keymap("us", SWAP, Some(0))),
            Some(swapped()),
            "a steady keymap replaces the physical keys"
        );
    }

    #[test]
    fn the_active_layout_decides_what_right_alt_types() {
        if !crate::text::xkb::tests::xkb_ready() {
            return;
        }
        let of = |group| ModifierKeys::of_keymap(&keymap("us,de", "", group).unwrap()).unwrap();
        assert!(
            of(Some(0)).only_alt_or_super(&chord("RightAlt")),
            "Alt on us"
        );
        assert!(
            !of(Some(1)).only_alt_or_super(&chord("RightAlt")),
            "AltGr on de"
        );
        assert_eq!(of(Some(1)).held([KeyCode::KEY_RIGHTALT]), None);
        assert_eq!(
            of(None).held([KeyCode::KEY_RIGHTALT]),
            Some(chord("Alt+A").modifiers),
            "with the active layout unknown, Right Alt keeps its name"
        );
    }
}
