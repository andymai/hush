//! Global hotkeys.
//!
//! [`HotkeyManager`] picks a backend: evdev reads `/dev/input` directly and
//! works on every session type; X11 hotkeys are the fallback when the event
//! devices are not readable but an X11 display is. Both report raw
//! [`HotkeyEvent`]s; [`HotkeyMode`] decides whether a press means hold-to-talk
//! or toggle.
//!
//! ```no_run
//! use hush::hotkey::{HotkeyEvent, HotkeyManager};
//!
//! let (manager, events) = HotkeyManager::new("Ctrl+Shift+Space")?;
//! manager.start_listening()?;
//! while let Ok(event) = events.recv() {
//!     match event {
//!         HotkeyEvent::Pressed => println!("recording"),
//!         HotkeyEvent::Released => println!("stopped"),
//!     }
//! }
//! # Ok::<(), anyhow::Error>(())
//! ```

pub mod combination;
pub mod evdev;
pub mod x11;

use crate::Result;
use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use tracing::warn;

pub use combination::KeyCombination;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    Pressed,
    Released,
}

/// What a hotkey press means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum HotkeyMode {
    /// Record while the key is held.
    #[default]
    Hold,
    /// One press starts recording, the next press stops it.
    Toggle,
}

/// Which backend reads the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum HotkeyBackend {
    /// evdev when `/dev/input` is readable, otherwise X11.
    #[default]
    Auto,
    Evdev,
    X11,
}

enum Backend {
    Evdev(evdev::EvdevHotkey),
    X11(x11::X11Hotkey),
}

pub struct HotkeyManager {
    backend: Backend,
    combination: KeyCombination,
}

impl HotkeyManager {
    /// Parse the combination and pick the backend automatically.
    pub fn new(combination: &str) -> Result<(Self, mpsc::Receiver<HotkeyEvent>)> {
        Self::with_backend(combination, HotkeyBackend::Auto)
    }

    pub fn with_backend(
        combination: &str,
        choice: HotkeyBackend,
    ) -> Result<(Self, mpsc::Receiver<HotkeyEvent>)> {
        let combination = KeyCombination::parse(combination)?;
        let (backend, receiver) = match choice {
            HotkeyBackend::Evdev => {
                let (hotkey, rx) = evdev::EvdevHotkey::new(combination.clone())?;
                (Backend::Evdev(hotkey), rx)
            },
            HotkeyBackend::X11 => {
                let (hotkey, rx) = x11::X11Hotkey::new(combination.clone())?;
                (Backend::X11(hotkey), rx)
            },
            HotkeyBackend::Auto => match evdev::EvdevHotkey::new(combination.clone()) {
                Ok((hotkey, rx)) => (Backend::Evdev(hotkey), rx),
                Err(evdev_err) if std::env::var_os("DISPLAY").is_some() => {
                    warn!("{}; falling back to X11 hotkeys", evdev_err);
                    let (hotkey, rx) = x11::X11Hotkey::new(combination.clone())
                        .map_err(|x11_err| anyhow::anyhow!("{}. {}", evdev_err, x11_err))?;
                    (Backend::X11(hotkey), rx)
                },
                Err(evdev_err) => {
                    return Err(anyhow::anyhow!(
                        "{}. No X11 display is available for the fallback backend.",
                        evdev_err
                    ))
                },
            },
        };
        Ok((
            Self {
                backend,
                combination,
            },
            receiver,
        ))
    }

    pub fn start_listening(&self) -> Result<()> {
        match &self.backend {
            Backend::Evdev(hotkey) => hotkey.start_listening(),
            Backend::X11(hotkey) => hotkey.start_listening(),
        }
    }

    pub fn stop_listening(&self) -> Result<()> {
        match &self.backend {
            Backend::Evdev(hotkey) => hotkey.stop_listening(),
            Backend::X11(hotkey) => hotkey.stop_listening(),
        }
    }

    pub fn get_combination(&self) -> String {
        self.combination.to_string()
    }

    pub fn backend_name(&self) -> &'static str {
        match self.backend {
            Backend::Evdev(_) => "evdev",
            Backend::X11(_) => "x11",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_and_backend_serialize_lowercase() {
        let v: HotkeyMode = serde_plain_from("toggle");
        assert_eq!(v, HotkeyMode::Toggle);
        let b: HotkeyBackend = serde_plain_from("x11");
        assert_eq!(b, HotkeyBackend::X11);
        assert_eq!(HotkeyMode::default(), HotkeyMode::Hold);
        assert_eq!(HotkeyBackend::default(), HotkeyBackend::Auto);
    }

    fn serde_plain_from<T: for<'de> Deserialize<'de>>(value: &str) -> T {
        toml::Value::String(value.to_string())
            .try_into()
            .expect("valid variant")
    }

    #[test]
    fn invalid_combination_fails_before_any_backend() {
        assert!(HotkeyManager::new("Ctrl+Bogus").is_err());
    }
}
