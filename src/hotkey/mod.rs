//! Global hotkeys.
//!
//! [`HotkeyManager`] picks a backend: evdev reads `/dev/input` directly and
//! works on every session type; X11 hotkeys are the fallback when the event
//! devices are not readable but an X11 display is. Both report raw
//! [`HotkeyEvent`]s; [`GestureDetector`] turns them into recording actions
//! according to the [`HotkeyMode`].
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
//!         HotkeyEvent::Cancel => println!("discarded"),
//!         HotkeyEvent::Action(action) => println!("{:?}", action),
//!     }
//! }
//! # Ok::<(), anyhow::Error>(())
//! ```

pub mod combination;
pub mod evdev;
pub mod gesture;
pub mod x11;

use crate::Result;
use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use tracing::warn;

pub use combination::KeyCombination;
pub use gesture::{GestureAction, GestureDetector};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    Pressed,
    Released,
    /// The cancel key went down.
    Cancel,
    /// A secondary chord went down.
    Action(HotkeyAction),
}

/// Secondary chords that run a daemon command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotkeyAction {
    PasteLast,
    Learn,
}

/// The keys a backend watches: the dictation hotkey, an optional cancel key,
/// and optional action chords.
#[derive(Debug, Clone)]
pub struct HotkeyBindings {
    pub primary: KeyCombination,
    pub cancel: Option<KeyCombination>,
    pub actions: Vec<(HotkeyAction, KeyCombination)>,
}

impl HotkeyBindings {
    pub fn parse(primary: &str, cancel: Option<&str>) -> Result<Self> {
        let primary = KeyCombination::parse(primary)?;
        let cancel = match cancel.map(str::trim).filter(|c| !c.is_empty()) {
            Some(text) => Some(KeyCombination::parse(text)?),
            None => None,
        };
        if let Some(cancel) = &cancel {
            if cancel.evdev_key() == primary.evdev_key() {
                return Err(anyhow::anyhow!(
                    "The cancel key {} cannot share a key with the hotkey {}",
                    cancel,
                    primary
                ));
            }
        }
        Ok(Self {
            primary,
            cancel,
            actions: Vec::new(),
        })
    }

    /// Bind an action chord; an empty string leaves the action unbound.
    pub fn with_action(mut self, action: HotkeyAction, text: &str) -> Result<Self> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(self);
        }
        let combination = KeyCombination::parse(text)?;
        let taken = self
            .all()
            .any(|existing| existing.to_string() == combination.to_string());
        if taken {
            return Err(anyhow::anyhow!(
                "{} is already bound; {:?} needs its own chord",
                combination,
                action
            ));
        }
        self.actions.push((action, combination));
        Ok(self)
    }

    fn all(&self) -> impl Iterator<Item = &KeyCombination> {
        std::iter::once(&self.primary)
            .chain(self.cancel.iter())
            .chain(self.actions.iter().map(|(_, c)| c))
    }
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
        Self::with_backend(
            HotkeyBindings::parse(combination, None)?,
            HotkeyBackend::Auto,
            false,
        )
    }

    /// `exclusive` makes the evdev backend grab the hotkey's device and hide
    /// the hotkey from applications; the X11 grab is exclusive by nature.
    pub fn with_backend(
        bindings: HotkeyBindings,
        choice: HotkeyBackend,
        exclusive: bool,
    ) -> Result<(Self, mpsc::Receiver<HotkeyEvent>)> {
        let combination = bindings.primary.clone();
        let (backend, receiver) = match choice {
            HotkeyBackend::Evdev => {
                let (hotkey, rx) = evdev::EvdevHotkey::new(bindings, exclusive)?;
                (Backend::Evdev(hotkey), rx)
            },
            HotkeyBackend::X11 => {
                let (hotkey, rx) = x11::X11Hotkey::new(bindings)?;
                (Backend::X11(hotkey), rx)
            },
            HotkeyBackend::Auto => match evdev::EvdevHotkey::new(bindings.clone(), exclusive) {
                Ok((hotkey, rx)) => (Backend::Evdev(hotkey), rx),
                Err(evdev_err) if std::env::var_os("DISPLAY").is_some() => {
                    warn!("{}; falling back to X11 hotkeys", evdev_err);
                    let (hotkey, rx) = x11::X11Hotkey::new(bindings)
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

    #[test]
    fn bindings_parse_an_optional_cancel_key() {
        let b = HotkeyBindings::parse("RightAlt", Some("Escape")).unwrap();
        assert_eq!(b.cancel.unwrap().to_string(), "Escape");
        assert!(HotkeyBindings::parse("RightAlt", Some("  "))
            .unwrap()
            .cancel
            .is_none());
        assert!(HotkeyBindings::parse("Escape", Some("Escape")).is_err());
    }

    #[test]
    fn action_chords_must_be_distinct() {
        let b = HotkeyBindings::parse("RightAlt", Some("Escape"))
            .unwrap()
            .with_action(HotkeyAction::PasteLast, "Shift+RightAlt")
            .unwrap()
            .with_action(HotkeyAction::Learn, "")
            .unwrap();
        assert_eq!(b.actions.len(), 1);
        assert!(b
            .clone()
            .with_action(HotkeyAction::Learn, "shift+rightalt")
            .is_err());
        assert!(b.with_action(HotkeyAction::Learn, "Escape").is_err());
    }
}
