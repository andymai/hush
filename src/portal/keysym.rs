//! Text as key presses for the RemoteDesktop portal.
//!
//! A character goes as a keysym, and the compositor finds its key in the
//! active layout. KWin and Mutter drop a keysym that the layout lacks, with
//! no error to the sender, so Hush types only the characters that the
//! layout surely has, on its first two levels, and pastes other text. A key
//! that has the same place on every layout, such as Enter, Tab, Backspace,
//! Shift, or Insert, goes as a key code, because a keysym can be missing:
//! no Latin `v` is on an active Russian or Greek layout.
//!
//! For a character on the second level of its key, Hush holds Shift as a
//! key code around the keysym of the first level. KWin does not press
//! Shift correctly for a second-level keysym: the character comes out on
//! the first level, and Shift stays down for each key after it, also on the
//! user's keyboard.

use crate::text::keymap::KeyTable;
use crate::text::uinput_keyboard::LineBreak;
use input_linux::Key;
use std::sync::Arc;
use std::time::Duration;

/// One event of a RemoteDesktop session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyStep {
    /// A character key, by keysym.
    SymPress(u32),
    SymRelease(u32),
    /// A key with one place on every layout, by evdev key code.
    CodePress(u16),
    CodeRelease(u16),
    Pause(Duration),
}

pub const ENTER: u16 = Key::Enter as u16;
pub const TAB: u16 = Key::Tab as u16;
pub const BACKSPACE: u16 = Key::Backspace as u16;
pub const LEFT_SHIFT: u16 = Key::LeftShift as u16;
pub const INSERT: u16 = Key::Insert as u16;

/// The keysym of a character, as `xkb_utf32_to_keysym` gives it, or
/// `None` for a control character.
pub fn keysym(ch: char) -> Option<u32> {
    if ch.is_control() {
        return None;
    }
    let sym = xkeysym::Keysym::from_char(ch).raw();
    (sym != xkeysym::key::NoSymbol).then_some(sym)
}

/// The keys for one character: a keysym, with a key that holds Shift
/// around it for a character on the second level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymTap {
    pub sym: u32,
    pub shift: Option<u16>,
}

impl SymTap {
    pub const fn plain(sym: u32) -> Self {
        Self { sym, shift: None }
    }
}

/// The characters that type through the portal; the rest paste.
///
/// With Caps Lock on, a letter comes out in the other case, with the held
/// Shift or without it, unless the table was built with the lock. Portal
/// mode builds its tables with no lock, because it has no keyboard of its
/// own whose Caps Lock LED it can read.
#[derive(Debug, Clone)]
pub enum Typeable {
    /// For a layout whose keymap Hush cannot read: letters and space, which
    /// every Latin layout has, with each capital letter on the key of its
    /// small letter. When the compositor presses Shift for a keysym itself,
    /// also digits and `. , ; : ! ? - ( )`, which every Latin layout has on
    /// its first two levels, with no dead key. `^`, `` ` ``, `'`, `"`, and
    /// `~` are dead keys or third-level keys on German and US International.
    Conservative {
        /// Mutter presses Shift for a second-level keysym, and KWin does
        /// not. A digit or a punctuation mark is on the second level on
        /// some layouts, such as `1` on French, so without the keymap Hush
        /// cannot hold Shift for it.
        compositor_shifts: bool,
    },
    /// The characters on the first two levels of the active layout's keymap,
    /// with no dead key.
    Keymap(Arc<KeyTable>),
}

/// Whether a desktop's compositor presses Shift for a second-level keysym.
/// `desktop` is `XDG_CURRENT_DESKTOP`.
fn compositor_shifts(desktop: &str) -> bool {
    !desktop
        .split(':')
        .any(|name| name.eq_ignore_ascii_case("kde"))
}

impl Typeable {
    /// The conservative set for the session's desktop.
    pub fn conservative() -> Self {
        let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        Self::Conservative {
            compositor_shifts: compositor_shifts(&desktop),
        }
    }

