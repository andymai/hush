//! The keys that type each character: the built-in US table, or a table
//! that [`super::xkb`] builds from the session's XKB keymap.

use super::uinput_keyboard::LineBreak;
use input_linux::Key;
use once_cell::sync::Lazy;
use std::collections::HashMap;

/// The keys that type characters on some layout. The keypad is not here,
/// because Num Lock changes what its keys type.
pub const MAIN_KEYS: [Key; 48] = [
    Key::Num1,
    Key::Num2,
    Key::Num3,
    Key::Num4,
    Key::Num5,
    Key::Num6,
    Key::Num7,
    Key::Num8,
    Key::Num9,
    Key::Num0,
    Key::Minus,
    Key::Equal,
    Key::Q,
    Key::W,
    Key::E,
    Key::R,
    Key::T,
    Key::Y,
    Key::U,
    Key::I,
    Key::O,
    Key::P,
    Key::LeftBrace,
    Key::RightBrace,
    Key::A,
    Key::S,
    Key::D,
    Key::F,
    Key::G,
    Key::H,
    Key::J,
    Key::K,
    Key::L,
    Key::Semicolon,
    Key::Apostrophe,
    Key::Grave,
    Key::Backslash,
    Key::Z,
    Key::X,
    Key::C,
    Key::V,
    Key::B,
    Key::N,
    Key::M,
    Key::Comma,
    Key::Dot,
    Key::Slash,
    Key::Space,
];

/// Keys that only ISO, Brazilian, and Japanese keyboards have. A character
/// comes from one of them only when no main key types it.
pub const EXTRA_KEYS: [Key; 3] = [Key::NonUsBackslashAndPipe, Key::Ro, Key::Yen];

/// Keys that can hold Shift or the third level, in the order Hush tries
/// them.
pub const MODIFIER_KEYS: [Key; 9] = [
    Key::LeftShift,
    Key::RightShift,
    Key::RightAlt,
    Key::LeftAlt,
    Key::CapsLock,
    Key::Compose,
    Key::LeftMeta,
    Key::RightMeta,
    Key::NonUsBackslashAndPipe,
];

/// The keys that hold Control and Shift in a chord such as Ctrl+V. An XKB
/// option can move a modifier: with `ctrl:swapcaps`, the Left Ctrl key
/// types Caps Lock, and with `ctrl:swap_lwin_lctl`, it types Super.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChordKeys {
    pub control: Key,
    pub shift: Key,
}

impl Default for ChordKeys {
    /// The keys of a keymap without options.
    fn default() -> Self {
        Self {
            control: Key::LeftCtrl,
            shift: Key::LeftShift,
        }
    }
}

/// One key press with the modifiers held around it. The third level goes
/// down before Shift and comes up after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tap {
    pub key: Key,
    pub shift: Option<Key>,
    pub level3: Option<Key>,
}

impl Tap {
    pub const fn plain(key: Key) -> Self {
        Self {
            key,
            shift: None,
            level3: None,
        }
    }

    pub const fn shifted(key: Key) -> Self {
        Self {
            key,
            shift: Some(Key::LeftShift),
            level3: None,
        }
    }

    /// The modifier keys in the order they go down.
    pub fn modifiers(&self) -> Vec<Key> {
        self.level3.into_iter().chain(self.shift).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stroke {
    Tap(Tap),
    Unmapped(char),
}

/// For each character, one tap, or a dead key tap and then a base tap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyTable {
    chars: HashMap<char, Vec<Tap>>,
    shift: Key,
    locks: TableLocks,
}

/// What a lock does to the keys outside the table: a line break and the
/// paste and submit chords.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TableLocks {
    /// Shift can be locked (Shift Lock), so every key Hush sends gets
    /// Shift: Ctrl+V arrives as Ctrl+Shift+V, and Enter as Shift+Enter.
    pub shift_locked: bool,
    /// A lock is on and no key holds Shift without a side effect, so Hush
    /// must not press Shift.
    pub no_shift: bool,
}

