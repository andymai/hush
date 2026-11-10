//! Checks on the chords under `[hotkey]`, language, style, and rewrite
//! chords included: two actions on one chord, a key that types AltGr, and
//! keys that type a character. Each check is a pure function, so the
//! settings window, the config load, and `hush doctor` give the same answer.

use super::keymap::ModifierKeys;
use super::KeyCombination;
use crate::text::layout::Layout;
use crate::text_processing::StyleName;
use crate::transcription::Language;
use evdev::KeyCode;
use std::fmt;

/// The Hush actions that a chord starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Dictate,
    Cancel,
    Command,
    PasteLast,
    Learn,
    /// A language chord: dictation in that language
    Language(Language),
    /// A style chord: dictation in that style
    Style(StyleName),
    /// A rewrite chord: the selected text, rewritten in that style
    Rewrite(StyleName),
}

impl Role {
    /// The actions with a key of their own under `[hotkey]`.
    pub const ALL: [Role; 5] = [
        Role::Dictate,
        Role::Cancel,
        Role::Command,
        Role::PasteLast,
        Role::Learn,
    ];

    /// The name the settings window shows.
    pub fn label(self) -> String {
        match self {
            Role::Dictate => "Dictate".to_string(),
            Role::Cancel => "Cancel".to_string(),
            Role::Command => "Command".to_string(),
            Role::PasteLast => "Type last again".to_string(),
            Role::Learn => "Learn selection".to_string(),
            Role::Language(language) => format!("Dictate in {}", language.name()),
            Role::Style(style) => format!("Dictate in the {} style", style),
            Role::Rewrite(style) => format!("Rewrite in the {} style", style),
        }
    }

    /// The key under `[hotkey]` that holds the chord.
    pub fn config_key(self) -> &'static str {
        match self {
            Role::Dictate => "combination",
            Role::Cancel => "cancel",
            Role::Command => "command",
            Role::PasteLast => "paste_last",
            Role::Learn => "learn",
            Role::Language(_) => "languages",
            Role::Style(_) => "styles",
            Role::Rewrite(_) => "rewrite",
        }
    }

    /// The chord starts a dictation: the dictation key, a language chord, or
    /// a style chord.
    pub fn dictates(self) -> bool {
        matches!(self, Role::Dictate | Role::Language(_) | Role::Style(_))
    }
}

/// One chord that two or more actions use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub chord: KeyCombination,
    pub roles: Vec<Role>,
}

impl fmt::Display for Conflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} use {}", labels(&self.roles), self.chord)
    }
}

/// A key that types AltGr as the main key of a chord.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AltGr {
    /// The layout as the session names it, when known.
    pub layout: Option<String>,
    /// The key that types AltGr.
    pub key: KeyCombination,
    pub roles: Vec<Role>,
    /// A key that types Ctrl, to use alone in place of `key`.
    pub instead: Option<KeyCombination>,
    /// Whether the keys come from the session keymap, not from their names.
    pub keymap: bool,
}

impl AltGr {
    /// What to do, as a clause: use a key that types Ctrl in place of the
    /// AltGr key. The advice never names a key that types Super.
    pub fn advice(&self) -> String {
        match &self.instead {
            Some(instead)
                if matches!(
                    instead.evdev_key(),
                    KeyCode::KEY_LEFTCTRL | KeyCode::KEY_RIGHTCTRL
                ) =>
            {
                let caveat = if self.keymap {
                    String::new()
                } else {
                    format!(", if {} types Ctrl on your layout", instead)
                };
                format!("use {} in place of {}{}", instead, self.key, caveat)
            },
            Some(instead) => format!(
                "use {}, which types Ctrl on this layout, in place of {}",
                instead, self.key
            ),
            None => format!(
                "use a key that types no character, such as F13, in place of {}",
                self.key
            ),
        }
    }
}

impl fmt::Display for AltGr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let target = if self.roles.iter().any(|role| role.dictates()) {
            "a dictation".to_string()
        } else {
            labels(&self.roles)
        };
        let layout = match &self.layout {
            Some(layout) => format!("the {} layout", layout),
            None => "this layout".to_string(),
        };
        write!(
            f,
            "On {}, {} is AltGr: it types special characters, and each one can also start {}",
            layout, self.key, target
        )
    }
}