    /// The keys for a character that types, or `None` when the take
    /// pastes.
    pub fn tap(&self, ch: char) -> Option<SymTap> {
        match self {
            Self::Conservative { compositor_shifts } => {
                if ch.is_ascii_uppercase() {
                    return Some(SymTap {
                        sym: keysym(ch.to_ascii_lowercase())?,
                        shift: Some(LEFT_SHIFT),
                    });
                }
                let allowed = ch.is_ascii_lowercase()
                    || ch == ' '
                    || (*compositor_shifts
                        && (ch.is_ascii_digit()
                            || matches!(ch, '.' | ',' | ';' | ':' | '!' | '?' | '-' | '(' | ')')));
                allowed.then(|| keysym(ch).map(SymTap::plain)).flatten()
            },
            Self::Keymap(table) => match table.taps(ch)? {
                [tap] if tap.level3.is_none() => Some(SymTap {
                    sym: table.first_level_sym(tap.key)?,
                    shift: tap.shift.map(|shift| shift.code()),
                }),
                _ => None,
            },
        }
    }

    pub fn allows(&self, ch: char) -> bool {
        self.tap(ch).is_some()
    }

    /// Whether every character of the text types. Line breaks and Tab are
    /// key codes, which every layout has.
    pub fn covers(&self, text: &str) -> bool {
        text.chars()
            .all(|ch| matches!(ch, '\n' | '\r' | '\t') || self.allows(ch))
    }
}

fn tap_sym(steps: &mut Vec<KeyStep>, tap: SymTap) {
    steps.extend(tap.shift.map(KeyStep::CodePress));
    steps.push(KeyStep::SymPress(tap.sym));
    steps.push(KeyStep::SymRelease(tap.sym));
    steps.extend(tap.shift.map(KeyStep::CodeRelease));
}

fn tap_code(steps: &mut Vec<KeyStep>, held: &[u16], code: u16) {
    steps.extend(held.iter().map(|&modifier| KeyStep::CodePress(modifier)));
    steps.push(KeyStep::CodePress(code));
    steps.push(KeyStep::CodeRelease(code));
    steps.extend(
        held.iter()
            .rev()
            .map(|&modifier| KeyStep::CodeRelease(modifier)),
    );
}

/// The steps that type a text, with `delay` between characters. `\r\n`,
/// `\r`, and `\n` are each one line break. A character that `typeable`
/// does not allow goes as its plain keysym, for an explicit choice to type
/// and for a failed paste. A character with no keysym is skipped.
pub fn text_steps(
    text: &str,
    typeable: &Typeable,
    line_break: LineBreak,
    delay: Duration,
) -> Vec<KeyStep> {
    let mut steps = Vec::with_capacity(text.len() * 3);
    let mut chars = text.chars().peekable();
    let line = |steps: &mut Vec<KeyStep>| match line_break {
        LineBreak::Enter => tap_code(steps, &[], ENTER),
        LineBreak::ShiftEnter => tap_code(steps, &[LEFT_SHIFT], ENTER),
    };
    while let Some(ch) = chars.next() {
        if !steps.is_empty() && !delay.is_zero() {
            steps.push(KeyStep::Pause(delay));
        }
        match ch {
            '\r' => {
                chars.next_if_eq(&'\n');
                line(&mut steps);
            },
            '\n' => line(&mut steps),
            '\t' => tap_code(&mut steps, &[], TAB),
            _ => match typeable.tap(ch).or_else(|| keysym(ch).map(SymTap::plain)) {
                Some(tap) => tap_sym(&mut steps, tap),
                None => {
                    if matches!(steps.last(), Some(KeyStep::Pause(_))) {
                        steps.pop();
                    }
                },
            },
        }
    }
    steps
}

/// The paste chord of portal mode: Shift+Insert as key codes, which every
/// layout has in one place. Ctrl+V as a keysym vanishes on a layout without
/// a Latin `v`, and Ctrl+V as a key code is Ctrl+K on Dvorak.
pub fn paste_steps() -> Vec<KeyStep> {
    let mut steps = Vec::with_capacity(4);
    tap_code(&mut steps, &[LEFT_SHIFT], INSERT);
    steps
}

/// Enter after `delay`, so the application handles the text first.
pub fn enter_steps(delay: Duration) -> Vec<KeyStep> {
    let mut steps = vec![KeyStep::Pause(delay)];
    tap_code(&mut steps, &[], ENTER);
    steps
}

