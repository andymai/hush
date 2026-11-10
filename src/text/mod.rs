//! Puts transcribed text into the focused window.
//!
//! [`TextInserter`] types through `/dev/uinput`, which reaches every
//! application, VM, and password field on X11 and Wayland. The keys for each
//! character come from the active layout's XKB keymap. Text that no key or
//! dead-key sequence types, and very long text, is pasted through the
//! clipboard with the [`PasteChord`] the focused window takes. The [`clipboard`]
//! module saves the old clipboard before the paste and puts it back
//! afterwards.
//! [`WindowProvider`] names the focused window when the compositor offers
//! that; insertion never depends on it.
//!
//! ```no_run
//! use hush::text::TextInserter;
//!
//! let mut inserter = TextInserter::new()?;
//! inserter.insert_text("Hello, world!")?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! Device access comes from the udev rule `hush setup permissions` installs.

use serde::{Deserialize, Serialize};
use std::fmt;

/// How transcribed text reaches the focused window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum InsertionMethod {
    /// uinput typing, clipboard paste for text uinput cannot type or that is very long
    #[default]
    Auto,
    Uinput,
    Clipboard,
}

/// The keys Hush presses to paste.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum PasteChord {
    /// Most applications
    CtrlV,
    /// Terminals, where Ctrl+V is a control character or pastes an image
    CtrlShiftV,
    /// Konsole, and editors with a terminal inside. Some applications paste
    /// the primary selection on it, so Hush sets that selection too.
    ShiftInsert,
}

impl PasteChord {
    pub const ALL: [Self; 3] = [Self::CtrlV, Self::CtrlShiftV, Self::ShiftInsert];

    /// The spelling in the configuration file.
    pub fn name(self) -> &'static str {
        match self {
            Self::CtrlV => "ctrl+v",
            Self::CtrlShiftV => "ctrl+shift+v",
            Self::ShiftInsert => "shift+insert",
        }
    }
}

impl fmt::Display for PasteChord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl TryFrom<String> for PasteChord {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let wanted = value.trim().to_lowercase();
        Self::ALL
            .into_iter()
            .find(|chord| chord.name() == wanted)
            .ok_or_else(|| {
                format!(
                    "unknown paste chord '{}': use ctrl+v, ctrl+shift+v, or shift+insert",
                    value
                )
            })
    }
}

impl From<PasteChord> for String {
    fn from(chord: PasteChord) -> Self {
        chord.name().to_string()
    }
}

/// Whether a window class rule names this class: the whole class, or its
/// last dot-separated part (`konsole` names `org.kde.konsole`), in any case.
/// X11 reports WM_CLASS as the instance and the class with a NUL between
/// them, and either part counts.
pub fn class_matches(rule: &str, class: &str) -> bool {
    let rule = rule.trim();
    if rule.is_empty() {
        return false;
    }
    class
        .split('\0')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .any(|part| {
            part.eq_ignore_ascii_case(rule)
                || part
                    .rsplit('.')
                    .next()
                    .is_some_and(|last| last.eq_ignore_ascii_case(rule))
        })
}

#[cfg(target_os = "linux")]
pub mod clipboard;
#[cfg(target_os = "linux")]
pub mod clock;
#[cfg(target_os = "linux")]
pub mod insertion;
#[cfg(target_os = "linux")]
pub mod keymap;
#[cfg(target_os = "linux")]
pub mod kwin;
#[cfg(target_os = "linux")]
pub mod layout;
#[cfg(target_os = "linux")]
pub mod modifiers;
#[cfg(target_os = "linux")]
pub mod selection;
#[cfg(target_os = "linux")]
pub mod typing;
#[cfg(target_os = "linux")]
pub mod uinput_keyboard;
#[cfg(target_os = "linux")]
pub mod window;
#[cfg(target_os = "linux")]
pub mod xkb;

#[cfg(target_os = "linux")]
pub use insertion::{
    choose_method, diagnose_uinput_issues, print_uinput_setup_guidance, Chosen, TextInserter,
};
#[cfg(target_os = "linux")]
pub use uinput_keyboard::{check_uinput_availability, UinputKeyboard};
#[cfg(target_os = "linux")]
pub use window::{WindowInfo, WindowProvider};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_rules_match_whole_names_only() {
        assert!(class_matches("konsole", "org.kde.konsole"));
        assert!(class_matches("Konsole", "konsole"));
        assert!(class_matches("org.kde.konsole", "org.kde.konsole"));
        assert!(class_matches("code", "code\0Code"));
        assert!(class_matches("Code", "com.visualstudio.code"));
        assert!(!class_matches("st", "steam"));
        assert!(!class_matches("st", "st-256color"));
        assert!(!class_matches("kde", "org.kde.konsole"));
        assert!(!class_matches("code", "codium"));
        assert!(!class_matches("", "konsole"));
        assert!(!class_matches("  ", ""));
    }

    #[test]
    fn paste_chords_parse_from_their_names() {
        for chord in PasteChord::ALL {
            assert_eq!(PasteChord::try_from(chord.name().to_string()), Ok(chord));
            assert_eq!(String::from(chord), chord.name());
        }
        assert_eq!(
            PasteChord::try_from(" Ctrl+Shift+V ".to_string()),
            Ok(PasteChord::CtrlShiftV)
        );
        let error = PasteChord::try_from("ctrl+insert".to_string()).unwrap_err();
        assert!(error.contains("ctrl+insert") && error.contains("shift+insert"));
    }
}