/// Each bound action with its parsed chord. An empty value is unbound, and
/// a value that does not parse gets its own error elsewhere.
fn parsed<'a>(bound: impl IntoIterator<Item = (Role, &'a str)>) -> Vec<(Role, KeyCombination)> {
    bound
        .into_iter()
        .filter_map(|(role, text)| Some((role, KeyCombination::parse(text).ok()?)))
        .collect()
}

/// Every chord that more than one action uses, in the order of `bound`.
/// Chords compare as parsed keys, so `ctrl+RightAlt` equals `Ctrl+RightAlt`.
pub fn conflicts<'a>(bound: impl IntoIterator<Item = (Role, &'a str)>) -> Vec<Conflict> {
    let mut groups: Vec<Conflict> = Vec::new();
    for (role, chord) in parsed(bound) {
        match groups.iter_mut().find(|group| group.chord == chord) {
            Some(group) => group.roles.push(role),
            None => groups.push(Conflict {
                chord,
                roles: vec![role],
            }),
        }
    }
    groups.retain(|group| group.roles.len() > 1);
    groups
}

/// The actions other than `role` that already use `chord`.
pub fn holders<'a>(
    bound: impl IntoIterator<Item = (Role, &'a str)>,
    role: Role,
    chord: &str,
) -> Vec<Role> {
    users(bound, chord)
        .into_iter()
        .filter(|other| *other != role)
        .collect()
}

/// Every action in `bound` that uses `chord`.
pub fn users<'a>(bound: impl IntoIterator<Item = (Role, &'a str)>, chord: &str) -> Vec<Role> {
    let Ok(chord) = KeyCombination::parse(chord) else {
        return Vec::new();
    };
    parsed(bound)
        .into_iter()
        .filter(|(_, used)| *used == chord)
        .map(|(role, _)| role)
        .collect()
}

/// The actions whose main key types AltGr in the session keymap. Without
/// the keymap, Right Alt is AltGr on every layout but plain US, and an
/// unknown layout counts as US.
pub fn altgr<'a>(
    bound: impl IntoIterator<Item = (Role, &'a str)>,
    layout: Option<&Layout>,
    keys: &ModifierKeys,
) -> Option<AltGr> {
    let altgr = keys.altgr_keys(layout);
    let found: Vec<(Role, KeyCombination)> = parsed(bound)
        .into_iter()
        .filter(|(_, chord)| altgr.contains(&chord.evdev_key()))
        .collect();
    let (_, first) = found.first()?;
    let bare = |key| KeyCombination::from_evdev(key, Default::default());
    Some(AltGr {
        layout: layout.map(ToString::to_string),
        key: bare(first.evdev_key())?,
        roles: found.iter().map(|(role, _)| *role).collect(),
        instead: keys.ctrl_key().and_then(bare),
        keymap: keys.from_keymap(),
    })
}

/// The chords whose main key types a character and so also reaches the
/// focused app. `exclusive` means applications never see the chords.
pub fn printable<'a>(
    bound: impl IntoIterator<Item = (Role, &'a str)>,
    exclusive: bool,
) -> Vec<(Role, KeyCombination)> {
    if exclusive {
        return Vec::new();
    }
    parsed(bound)
        .into_iter()
        .filter(|(_, chord)| chord.types_character())
        .collect()
}

/// Action names as an English list: `Dictate and Cancel`.
pub fn labels(roles: &[Role]) -> String {
    let names: Vec<String> = roles.iter().map(|role| role.label()).collect();
    join(&names)
}