pub fn backspace_steps(count: usize, delay: Duration) -> Vec<KeyStep> {
    let mut steps = Vec::with_capacity(count * 3);
    for i in 0..count {
        if i > 0 && !delay.is_zero() {
            steps.push(KeyStep::Pause(delay));
        }
        tap_code(&mut steps, &[], BACKSPACE);
    }
    steps
}

#[cfg(test)]
mod tests {
    use super::*;
    use KeyStep::{CodePress, CodeRelease, Pause, SymPress, SymRelease};

    #[test]
    fn keysyms_match_xkb_utf32_to_keysym() {
        assert_eq!(keysym('a'), Some(0x61));
        assert_eq!(keysym('A'), Some(0x41));
        assert_eq!(keysym(' '), Some(0x20));
        assert_eq!(keysym('~'), Some(0x7e));
        assert_eq!(keysym('é'), Some(0xe9), "Latin-1 maps one to one");
        assert_eq!(keysym('ł'), Some(0x1b3), "a legacy keysym from the table");
        assert_eq!(keysym('€'), Some(0x20ac), "EuroSign");
        assert_eq!(
            keysym('👍'),
            Some(0x0100_0000 | 0x1f44d),
            "a Unicode keysym"
        );
        assert_eq!(keysym('\n'), None);
        assert_eq!(keysym('\u{7}'), None);
    }

    const GNOME: Typeable = Typeable::Conservative {
        compositor_shifts: true,
    };
    const KDE: Typeable = Typeable::Conservative {
        compositor_shifts: false,
    };

    fn us() -> Typeable {
        Typeable::Keymap(Arc::new(KeyTable::us().clone()))
    }

    fn steps(text: &str, typeable: &Typeable) -> Vec<KeyStep> {
        text_steps(text, typeable, LineBreak::Enter, Duration::ZERO)
    }

    fn shifted(sym: u32) -> [KeyStep; 4] {
        [
            CodePress(LEFT_SHIFT),
            SymPress(sym),
            SymRelease(sym),
            CodeRelease(LEFT_SHIFT),
        ]
    }

    #[test]
    fn without_the_keymap_hush_types_only_what_every_latin_layout_has() {
        assert!(GNOME.covers("Hello, world! Is it 42 (or 43)? Yes: no; well-known.\n\t"));
        for risky in ["^", "`", "'", "\"", "~", "@", "café", "Émile", "日本", "👍"] {
            assert!(
                !GNOME.covers(risky),
                "{risky} is a dead key, a third-level key, or not on the layout"
            );
        }
        assert!(GNOME.covers(""));
    }

    #[test]
    fn on_kde_without_the_keymap_only_letters_and_spaces_type() {
        assert!(KDE.covers("And so my fellow Americans\n\task not"));
        for text in [
            "1", "42", ".", ",", ";", ":", "!", "?", "-", "(", ")", "Hi, you",
        ] {
            assert!(
                !KDE.covers(text),
                "{text}: without the keymap, Hush cannot tell if it needs Shift"
            );
        }
        assert!(!KDE.covers("^"));
    }

    #[test]
    fn only_kwin_cannot_shift_a_keysym() {
        assert!(!compositor_shifts("KDE"));
        assert!(!compositor_shifts("kde"));
        assert!(!compositor_shifts("GNOME:KDE"));
        assert!(compositor_shifts("GNOME"));
        assert!(compositor_shifts("ubuntu:GNOME"));
        assert!(compositor_shifts("KDEish"), "only a whole name counts");
        assert!(compositor_shifts(""));
    }

