//! Puts transcribed text into the focused window.
//!
//! [`TextInserter`] types through `/dev/uinput`, which reaches every
//! application, VM, and password field on X11 and Wayland. Text the US key
//! map cannot express, or very long text, is pasted through the clipboard
//! with a uinput `Ctrl+V` and the clipboard is restored afterwards.
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

#[cfg(target_os = "linux")]
pub mod insertion;
#[cfg(target_os = "linux")]
pub mod uinput_keyboard;
#[cfg(target_os = "linux")]
pub mod window;

#[cfg(target_os = "linux")]
pub use insertion::{
    choose_method, diagnose_uinput_issues, print_uinput_setup_guidance, Chosen, TextInserter,
};
#[cfg(target_os = "linux")]
pub use uinput_keyboard::{check_uinput_availability, UinputKeyboard};
#[cfg(target_os = "linux")]
pub use window::{WindowInfo, WindowProvider};
