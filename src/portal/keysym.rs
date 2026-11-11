//! Text as keysym presses for the RemoteDesktop portal.
//!
//! The portal takes keysyms, and the compositor finds the key for each in
//! the active layout. KWin and Mutter drop a keysym that the layout lacks,
//! with no error to the sender, so a character outside the layout would
//! vanish. Printable ASCII is on every Latin layout, so Hush types it and
//! pastes other text.

use crate::text::uinput_keyboard::LineBreak;
use crate::text::PasteChord;
use std::time::Duration;

pub const RETURN: u32 = xkeysym::key::Return;
pub const TAB: u32 = xkeysym::key::Tab;
pub const BACKSPACE: u32 = xkeysym::key::BackSpace;
pub const SHIFT_L: u32 = xkeysym::key::Shift_L;
pub const CONTROL_L: u32 = xkeysym::key::Control_L;
pub const INSERT: u32 = xkeysym::key::Insert;
pub const LOWER_V: u32 = xkeysym::key::v;

/// One event of a RemoteDesktop session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyStep {
    Press(u32),
    Release(u32),
    Pause(Duration),
}

/// The keysym of a character, as `xkb_utf32_to_keysym` gives it, or
/// `None` for a control character.
pub fn keysym(ch: char) -> Option<u32> {
    if ch.is_control() {
        return None;
    }
    let sym = xkeysym::Keysym::from_char(ch).raw();
    (sym != xkeysym::key::NoSymbol).then_some(sym)
}

/// Whether the compositor finds a key for the character on every Latin
/// layout.
pub fn types_reliably(ch: char) -> bool {
    matches!(ch, ' '..='~')
}

/// Whether every character of the text types through the portal. Line
/// breaks and Tab have keys on every layout.
pub fn covers(text: &str) -> bool {
    text.chars()
        .all(|ch| matches!(ch, '\n' | '\r' | '\t') || types_reliably(ch))
}

fn tap(steps: &mut Vec<KeyStep>, held: &[u32], sym: u32) {
    steps.extend(held.iter().map(|&modifier| KeyStep::Press(modifier)));
    steps.push(KeyStep::Press(sym));
    steps.push(KeyStep::Release(sym));
    steps.extend(
        held.iter()
            .rev()
            .map(|&modifier| KeyStep::Release(modifier)),
    );
}

/// The steps that type a text, with `delay` between characters. `\r\n`,
/// `\r`, and `\n` are each one line break. A character with no keysym is
/// skipped.
pub fn text_steps(text: &str, line_break: LineBreak, delay: Duration) -> Vec<KeyStep> {
    let mut steps = Vec::with_capacity(text.len() * 3);
    let mut chars = text.chars().peekable();
    let line = |steps: &mut Vec<KeyStep>| match line_break {
        LineBreak::Enter => tap(steps, &[], RETURN),
        LineBreak::ShiftEnter => tap(steps, &[SHIFT_L], RETURN),
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
            '\t' => tap(&mut steps, &[], TAB),
            _ => match keysym(ch) {
                Some(sym) => tap(&mut steps, &[], sym),
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

/// The steps of a paste chord. Keysyms do not depend on the layout, and the
/// compositor holds the modifiers the keysym needs.
pub fn chord_steps(chord: PasteChord) -> Vec<KeyStep> {
    let mut steps = Vec::with_capacity(6);
    match chord {
        PasteChord::CtrlV => tap(&mut steps, &[CONTROL_L], LOWER_V),
        PasteChord::CtrlShiftV => tap(&mut steps, &[CONTROL_L, SHIFT_L], LOWER_V),
        PasteChord::ShiftInsert => tap(&mut steps, &[SHIFT_L], INSERT),
    }
    steps
}

/// Enter after `delay`, so the application handles the text first.
pub fn enter_steps(delay: Duration) -> Vec<KeyStep> {
    let mut steps = vec![KeyStep::Pause(delay)];
    tap(&mut steps, &[], RETURN);
    steps
}

pub fn backspace_steps(count: usize, delay: Duration) -> Vec<KeyStep> {
    let mut steps = Vec::with_capacity(count * 3);
    for i in 0..count {
        if i > 0 && !delay.is_zero() {
            steps.push(KeyStep::Pause(delay));
        }
        tap(&mut steps, &[], BACKSPACE);
    }
    steps
}

#[cfg(test)]
mod tests {
    use super::*;
    use KeyStep::{Pause, Press, Release};

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
    fn only_printable_ascii_types() {
        assert!(covers("Hello, world! 42 ~`|\\\n\ttabs"));
        assert!(!covers("café"), "the layout can lack é, and KWin drops it");
        assert!(!covers("日本"));
        assert!(!covers("👍"));
        assert!(covers(""));
    }

    #[test]
    fn text_becomes_taps_with_a_delay_between_characters() {
        let delay = Duration::from_millis(20);
        assert_eq!(
            text_steps("aB", LineBreak::Enter, delay),
            [
                Press(0x61),
                Release(0x61),
                Pause(delay),
                Press(0x42),
                Release(0x42)
            ]
        );
        assert_eq!(
            text_steps("a\r\nb", LineBreak::Enter, Duration::ZERO),
            [
                Press(0x61),
                Release(0x61),
                Press(RETURN),
                Release(RETURN),
                Press(0x62),
                Release(0x62)
            ],
            "\\r\\n is one line break"
        );
        assert_eq!(
            text_steps("\n\t", LineBreak::ShiftEnter, Duration::ZERO),
            [
                Press(SHIFT_L),
                Press(RETURN),
                Release(RETURN),
                Release(SHIFT_L),
                Press(TAB),
                Release(TAB)
            ],
            "a chat line break is Shift+Enter"
        );
        assert_eq!(
            text_steps("a\u{7}b", LineBreak::Enter, delay),
            [
                Press(0x61),
                Release(0x61),
                Pause(delay),
                Press(0x62),
                Release(0x62)
            ],
            "a control character is skipped with its pause"
        );
    }

    #[test]
    fn chords_enter_and_backspace_are_keysyms_too() {
        assert_eq!(
            chord_steps(PasteChord::CtrlShiftV),
            [
                Press(CONTROL_L),
                Press(SHIFT_L),
                Press(LOWER_V),
                Release(LOWER_V),
                Release(SHIFT_L),
                Release(CONTROL_L)
            ]
        );
        assert_eq!(
            chord_steps(PasteChord::ShiftInsert),
            [
                Press(SHIFT_L),
                Press(INSERT),
                Release(INSERT),
                Release(SHIFT_L)
            ]
        );
        let delay = Duration::from_millis(150);
        assert_eq!(
            enter_steps(delay),
            [Pause(delay), Press(RETURN), Release(RETURN)]
        );
        assert_eq!(
            backspace_steps(2, Duration::ZERO),
            [
                Press(BACKSPACE),
                Release(BACKSPACE),
                Press(BACKSPACE),
                Release(BACKSPACE)
            ]
        );
    }
}