    #[test]
    fn the_keymap_types_its_first_two_levels() {
        use crate::text::keymap::Tap;
        use std::collections::HashMap;
        let altgr = Tap {
            key: Key::Q,
            shift: None,
            level3: Some(Key::RightAlt),
        };
        let table = KeyTable::new(
            HashMap::from([
                ('a', vec![Tap::plain(Key::A)]),
                ('A', vec![Tap::shifted(Key::A)]),
                ('é', vec![Tap::plain(Key::Num2)]),
                ('2', vec![Tap::shifted(Key::Num2)]),
                ('z', vec![Tap::plain(Key::Z)]),
                ('Z', vec![Tap::shifted(Key::Z)]),
                ('@', vec![altgr]),
                ('ê', vec![Tap::plain(Key::LeftBrace), Tap::plain(Key::E)]),
            ]),
            Key::LeftShift,
        )
        .with_first_level(HashMap::from([
            (Key::A, 0x61),
            (Key::Num2, 0xe9),
            (Key::Q, 0x71),
        ]));
        let keymap = Typeable::Keymap(Arc::new(table));
        assert!(keymap.covers("aAé2\n"));
        assert!(!keymap.covers("@"), "a third-level key");
        assert!(!keymap.covers("ê"), "a dead-key sequence");
        assert!(!keymap.covers("b"), "not on the layout");
        assert!(
            !keymap.covers("z") && !keymap.covers("Z"),
            "the compositor cannot find the key from its keysym"
        );
    }

    #[test]
    fn a_first_level_character_is_a_plain_keysym() {
        for typeable in [&us(), &GNOME, &KDE] {
            assert_eq!(
                steps("a z", typeable),
                [
                    SymPress(0x61),
                    SymRelease(0x61),
                    SymPress(0x20),
                    SymRelease(0x20),
                    SymPress(0x7a),
                    SymRelease(0x7a)
                ],
                "{typeable:?}"
            );
        }
        assert_eq!(
            steps(",1'", &us()),
            [
                SymPress(0x2c),
                SymRelease(0x2c),
                SymPress(0x31),
                SymRelease(0x31),
                SymPress(0x27),
                SymRelease(0x27)
            ]
        );
    }

    #[test]
    fn a_second_level_character_holds_shift_around_the_first_level() {
        let us = us();
        assert_eq!(steps("A", &us), shifted(0x61));
        assert_eq!(steps("!", &us), shifted(0x31), "Shift and 1");
        assert_eq!(steps("?", &us), shifted(0x2f), "Shift and /");
        assert_eq!(steps(":", &us), shifted(0x3b), "Shift and ;");
        assert_eq!(steps("~", &us), shifted(0x60), "Shift and `");
        assert_eq!(
            [steps("A", &GNOME), steps("A", &KDE)],
            [shifted(0x61), shifted(0x61)],
            "a capital letter is on the key of its small letter"
        );
    }

    #[test]
    fn the_shift_key_comes_from_the_table() {
        use crate::text::keymap::Tap;
        use std::collections::HashMap;
        let right = Tap {
            key: Key::A,
            shift: Some(Key::RightShift),
            level3: None,
        };
        let table = KeyTable::new(HashMap::from([('A', vec![right])]), Key::RightShift)
            .with_first_level(HashMap::from([(Key::A, 0x61)]));
        let right_shift = Key::RightShift as u16;
        assert_eq!(
            steps("A", &Typeable::Keymap(Arc::new(table))),
            [
                CodePress(right_shift),
                SymPress(0x61),
                SymRelease(0x61),
                CodeRelease(right_shift)
            ]
        );
    }

    #[test]
    fn without_the_keymap_the_compositor_shifts_digits_and_punctuation() {
        assert_eq!(
            steps("1!", &GNOME),
            [
                SymPress(0x31),
                SymRelease(0x31),
                SymPress(0x21),
                SymRelease(0x21)
            ],
            "Mutter presses Shift for ! itself"
        );
    }

    #[test]
    fn a_character_outside_the_set_goes_as_its_keysym() {
        let exclam = [SymPress(0x21), SymRelease(0x21)];
        assert_eq!(steps("!", &KDE), exclam, "an explicit choice to type");
        assert_eq!(steps("é", &GNOME), [SymPress(0xe9), SymRelease(0xe9)]);
        let no_first_level = Typeable::Keymap(Arc::new(KeyTable::new(
            std::collections::HashMap::new(),
            Key::LeftShift,
        )));
        assert_eq!(steps("!", &no_first_level), exclam);
    }

