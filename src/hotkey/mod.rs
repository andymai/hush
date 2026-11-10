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
//!         HotkeyEvent::Command(pressed) => println!("command chord {}", pressed),
//!         HotkeyEvent::Language(language, pressed) => println!("{} chord {}", language, pressed),
//!         HotkeyEvent::Style(style, pressed) => println!("{} chord {}", style, pressed),
//!     }
//! }
//! # Ok::<(), anyhow::Error>(())
//! ```

pub mod combination;
pub mod evdev;
pub mod gesture;
pub mod keymap;
pub mod menu_mask;
pub mod modifiers;
pub mod warnings;
pub mod x11;

use crate::text_processing::StyleName;
use crate::transcription::Language;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use tracing::warn;

pub use combination::KeyCombination;
pub use gesture::{GestureAction, GestureDetector};
pub use warnings::Role;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    Pressed,
    Released,
    /// The cancel key went down.
    Cancel,
    /// A secondary chord went down.
    Action(HotkeyAction),
    /// The Command Mode chord went down (`true`) or up (`false`).
    Command(bool),
    /// A language chord went down (`true`) or up (`false`). It works like
    /// the dictation hotkey, for a take in that language.
    Language(Language, bool),
    /// A style chord went down (`true`) or up (`false`). It works like the
    /// dictation hotkey, for a take in that style.
    Style(StyleName, bool),
}

/// Secondary chords that run a daemon command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotkeyAction {
    PasteLast,
    Learn,
    /// Rewrite the selected text with the style's prompt
    Rewrite(StyleName),
}

