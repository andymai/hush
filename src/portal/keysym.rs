//! Text as key presses for the RemoteDesktop portal.
//!
//! A character goes as a keysym, and the compositor finds its key in the
//! active layout. KWin and Mutter drop a keysym that the layout lacks, with
//! no error to the sender, so Hush types only the characters that the
//! layout surely has, on its first two levels, and pastes other text. A key
//! that has the same place on every layout, such as Enter, Tab, Backspace,
//! Shift, or Insert, goes as a key code, because a keysym can be missing:
//! no Latin `v` is on an active Russian or Greek layout.

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

/// The characters that type through the portal; the rest paste.
#[derive(Debug, Clone)]
pub enum Typeable {
    /// Letters, digits, space, and `. , ; : ! ? - ( )`, which every Latin
    /// layout has on its first two levels, with no dead key. `^`, `` ` ``,
    /// `'`, `"`, and `~` are dead keys or third-level keys on German and US
    /// International. For a sandbox, which cannot read the keymap.
    Conservative,
    /// The characters on the first two levels of the active layout's keymap,
    /// with no dead key.
    Keymap(Arc<KeyTable>),
}

impl Typeable {
    pub fn allows(&self, ch: char) -> bool {
        match self {
            Self::Conservative => {
                ch.is_ascii_alphanumeric()
                    || matches!(
                        ch,
                        ' ' | '.' | ',' | ';' | ':' | '!' | '?' | '-' | '(' | ')'
                    )
            },
            Self::Keymap(table) => matches!(
                table.taps(ch),
                Some([tap]) if tap.level3.is_none()
            ),
        }
    }

    /// Whether every character of the text types. Line breaks and Tab are
    /// key codes, which every layout has.
    pub fn covers(&self, text: &str) -> bool {
        text.chars()
            .all(|ch| matches!(ch, '\n' | '\r' | '\t') || self.allows(ch))
    }
}

fn tap_sym(steps: &mut Vec<KeyStep>, sym: u32) {
    steps.push(KeyStep::SymPress(sym));
    steps.push(KeyStep::SymRelease(sym));
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
/// `\r`, and `\n` are each one line break. A character with no keysym is
/// skipped.
pub fn text_steps(text: &str, line_break: LineBreak, delay: Duration) -> Vec<KeyStep> {
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
            _ => match keysym(ch) {
                Some(sym) => tap_sym(&mut steps, sym),
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

    #[test]
    fn the_sandbox_types_only_what_every_latin_layout_has() {
        let conservative = Typeable::Conservative;
        assert!(conservative.covers("Hello, world! Is it 42 (or 43)? Yes: no; well-known.\n\t"));
        for risky in ["^", "`", "'", "\"", "~", "@", "café", "日本", "👍"] {
            assert!(
                !conservative.covers(risky),
                "{risky} is a dead key, a third-level key, or not on the layout"
            );
        }
        assert!(conservative.covers(""));
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
                ('@', vec![altgr]),
                ('ê', vec![Tap::plain(Key::LeftBrace), Tap::plain(Key::E)]),
            ]),
            Key::LeftShift,
        );
        let keymap = Typeable::Keymap(Arc::new(table));
        assert!(keymap.covers("aAé\n"));
        assert!(!keymap.covers("@"), "a third-level key");
        assert!(!keymap.covers("ê"), "a dead-key sequence");
        assert!(!keymap.covers("b"), "not on the layout");
    }

    #[test]
    fn text_becomes_keysyms_and_line_breaks_key_codes() {
        let delay = Duration::from_millis(20);
        assert_eq!(
            text_steps("aB", LineBreak::Enter, delay),
            [
                SymPress(0x61),
                SymRelease(0x61),
                Pause(delay),
                SymPress(0x42),
                SymRelease(0x42)
            ]
        );
        assert_eq!(
            text_steps("a\r\nb", LineBreak::Enter, Duration::ZERO),
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
            text_steps("\n\t", LineBreak::ShiftEnter, Duration::ZERO),
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
            text_steps("a\u{7}b", LineBreak::Enter, delay),
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