const US: [(char, Key, bool); 95] = [
    ('a', Key::A, false),
    ('b', Key::B, false),
    ('c', Key::C, false),
    ('d', Key::D, false),
    ('e', Key::E, false),
    ('f', Key::F, false),
    ('g', Key::G, false),
    ('h', Key::H, false),
    ('i', Key::I, false),
    ('j', Key::J, false),
    ('k', Key::K, false),
    ('l', Key::L, false),
    ('m', Key::M, false),
    ('n', Key::N, false),
    ('o', Key::O, false),
    ('p', Key::P, false),
    ('q', Key::Q, false),
    ('r', Key::R, false),
    ('s', Key::S, false),
    ('t', Key::T, false),
    ('u', Key::U, false),
    ('v', Key::V, false),
    ('w', Key::W, false),
    ('x', Key::X, false),
    ('y', Key::Y, false),
    ('z', Key::Z, false),
    ('A', Key::A, true),
    ('B', Key::B, true),
    ('C', Key::C, true),
    ('D', Key::D, true),
    ('E', Key::E, true),
    ('F', Key::F, true),
    ('G', Key::G, true),
    ('H', Key::H, true),
    ('I', Key::I, true),
    ('J', Key::J, true),
    ('K', Key::K, true),
    ('L', Key::L, true),
    ('M', Key::M, true),
    ('N', Key::N, true),
    ('O', Key::O, true),
    ('P', Key::P, true),
    ('Q', Key::Q, true),
    ('R', Key::R, true),
    ('S', Key::S, true),
    ('T', Key::T, true),
    ('U', Key::U, true),
    ('V', Key::V, true),
    ('W', Key::W, true),
    ('X', Key::X, true),
    ('Y', Key::Y, true),
    ('Z', Key::Z, true),
    ('1', Key::Num1, false),
    ('2', Key::Num2, false),
    ('3', Key::Num3, false),
    ('4', Key::Num4, false),
    ('5', Key::Num5, false),
    ('6', Key::Num6, false),
    ('7', Key::Num7, false),
    ('8', Key::Num8, false),
    ('9', Key::Num9, false),
    ('0', Key::Num0, false),
    (' ', Key::Space, false),
    ('.', Key::Dot, false),
    (',', Key::Comma, false),
    (';', Key::Semicolon, false),
    ('\'', Key::Apostrophe, false),
    ('/', Key::Slash, false),
    ('\\', Key::Backslash, false),
    ('-', Key::Minus, false),
    ('=', Key::Equal, false),
    ('[', Key::LeftBrace, false),
    (']', Key::RightBrace, false),
    ('`', Key::Grave, false),
    ('!', Key::Num1, true),
    ('@', Key::Num2, true),
    ('#', Key::Num3, true),
    ('$', Key::Num4, true),
    ('%', Key::Num5, true),
    ('^', Key::Num6, true),
    ('&', Key::Num7, true),
    ('*', Key::Num8, true),
    ('(', Key::Num9, true),
    (')', Key::Num0, true),
    ('_', Key::Minus, true),
    ('+', Key::Equal, true),
    ('{', Key::LeftBrace, true),
    ('}', Key::RightBrace, true),
    ('|', Key::Backslash, true),
    (':', Key::Semicolon, true),
    ('"', Key::Apostrophe, true),
    ('<', Key::Comma, true),
    ('>', Key::Dot, true),
    ('?', Key::Slash, true),
    ('~', Key::Grave, true),
];

static US_TABLE: Lazy<KeyTable> = Lazy::new(|| {
    let chars = US
        .iter()
        .map(|&(ch, key, shift)| {
            let tap = if shift {
                Tap::shifted(key)
            } else {
                Tap::plain(key)
            };
            (ch, vec![tap])
        })
        .collect();
    KeyTable::new(chars, Key::LeftShift)
});

impl KeyTable {
    /// `shift` is the key that holds Shift for a chat line break.
    pub fn new(chars: HashMap<char, Vec<Tap>>, shift: Key) -> Self {
        Self {
            chars,
            shift,
            locks: TableLocks::default(),
        }
    }

    pub fn with_locks(self, locks: TableLocks) -> Self {
        Self { locks, ..self }
    }

    pub fn locks(&self) -> TableLocks {
        self.locks
    }

