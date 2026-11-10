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
use crate::text::xkb::{self, KeyClasses, Names};
use evdev::KeyCode;
use tracing::info;

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

    /// The keys of the keymap with these names, or the physical keys and
    /// why, when Hush cannot read the keymap.
    pub fn of_names(names: &Result<Names, String>) -> (Self, Option<String>) {
        let names = match names {
            Ok(names) => names,
            Err(why) => return (Self::physical(), Some(why.clone())),
        };
        if !xkb::available() {
            return (
                Self::physical(),
                Some("libxkbcommon is not installed".to_string()),
            );
        }
        match xkb::key_classes(names) {
            Ok(classes) => (Self::from_classes(&classes), None),
            Err(e) => (Self::physical(), Some(format!("{:#}", e))),
        }
    }

    /// The keys of the session's keymap.
    pub fn session() -> Self {
        Self::of_names(&crate::text::typing::keyboard_names()).0
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

/// Reads the keymap names again on each call, and compiles a keymap only
/// when they change, so a hotkey reader can follow a change of the XKB
/// options.
#[derive(Default)]
pub struct KeymapWatch {
    names: Option<Result<Names, String>>,
}

impl KeymapWatch {
    pub fn new() -> Self {
        Self::default()
    }

    /// The modifier keys when the keymap names changed since the last call.
    /// The first call always gives keys. The log says once why Hush keeps
    /// the physical keys.
    pub fn changed(&mut self) -> Option<ModifierKeys> {
        self.changed_to(crate::text::typing::keyboard_names())
    }

    fn changed_to(&mut self, names: Result<Names, String>) -> Option<ModifierKeys> {
        if self.names.as_ref() == Some(&names) {
            return None;
        }
        let (keys, why) = ModifierKeys::of_names(&names);
        match why {
            Some(why) => info!(
                "Modifier names in hotkeys mean the keys with those names: {}",
                why
            ),
            None => info!(
                "Modifier names in hotkeys follow the keymap: {}",
                keys.describe()
            ),
        }
        self.names = Some(names);
        Some(keys)
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
        assert_eq!(watch.changed_to(sway), None, "the same names compile once");
        let (keys, why) = ModifierKeys::of_names(&Err("no session".to_string()));
        assert_eq!(keys, ModifierKeys::physical());
        assert_eq!(why.as_deref(), Some("no session"));
    }

    #[test]
    fn the_watch_compiles_the_options_it_is_given() {
        if !crate::text::xkb::tests::xkb_ready() {
            return;
        }
        let names = |options: &str| {
            Ok(Names {
                layout: "us".into(),
                options: options.into(),
                ..Names::default()
            })
        };
        let mut watch = KeymapWatch::new();
        assert_eq!(
            watch.changed_to(names("ctrl:swap_lwin_lctl,ctrl:swap_rwin_rctl")),
            Some(swapped())
        );
        assert_eq!(
            watch.changed_to(names("ctrl:swap_lwin_lctl,ctrl:swap_rwin_rctl")),
            None
        );
        let swapcaps = watch.changed_to(names("ctrl:swapcaps")).unwrap();
        assert_eq!(
            swapcaps.held([KeyCode::KEY_CAPSLOCK]),
            Some(chord("Ctrl+A").modifiers)
        );
        assert_eq!(
            swapcaps.held([KeyCode::KEY_LEFTCTRL]),
            Some(Modifiers::default()),
            "Left Ctrl types Caps Lock"
        );
        let alt_win = watch.changed_to(names("altwin:swap_alt_win")).unwrap();
        assert_eq!(
            alt_win.held([KeyCode::KEY_LEFTMETA]),
            Some(chord("Alt+A").modifiers)
        );
        assert!(alt_win.only_alt_or_super(&chord("RightAlt")));
    }
}
