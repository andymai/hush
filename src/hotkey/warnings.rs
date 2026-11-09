//! Checks on the chords under `[hotkey]`: two actions on one chord, Right Alt
//! on a layout where it is AltGr, and keys that type a character. Each check
//! is a pure function, so the settings window, the config load, and
//! `hush doctor` give the same answer.

use super::KeyCombination;
use crate::text::layout::Layout;
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
}

impl Role {
    pub const ALL: [Role; 5] = [
        Role::Dictate,
        Role::Cancel,
        Role::Command,
        Role::PasteLast,
        Role::Learn,
    ];

    /// The name the settings window shows.
    pub fn label(self) -> &'static str {
        match self {
            Role::Dictate => "Dictate",
            Role::Cancel => "Cancel",
            Role::Command => "Command",
            Role::PasteLast => "Type last again",
            Role::Learn => "Learn selection",
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
        }
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

/// Right Alt as the main key of a chord, on a layout where Right Alt is AltGr.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AltGr {
    pub layout: String,
    pub roles: Vec<Role>,
}

impl fmt::Display for AltGr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let target = if self.roles.contains(&Role::Dictate) {
            "a dictation".to_string()
        } else {
            labels(&self.roles)
        };
        write!(
            f,
            "On the {} layout, Right Alt is AltGr: it types special characters, and each one can also start {}",
            self.layout, target
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
    let Ok(chord) = KeyCombination::parse(chord) else {
        return Vec::new();
    };
    parsed(bound)
        .into_iter()
        .filter(|(other, used)| *other != role && *used == chord)
        .map(|(other, _)| other)
        .collect()
}

/// The actions whose main key is Right Alt, when the layout uses Right Alt
/// as AltGr. Only the plain US layout has no AltGr, and an unknown layout
/// counts as US.
pub fn altgr<'a>(
    bound: impl IntoIterator<Item = (Role, &'a str)>,
    layout: Option<&Layout>,
) -> Option<AltGr> {
    let layout = layout.filter(|layout| !layout.types_directly())?;
    let roles: Vec<Role> = parsed(bound)
        .into_iter()
        .filter(|(_, chord)| chord.evdev_key() == KeyCode::KEY_RIGHTALT)
        .map(|(role, _)| role)
        .collect();
    (!roles.is_empty()).then(|| AltGr {
        layout: layout.to_string(),
        roles,
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
    let names: Vec<String> = roles.iter().map(|role| role.label().to_string()).collect();
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
        assert_eq!(altgr(defaults(), Some(&xkb("us", ""))), None);
        assert_eq!(
            altgr(defaults(), None),
            None,
            "an unknown layout counts as US"
        );
        for layout in [xkb("us(intl)", ""), xkb("de", ""), xkb("gb", "")] {
            let warning = altgr(defaults(), Some(&layout)).expect("AltGr layout");
            assert_eq!(warning.roles, vec![Role::Dictate, Role::Command]);
        }
        let german = altgr(defaults(), Some(&xkb("de", ""))).unwrap();
        assert_eq!(
            german.to_string(),
            "On the de layout, Right Alt is AltGr: it types special characters, and each one can also start a dictation"
        );
    }

    #[test]
    fn altgr_needs_right_alt_as_the_main_key() {
        let bound = [(Role::Dictate, "RightCtrl"), (Role::Command, "Alt+Space")];
        assert_eq!(altgr(bound, Some(&xkb("de", ""))), None);
        let learn_only = altgr(
            [(Role::Dictate, "F13"), (Role::Learn, "Super+AltGr")],
            Some(&xkb("de", "")),
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