    /// The plain US layout, for when libxkbcommon is not available.
    pub fn us() -> &'static KeyTable {
        &US_TABLE
    }

    pub fn taps(&self, ch: char) -> Option<&[Tap]> {
        self.chars.get(&ch).map(Vec::as_slice)
    }

    /// The key that types `ch` with no modifier and no dead key.
    pub fn plain_key(&self, ch: char) -> Option<Key> {
        match self.taps(ch)? {
            [tap] if tap.shift.is_none() && tap.level3.is_none() => Some(tap.key),
            _ => None,
        }
    }

    /// The key that types a letter, in either case, with no modifier. With
    /// Caps Lock on, the plain key types the capital letter.
    pub fn letter_key(&self, letter: char) -> Option<Key> {
        self.plain_key(letter).or_else(|| {
            letter
                .to_uppercase()
                .next()
                .and_then(|upper| self.plain_key(upper))
        })
    }

    /// The characters that both tables type with the same taps, for a
    /// keyboard that can have either keymap.
    pub fn common(&self, other: &KeyTable) -> KeyTable {
        let chars = self
            .chars
            .iter()
            .filter(|(ch, taps)| other.chars.get(ch) == Some(taps))
            .map(|(&ch, taps)| (ch, taps.clone()))
            .collect();
        let shift = if self.shift == other.shift {
            self.shift
        } else {
            Key::LeftShift
        };
        KeyTable::new(chars, shift).with_locks(TableLocks {
            shift_locked: self.locks.shift_locked || other.locks.shift_locked,
            no_shift: self.locks.no_shift || other.locks.no_shift,
        })
    }

    pub fn dead_key_chars(&self) -> usize {
        self.chars.values().filter(|taps| taps.len() > 1).count()
    }

    /// Whether the table has keys for every character. Line breaks and Tab
    /// use the same keys on every layout. Without a Shift key that is safe
    /// to press, a line break, which is Shift+Enter in chat, does not count.
    pub fn covers(&self, text: &str) -> bool {
        text.chars().all(|ch| match ch {
            '\n' | '\r' => !self.locks.no_shift,
            '\t' => true,
            _ => self.chars.contains_key(&ch),
        })
    }

    /// The taps that type a text. `\r\n`, `\r`, and `\n` are each one line
    /// break.
    pub fn strokes(&self, text: &str, line_break: LineBreak) -> Vec<Stroke> {
        let enter = match line_break {
            LineBreak::ShiftEnter if self.locks.no_shift => Stroke::Unmapped('\n'),
            LineBreak::ShiftEnter => Stroke::Tap(Tap {
                key: Key::Enter,
                shift: Some(self.shift),
                level3: None,
            }),
            LineBreak::Enter => Stroke::Tap(Tap::plain(Key::Enter)),
        };
        let mut strokes = Vec::with_capacity(text.len());
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            match ch {
                '\r' => {
                    chars.next_if_eq(&'\n');
                    strokes.push(enter);
                },
                '\n' => strokes.push(enter),
                '\t' => strokes.push(Stroke::Tap(Tap::plain(Key::Tab))),
                _ => match self.chars.get(&ch) {
                    Some(taps) => strokes.extend(taps.iter().copied().map(Stroke::Tap)),
                    None => strokes.push(Stroke::Unmapped(ch)),
                },
            }
        }
        strokes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tap(key: Key, shift: bool) -> Stroke {
        Stroke::Tap(if shift {
            Tap::shifted(key)
        } else {
            Tap::plain(key)
        })
    }

    #[test]
    fn the_us_table_maps_letters_digits_and_symbols() {
        let us = KeyTable::us();
        assert_eq!(us.taps('a'), Some(&[Tap::plain(Key::A)][..]));
        assert_eq!(us.taps('A'), Some(&[Tap::shifted(Key::A)][..]));
        assert_eq!(us.taps('1'), Some(&[Tap::plain(Key::Num1)][..]));
        assert_eq!(us.taps('!'), Some(&[Tap::shifted(Key::Num1)][..]));
        assert_eq!(us.taps(' '), Some(&[Tap::plain(Key::Space)][..]));
        assert_eq!(us.taps('é'), None);
        assert_eq!(us.dead_key_chars(), 0);
        for (ch, key, _) in US {
            assert!(
                MAIN_KEYS.contains(&key),
                "{ch:?} uses a key outside MAIN_KEYS"
            );
        }
    }

    #[test]
    fn chat_line_breaks_press_shift_enter() {
        let us = KeyTable::us();
        let a = tap(Key::A, false);
        let b = tap(Key::B, false);
        let c = tap(Key::C, false);
        let shift_enter = tap(Key::Enter, true);
        assert_eq!(
            us.strokes("a\nb", LineBreak::ShiftEnter),
            [a, shift_enter, b]
        );
        assert_eq!(
            us.strokes("a\r\nb\rc", LineBreak::ShiftEnter),
            [a, shift_enter, b, shift_enter, c],
            "\\r\\n is one line break, and so is a lone \\r"
        );
        assert_eq!(
            us.strokes("a\n\nb", LineBreak::ShiftEnter),
            [a, shift_enter, shift_enter, b],
            "a paragraph break is two new lines"
        );
    }

    #[test]
    fn other_line_breaks_press_enter() {
        let us = KeyTable::us();
        assert_eq!(
            us.strokes("A\r\n\t!é", LineBreak::Enter),
            [
                tap(Key::A, true),
                tap(Key::Enter, false),
                tap(Key::Tab, false),
                tap(Key::Num1, true),
                Stroke::Unmapped('é'),
            ]
        );
        assert!(us.covers("one\r\ntwo\tthree"));
        assert!(!us.covers("café"));
    }

    #[test]
    fn a_dead_key_character_is_two_taps() {
        let dead_acute = Tap::plain(Key::Equal);
        let e = Tap::plain(Key::E);
        let mut chars = HashMap::new();
        chars.insert('é', vec![dead_acute, e]);
        chars.insert('e', vec![e]);
        let table = KeyTable::new(chars, Key::RightShift);
        assert_eq!(
            table.strokes("eé\n", LineBreak::ShiftEnter),
            [
                Stroke::Tap(e),
                Stroke::Tap(dead_acute),
                Stroke::Tap(e),
                Stroke::Tap(Tap {
                    key: Key::Enter,
                    shift: Some(Key::RightShift),
                    level3: None
                }),
            ]
        );
        assert_eq!(table.dead_key_chars(), 1);
        assert!(table.covers("eé"));
        assert!(!table.covers("ë"));
    }

    #[test]
    fn two_tables_share_only_the_same_taps() {
        let altgr_q = Tap {
            key: Key::Q,
            shift: None,
            level3: Some(Key::RightAlt),
        };
        let one = KeyTable::new(
            HashMap::from([
                ('a', vec![Tap::plain(Key::A)]),
                ('<', vec![Tap::plain(Key::NonUsBackslashAndPipe)]),
                ('@', vec![altgr_q]),
                ('é', vec![Tap::plain(Key::Equal), Tap::plain(Key::E)]),
            ]),
            Key::LeftShift,
        );
        let other = KeyTable::new(
            HashMap::from([
                ('a', vec![Tap::plain(Key::A)]),
                ('@', vec![altgr_q]),
                ('é', vec![Tap::plain(Key::Grave), Tap::plain(Key::E)]),
                ('z', vec![Tap::plain(Key::Z)]),
            ]),
            Key::LeftShift,
        );
        let common = one.common(&other);
        assert_eq!(common.taps('a'), Some(&[Tap::plain(Key::A)][..]));
        assert_eq!(common.taps('@'), Some(&[altgr_q][..]));
        assert_eq!(common.taps('<'), None, "only one table has it");
        assert_eq!(common.taps('z'), None, "only the other table has it");
        assert_eq!(common.taps('é'), None, "the dead keys differ");
        assert_eq!(common, other.common(&one));
    }

    #[test]
    fn without_a_safe_shift_a_line_break_is_not_covered() {
        let no_shift = KeyTable::us().clone().with_locks(TableLocks {
            shift_locked: false,
            no_shift: true,
        });
        assert!(no_shift.covers("one two\tthree"));
        assert!(!no_shift.covers("one\ntwo"));
        assert!(!no_shift.covers("one\r\ntwo"));
        assert_eq!(
            no_shift.strokes("a\nb", LineBreak::ShiftEnter),
            [
                tap(Key::A, false),
                Stroke::Unmapped('\n'),
                tap(Key::B, false)
            ],
            "an explicit uinput choice drops the chat line break, not Caps Lock"
        );
        assert_eq!(
            no_shift.strokes("a\nb", LineBreak::Enter),
            [
                tap(Key::A, false),
                tap(Key::Enter, false),
                tap(Key::B, false)
            ]
        );
        assert!(KeyTable::us().covers("one\ntwo"));
    }

    #[test]
    fn common_keys_keep_every_lock_flag() {
        let shift_locked = KeyTable::us().clone().with_locks(TableLocks {
            shift_locked: true,
            no_shift: true,
        });
        let common = KeyTable::us().common(&shift_locked);
        assert_eq!(
            common.locks(),
            TableLocks {
                shift_locked: true,
                no_shift: true
            }
        );
        assert_eq!(KeyTable::us().locks(), TableLocks::default());
    }

    #[test]
    fn a_tap_presses_the_third_level_before_shift() {
        let tap = Tap {
            key: Key::Q,
            shift: Some(Key::LeftShift),
            level3: Some(Key::RightAlt),
        };
        assert_eq!(tap.modifiers(), [Key::RightAlt, Key::LeftShift]);
        assert!(Tap::plain(Key::Q).modifiers().is_empty());
    }
}