/// The keys a backend watches: the dictation hotkey, an optional cancel key,
/// optional action chords, and a dictation chord for each language and each
/// style.
#[derive(Debug, Clone)]
pub struct HotkeyBindings {
    pub primary: KeyCombination,
    pub cancel: Option<KeyCombination>,
    pub actions: Vec<(HotkeyAction, KeyCombination)>,
    pub command: Option<KeyCombination>,
    pub languages: Vec<(Language, KeyCombination)>,
    pub styles: Vec<(StyleName, KeyCombination)>,
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
            command: None,
            languages: Vec::new(),
            styles: Vec::new(),
        })
    }

    /// Bind the Command Mode chord; an empty string leaves it unbound.
    pub fn with_command(mut self, text: &str) -> Result<Self> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(self);
        }
        let combination = KeyCombination::parse(text)?;
        if self
            .all()
            .any(|existing| existing.to_string() == combination.to_string())
        {
            return Err(anyhow::anyhow!(
                "{} is already bound; Command Mode needs its own chord",
                combination
            ));
        }
        self.command = Some(combination);
        Ok(self)
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
            let name = match action {
                HotkeyAction::Rewrite(style) => format!("the {} rewrite", style),
                other => format!("{:?}", other),
            };
            return Err(anyhow::anyhow!(
                "{} is already bound; {} needs its own chord",
                combination,
                name
            ));
        }
        self.actions.push((action, combination));
        Ok(self)
    }

    /// Bind a dictation chord for `language`.
    pub fn with_language(mut self, language: Language, text: &str) -> Result<Self> {
        let combination = KeyCombination::parse(text)?;
        if self
            .all()
            .any(|existing| existing.to_string() == combination.to_string())
        {
            return Err(anyhow::anyhow!(
                "{} is already bound; the {} chord needs its own chord",
                combination,
                language.name()
            ));
        }
        self.languages.push((language, combination));
        Ok(self)
    }

    /// Bind a dictation chord for `style`.
    pub fn with_style(mut self, style: StyleName, text: &str) -> Result<Self> {
        let combination = KeyCombination::parse(text)?;
        if self
            .all()
            .any(|existing| existing.to_string() == combination.to_string())
        {
            return Err(anyhow::anyhow!(
                "{} is already bound; the {} style chord needs its own chord",
                combination,
                style
            ));
        }
        self.styles.push((style, combination));
        Ok(self)
    }

    fn all(&self) -> impl Iterator<Item = &KeyCombination> {
        std::iter::once(&self.primary)
            .chain(self.cancel.iter())
            .chain(self.actions.iter().map(|(_, c)| c))
            .chain(self.command.iter())
            .chain(self.languages.iter().map(|(_, c)| c))
            .chain(self.styles.iter().map(|(_, c)| c))
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
            false,
        )
    }

    /// `exclusive` makes the evdev backend grab the hotkey's device and hide
    /// the hotkey from applications; the X11 grab is exclusive by nature.
    /// Without a grab, `mask_menu` makes the evdev backend tap Ctrl when a
    /// chord of only Alt and Super keys goes down. Then its release opens no
    /// application menu and no launcher.
    pub fn with_backend(
        bindings: HotkeyBindings,
        choice: HotkeyBackend,
        exclusive: bool,
        mask_menu: bool,
    ) -> Result<(Self, mpsc::Receiver<HotkeyEvent>)> {
        let combination = bindings.primary.clone();
        let open_evdev = |bindings| evdev::EvdevHotkey::new(bindings, exclusive, mask_menu);
        let (backend, receiver) = match choice {
            HotkeyBackend::Evdev => {
                let (hotkey, rx) = open_evdev(bindings)?;
                (Backend::Evdev(hotkey), rx)
            },
            HotkeyBackend::X11 => {
                let (hotkey, rx) = x11::X11Hotkey::new(bindings)?;
                (Backend::X11(hotkey), rx)
            },
            HotkeyBackend::Auto => match open_evdev(bindings.clone()) {
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

    /// The keys, as evdev codes, that Hush's typing must not press.
    pub fn grabbed_keys(&self) -> Vec<u16> {
        grabbed_keys(matches!(self.backend, Backend::X11(_)), &self.combination)
    }
}

/// The X11 grab takes its key from every keyboard, Hush's typing device
/// included, so a typed AltGr on a bare RightAlt hotkey would start a
/// dictation. Typing never holds Ctrl, Alt, or Super, so a chord with one
/// of those is safe. The evdev backend skips Hush's own devices and
/// reserves nothing.
fn grabbed_keys(x11: bool, combination: &KeyCombination) -> Vec<u16> {
    if x11 && !combination.is_mouse_button() && !combination.has_modifiers() {
        vec![combination.evdev_key().code()]
    } else {
        Vec::new()
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
    fn only_the_x11_grab_reserves_its_key() {
        let right_alt = KeyCombination::parse("RightAlt").unwrap();
        assert_eq!(
            grabbed_keys(true, &right_alt),
            [::evdev::KeyCode::KEY_RIGHTALT.code()]
        );
        assert!(
            grabbed_keys(false, &right_alt).is_empty(),
            "the evdev reader skips Hush's typing device"
        );
        let chord = KeyCombination::parse("Ctrl+RightAlt").unwrap();
        assert!(
            grabbed_keys(true, &chord).is_empty(),
            "typing never holds Ctrl"
        );
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

    #[test]
    fn language_chords_are_distinct_from_every_other_chord() {
        let german = Language::parse("de").unwrap();
        let french = Language::parse("fr").unwrap();
        let b = HotkeyBindings::parse("RightAlt", Some("Escape"))
            .unwrap()
            .with_command("Ctrl+RightAlt")
            .unwrap()
            .with_language(german, "Shift+RightAlt")
            .unwrap();
        assert_eq!(b.languages.len(), 1);
        assert_eq!(b.languages[0].0, german);
        assert!(b.clone().with_language(french, "shift+rightalt").is_err());
        assert!(b.clone().with_language(french, "Ctrl+RightAlt").is_err());
        assert!(b.clone().with_language(french, "Ctrl+Nope").is_err());
        assert!(b
            .clone()
            .with_action(HotkeyAction::Learn, "Shift+RightAlt")
            .is_err());
        assert!(b.with_language(french, "F13").is_ok());
    }

    #[test]
    fn style_chords_are_distinct_from_every_other_chord() {
        let german = Language::parse("de").unwrap();
        let verbatim = crate::text_processing::styles::VERBATIM;
        let casual = StyleName::parse("casual").unwrap();
        let b = HotkeyBindings::parse("RightAlt", Some("Escape"))
            .unwrap()
            .with_command("Ctrl+RightAlt")
            .unwrap()
            .with_language(german, "Shift+RightAlt")
            .unwrap()
            .with_style(verbatim, "Super+RightAlt")
            .unwrap();
        assert_eq!(
            b.styles,
            vec![(verbatim, KeyCombination::parse("Super+RightAlt").unwrap())]
        );
        assert!(b.clone().with_style(casual, "super+rightalt").is_err());
        assert!(b.clone().with_style(casual, "Shift+RightAlt").is_err());
        assert!(b.clone().with_style(casual, "Ctrl+RightAlt").is_err());
        assert!(b.clone().with_style(casual, "Ctrl+Nope").is_err());
        assert!(b.clone().with_language(german, "Super+RightAlt").is_err());
        assert!(b
            .clone()
            .with_action(HotkeyAction::PasteLast, "Super+RightAlt")
            .is_err());
        assert!(b.with_style(casual, "F13").is_ok());
    }

    #[test]
    fn rewrite_chords_are_actions_with_their_own_chord() {
        let formal = StyleName::parse("formal").unwrap();
        let b = HotkeyBindings::parse("RightAlt", Some("Escape"))
            .unwrap()
            .with_style(formal, "Super+RightAlt")
            .unwrap()
            .with_action(HotkeyAction::Rewrite(formal), "Super+F")
            .unwrap();
        assert_eq!(b.actions.len(), 1);
        assert_eq!(b.actions[0].0, HotkeyAction::Rewrite(formal));
        let err = b
            .clone()
            .with_action(HotkeyAction::Rewrite(formal), "super+rightalt")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("the formal rewrite needs its own chord"),
            "{err}"
        );
        assert!(b.clone().with_style(formal, "Super+F").is_err());
        assert!(b.with_action(HotkeyAction::Rewrite(formal), "").is_ok());
    }

    #[test]
    fn the_command_chord_is_distinct_too() {
        let b = HotkeyBindings::parse("RightAlt", Some("Escape"))
            .unwrap()
            .with_command("Ctrl+RightAlt")
            .unwrap();
        assert_eq!(b.command.as_ref().unwrap().to_string(), "Ctrl+RightAlt");
        assert!(b
            .clone()
            .with_action(HotkeyAction::Learn, "ctrl+rightalt")
            .is_err());
        assert!(HotkeyBindings::parse("RightAlt", None)
            .unwrap()
            .with_command("RightAlt")
            .is_err());
    }
}