/// Items as an English list: `a`, `a and b`, `a, b, and c`.
pub fn join(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [first, second] => format!("{} and {}", first, second),
        [rest @ .., last] => format!("{}, and {}", rest.join(", "), last),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xkb(layout: &str, variant: &str) -> Layout {
        Layout::Xkb {
            layout: layout.to_string(),
            variant: variant.to_string(),
        }
    }

    fn defaults() -> [(Role, &'static str); 5] {
        [
            (Role::Dictate, "RightAlt"),
            (Role::Cancel, "Escape"),
            (Role::Command, "Ctrl+RightAlt"),
            (Role::PasteLast, ""),
            (Role::Learn, ""),
        ]
    }

    #[test]
    fn the_defaults_have_no_conflict() {
        assert!(conflicts(defaults()).is_empty());
    }

    #[test]
    fn chords_compare_after_parsing() {
        let found = conflicts([
            (Role::Dictate, "RightAlt"),
            (Role::Command, "Ctrl+RightAlt"),
            (Role::Learn, " ctrl + rightalt "),
        ]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].roles, vec![Role::Command, Role::Learn]);
        assert_eq!(found[0].chord.to_string(), "Ctrl+RightAlt");
        assert_eq!(
            found[0].to_string(),
            "Command and Learn selection use Ctrl+RightAlt"
        );
        assert_eq!(
            conflicts([(Role::Dictate, "AltGr"), (Role::Cancel, "RightAlt")])[0].roles,
            vec![Role::Dictate, Role::Cancel],
            "an alias is the same key"
        );
    }

    #[test]
    fn unbound_and_unparseable_values_never_conflict() {
        assert!(conflicts([
            (Role::Dictate, "F12"),
            (Role::Cancel, ""),
            (Role::PasteLast, ""),
            (Role::Learn, "   "),
        ])
        .is_empty());
        assert!(conflicts([(Role::PasteLast, "Ctrl+Nope"), (Role::Learn, "Ctrl+Nope")]).is_empty());
    }

    #[test]
    fn several_conflicts_are_each_reported() {
        let found = conflicts([
            (Role::Dictate, "F12"),
            (Role::Cancel, "Escape"),
            (Role::Command, "f12"),
            (Role::PasteLast, "escape"),
            (Role::Learn, "Esc"),
        ]);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].roles, vec![Role::Dictate, Role::Command]);
        assert_eq!(
            found[1].roles,
            vec![Role::Cancel, Role::PasteLast, Role::Learn]
        );
        assert_eq!(
            found[1].to_string(),
            "Cancel, Type last again, and Learn selection use Escape"
        );
    }

    #[test]
    fn holders_are_the_other_actions_on_a_chord() {
        assert_eq!(
            holders(defaults(), Role::Learn, "ctrl+rightalt"),
            vec![Role::Command]
        );
        assert!(
            holders(defaults(), Role::Command, "Ctrl+RightAlt").is_empty(),
            "an action never conflicts with itself"
        );
        assert!(holders(defaults(), Role::Learn, "").is_empty());
        assert!(holders(defaults(), Role::Learn, "Shift+RightAlt").is_empty());
    }

    #[test]
    fn right_alt_warns_on_every_layout_but_plain_us() {
        let physical = ModifierKeys::physical();
        assert_eq!(altgr(defaults(), Some(&xkb("us", "")), &physical), None);
        assert_eq!(
            altgr(defaults(), None, &physical),
            None,
            "an unknown layout counts as US"
        );
        for layout in [xkb("us(intl)", ""), xkb("de", ""), xkb("gb", "")] {
            let warning = altgr(defaults(), Some(&layout), &physical).expect("AltGr layout");
            assert_eq!(warning.roles, vec![Role::Dictate, Role::Command]);
        }
        let german = altgr(defaults(), Some(&xkb("de", "")), &physical).unwrap();
        assert_eq!(
            german.to_string(),
            "On the de layout, RightAlt is AltGr: it types special characters, and each one can also start a dictation"
        );
        assert_eq!(
            german.advice(),
            "use RightCtrl in place of RightAlt, if RightCtrl types Ctrl on your layout",
            "without the keymap, Hush cannot know what Right Ctrl types"
        );
    }

    #[test]
    fn the_keymap_names_the_altgr_key_and_a_key_that_types_ctrl() {
        use crate::hotkey::keymap::tests::{german as german_keys, swapped};
        assert_eq!(
            altgr(defaults(), Some(&xkb("de", "")), &swapped()),
            None,
            "the keymap types Alt_R on Right Alt, whatever the layout name"
        );
        let warning = altgr(defaults(), None, &german_keys()).unwrap();
        assert!(warning
            .to_string()
            .starts_with("On this layout, RightAlt is AltGr"));
        assert_eq!(warning.advice(), "use RightCtrl in place of RightAlt");

        let moved = ModifierKeys::from_classes(&crate::text::xkb::KeyClasses {
            control: vec![input_linux::Key::LeftCtrl, input_linux::Key::RightMeta],
            super_keys: vec![input_linux::Key::RightCtrl, input_linux::Key::LeftMeta],
            altgr: vec![input_linux::Key::RightAlt],
            ..Default::default()
        });
        assert_eq!(
            altgr(defaults(), Some(&xkb("de", "")), &moved)
                .unwrap()
                .advice(),
            "use RightSuper, which types Ctrl on this layout, in place of RightAlt",
            "Right Ctrl types Super with ctrl:swap_rwin_rctl"
        );

        let caps = ModifierKeys::from_classes(&crate::text::xkb::KeyClasses {
            control: vec![input_linux::Key::LeftCtrl, input_linux::Key::RightCtrl],
            altgr: vec![input_linux::Key::CapsLock],
            ..Default::default()
        });
        assert_eq!(
            altgr(defaults(), Some(&xkb("us", "")), &caps),
            None,
            "with lv3:caps_switch, Right Alt still types Alt"
        );
        let warning = altgr([(Role::Dictate, "CapsLock")], None, &caps).unwrap();
        assert_eq!(warning.key.to_string(), "Capslock");
    }

    #[test]
    fn altgr_needs_right_alt_as_the_main_key() {
        let physical = ModifierKeys::physical();
        let bound = [(Role::Dictate, "RightCtrl"), (Role::Command, "Alt+Space")];
        assert_eq!(altgr(bound, Some(&xkb("de", "")), &physical), None);
        let learn_only = altgr(
            [(Role::Dictate, "F13"), (Role::Learn, "Super+AltGr")],
            Some(&xkb("de", "")),
            &physical,
        )
        .unwrap();
        assert_eq!(learn_only.roles, vec![Role::Learn]);
        assert!(learn_only
            .to_string()
            .ends_with("can also start Learn selection"));
    }

    #[test]
    fn printable_keys_warn_unless_exclusive() {
        for chord in [
            "a",
            "Shift+Z",
            "7",
            ",",
            "Ctrl+/",
            "`",
            "Space",
            "Ctrl+Shift+Space",
            "Enter",
        ] {
            let found = printable([(Role::Dictate, chord)], false);
            assert_eq!(found.len(), 1, "{chord} types a character");
            assert!(
                printable([(Role::Dictate, chord)], true).is_empty(),
                "{chord} is hidden when exclusive"
            );
        }
        for chord in [
            "F13",
            "F1",
            "Mouse4",
            "Ctrl+Mouse5",
            "RightAlt",
            "LeftShift",
            "RightCtrl",
            "Escape",
            "Tab",
            "Up",
            "",
        ] {
            assert!(
                printable([(Role::Dictate, chord)], false).is_empty(),
                "{chord} types nothing"
            );
        }
    }

    fn german() -> Role {
        Role::Language(Language::parse("de").unwrap())
    }

    #[test]
    fn language_chords_conflict_with_actions_and_with_each_other() {
        let french = Role::Language(Language::parse("fr").unwrap());
        let mut bound = defaults().to_vec();
        bound.push((german(), "Shift+RightAlt"));
        bound.push((french, "F13"));
        assert!(conflicts(bound.clone()).is_empty());

        bound.push((Role::PasteLast, "shift+rightalt"));
        let found = conflicts(bound.clone());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].roles, vec![german(), Role::PasteLast]);
        assert_eq!(
            found[0].to_string(),
            "Dictate in German and Type last again use Shift+RightAlt"
        );

        let found = conflicts([(german(), "F13"), (french, "f13")]);
        assert_eq!(found.len(), 1, "two language chords must differ");
        assert_eq!(found[0].roles, vec![german(), french]);
        assert_eq!(
            conflicts([(Role::Dictate, "RightAlt"), (german(), "AltGr")])[0].roles,
            vec![Role::Dictate, german()]
        );

        assert_eq!(
            holders(bound.clone(), Role::Learn, "Shift+RightAlt"),
            vec![german(), Role::PasteLast]
        );
        assert_eq!(users(bound, "F13"), vec![french]);
    }

    #[test]
    fn language_chords_get_the_altgr_and_typing_warnings() {
        let warning = altgr(
            [(Role::Dictate, "F13"), (german(), "Shift+RightAlt")],
            Some(&xkb("de", "")),
            &ModifierKeys::physical(),
        )
        .unwrap();
        assert_eq!(warning.roles, vec![german()]);
        assert!(
            warning.to_string().ends_with("can also start a dictation"),
            "{warning}"
        );
        let typing = printable([(Role::Dictate, "RightAlt"), (german(), "Ctrl+G")], false);
        assert_eq!(typing.len(), 1);
        assert_eq!(typing[0].0, german());
        assert_eq!(german().label(), "Dictate in German");
        assert_eq!(german().config_key(), "languages");
        assert!(german().dictates() && Role::Dictate.dictates());
        assert!(!Role::Command.dictates());
    }

    #[test]
    fn style_chords_conflict_with_every_other_chord() {
        let verbatim = Role::Style(crate::text_processing::styles::VERBATIM);
        let casual = Role::Style(StyleName::parse("casual").unwrap());
        let mut bound = defaults().to_vec();
        bound.push((german(), "Shift+RightAlt"));
        bound.push((verbatim, "Super+RightAlt"));
        bound.push((casual, "F14"));
        assert!(conflicts(bound.clone()).is_empty());

        bound.push((Role::Learn, "super+rightalt"));
        bound.push((german(), "f14"));
        let found = conflicts(bound.clone());
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].roles, vec![verbatim, Role::Learn]);
        assert_eq!(
            found[0].to_string(),
            "Dictate in the verbatim style and Learn selection use Super+RightAlt"
        );
        assert_eq!(found[1].roles, vec![casual, german()]);
        assert_eq!(
            conflicts([(Role::Command, "Ctrl+RightAlt"), (casual, "ctrl+rightalt")])[0].roles,
            vec![Role::Command, casual]
        );
        assert_eq!(verbatim.config_key(), "styles");
        assert!(verbatim.dictates());
        let typing = printable([(Role::Dictate, "RightAlt"), (casual, "Ctrl+G")], false);
        assert_eq!(typing[0].0, casual);
    }

    #[test]
    fn rewrite_chords_conflict_with_every_other_chord() {
        let formal = StyleName::parse("formal").unwrap();
        let rewrite = Role::Rewrite(formal);
        let mut bound = defaults().to_vec();
        bound.push((Role::Style(formal), "Super+RightAlt"));
        bound.push((rewrite, "Super+F"));
        assert!(conflicts(bound.clone()).is_empty());
        bound.push((german(), "super+f"));
        let found = conflicts(bound);
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].to_string(),
            "Rewrite in the formal style and Dictate in German use Super+F"
        );
        assert_eq!(
            conflicts([(rewrite, "F14"), (Role::Style(formal), "f14")])[0].roles,
            vec![rewrite, Role::Style(formal)],
            "a rewrite chord and a style chord of one style still differ"
        );
        assert_eq!(rewrite.config_key(), "rewrite");
        assert!(!rewrite.dictates());
        let typing = printable([(Role::Dictate, "RightAlt"), (rewrite, "Ctrl+G")], false);
        assert_eq!(typing[0].0, rewrite);
    }

    #[test]
    fn lists_read_as_english() {
        assert_eq!(join(&[]), "");
        assert_eq!(join(&["a".to_string()]), "a");
        assert_eq!(join(&["a".to_string(), "b".to_string()]), "a and b");
        assert_eq!(
            join(&["a".to_string(), "b".to_string(), "c".to_string()]),
            "a, b, and c"
        );
    }
}