    #[test]
    fn a_keymap_text_types_through_its_first_levels() {
        use crate::text::xkb::{self, tests::small_keymap, tests::SMALL_US};
        if !xkb::available() {
            eprintln!("Skipping the keymap text test: libxkbcommon does not load");
            return;
        }
        let table = xkb::build_from_text(&small_keymap(SMALL_US)).expect("the keymap compiles");
        let keymap = Typeable::Keymap(Arc::new(table));
        let mut expected = vec![SymPress(0x61), SymRelease(0x61)];
        expected.extend(shifted(0x31));
        expected.extend(shifted(0x2c));
        expected.extend(shifted(0x61));
        assert_eq!(steps("a!<A", &keymap), expected, "< is Shift and comma");
        assert!(keymap.covers("a, A! <"));
        assert!(
            !keymap.covers(">"),
            "only the ISO key has >, and its first level < is also on the comma key"
        );
        assert!(!keymap.covers("b"));
    }

    #[test]
    fn a_table_with_caps_lock_gives_each_letter_its_case() {
        use crate::text::xkb::{self, tests::names, tests::xkb_ready, Locked, Request};
        if !xkb_ready() {
            return;
        }
        let table = xkb::build(&Request {
            names: names("us", ""),
            group: 0,
            level3: true,
            reserved: Vec::new(),
            locale: "en_US.UTF-8".into(),
            locked: Locked::Named(xkb::LOCK_MOD),
        })
        .expect("the keymap compiles");
        let keymap = Typeable::Keymap(Arc::new(table));
        assert_eq!(
            steps("A", &keymap),
            [SymPress(0x61), SymRelease(0x61)],
            "Caps Lock gives the capital"
        );
        assert_eq!(
            steps("a", &keymap),
            shifted(0x61),
            "Shift with Caps Lock gives the small letter"
        );
    }

    #[test]
    fn text_becomes_keysyms_and_line_breaks_key_codes() {
        let delay = Duration::from_millis(20);
        let mut expected = vec![SymPress(0x61), SymRelease(0x61), Pause(delay)];
        expected.extend(shifted(0x62));
        assert_eq!(
            text_steps("aB", &GNOME, LineBreak::Enter, delay),
            expected,
            "B is Shift and b"
        );
        assert_eq!(
            text_steps("a\r\nb", &GNOME, LineBreak::Enter, Duration::ZERO),
            [
                SymPress(0x61),
                SymRelease(0x61),
                CodePress(ENTER),
                CodeRelease(ENTER),
                SymPress(0x62),
                SymRelease(0x62)
            ],
            "\\r\\n is one line break"
        );
        assert_eq!(
            text_steps("\n\t", &KDE, LineBreak::ShiftEnter, Duration::ZERO),
            [
                CodePress(LEFT_SHIFT),
                CodePress(ENTER),
                CodeRelease(ENTER),
                CodeRelease(LEFT_SHIFT),
                CodePress(TAB),
                CodeRelease(TAB)
            ],
            "a chat line break is Shift+Enter"
        );
        assert_eq!(
            text_steps("a\u{7}b", &GNOME, LineBreak::Enter, delay),
            [
                SymPress(0x61),
                SymRelease(0x61),
                Pause(delay),
                SymPress(0x62),
                SymRelease(0x62)
            ],
            "a control character is skipped with its pause"
        );
    }

    #[test]
    fn the_paste_enter_and_backspace_are_key_codes() {
        assert_eq!(
            paste_steps(),
            [
                CodePress(LEFT_SHIFT),
                CodePress(INSERT),
                CodeRelease(INSERT),
                CodeRelease(LEFT_SHIFT)
            ],
            "Shift+Insert has one place on every layout"
        );
        assert_eq!((LEFT_SHIFT, INSERT, ENTER), (42, 110, 28), "evdev codes");
        let delay = Duration::from_millis(150);
        assert_eq!(
            enter_steps(delay),
            [Pause(delay), CodePress(ENTER), CodeRelease(ENTER)]
        );
        assert_eq!(
            backspace_steps(2, Duration::ZERO),
            [
                CodePress(BACKSPACE),
                CodeRelease(BACKSPACE),
                CodePress(BACKSPACE),
                CodeRelease(BACKSPACE)
            ]
        );
    }
}
