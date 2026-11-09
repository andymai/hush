//! Puts transcribed text into the focused window.
//!
//! uinput types the text as key events, which works in every application,
//! VM, and password field. Text the US-layout key map cannot express (accents,
//! CJK, emoji) and very long text go through the clipboard: the previous
//! clipboard is saved, the text is pasted with a uinput Ctrl+V, and the
//! clipboard is restored if it still holds the text. Both paths first wait
//! for the user to release Ctrl, Alt, Shift, and Super.

use super::clipboard::{self, ClipboardBackend, Restore, Selection, Transcript};
use super::clock::{Clock, SystemClock};
use super::modifiers::{self, ModifierWait, PhysicalModifiers};
use super::uinput_keyboard::UinputKeyboard;
use super::window::{WindowInfo, WindowProvider};
use super::InsertionMethod;
use crate::config::settings::InsertionConfig;
use crate::Result;
use anyhow::Context;
use std::thread;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Texts longer than this paste even when every character is typeable.
pub const PASTE_THRESHOLD_CHARS: usize = 2000;
/// A slow application reads the clipboard some time after the paste chord.
pub const AFTER_PASTE: Duration = Duration::from_millis(500);

/// The path a given text will take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chosen {
    Uinput,
    Clipboard,
}

/// `types_like_us` is false when the session's keyboard layout is not plain
/// US, so Hush's key codes would turn into different characters.
pub fn choose_method(configured: InsertionMethod, text: &str, types_like_us: bool) -> Chosen {
    match configured {
        InsertionMethod::Uinput => Chosen::Uinput,
        InsertionMethod::Clipboard => Chosen::Clipboard,
        InsertionMethod::Auto => {
            if !types_like_us
                || !UinputKeyboard::can_type_all(text)
                || text.chars().count() > PASTE_THRESHOLD_CHARS
            {
                Chosen::Clipboard
            } else {
                Chosen::Uinput
            }
        },
    }
}

pub struct TextInserter {
    uinput: UinputKeyboard,
    clipboard: Option<Box<dyn ClipboardBackend>>,
    windows: WindowProvider,
    config: InsertionConfig,
    types_like_us: bool,
}

impl TextInserter {
    /// Needs a working uinput device; both paths type through it.
    pub fn new() -> Result<Self> {
        info!("Initializing text insertion");
        let uinput = UinputKeyboard::new()?;
        if !uinput.is_ready() {
            return Err(anyhow::anyhow!(
                "uinput is not available; run `hush setup permissions` and try again"
            ));
        }
        let clipboard = clipboard::open();
        let windows = WindowProvider::detect();
        let types_like_us = super::layout::types_like_us();
        info!(
            "Text insertion ready: {}, clipboard restore {}, window detection via {}",
            if types_like_us {
                "typed key by key"
            } else {
                "pasted, because the keyboard layout is not plain US"
            },
            clipboard
                .as_ref()
                .map_or("off", |clipboard| clipboard.describe()),
            windows.name()
        );
        Ok(Self {
            uinput,
            clipboard,
            windows,
            config: InsertionConfig::default(),
            types_like_us,
        })
    }

    pub fn configure(&mut self, config: &InsertionConfig) {
        self.config = config.clone();
        self.uinput.set_typing_delay(config.typing_delay_ms);
    }

    pub fn get_focused_window(&self) -> Option<WindowInfo> {
        self.windows.focused()
    }

    pub fn insert_text(&mut self, text: &str) -> Result<()> {
        info!(
            "Inserting text: '{}' ({} chars)",
            text.chars().take(50).collect::<String>(),
            text.chars().count()
        );
        let method = match self.windows.focused() {
            Some(window) => {
                let method = self.config.method_for(&window.class);
                debug!(
                    "Target window: {} ({}), method {:?}",
                    window.title, window.class, method
                );
                method
            },
            None => self.config.method,
        };

        match choose_method(method, text, self.types_like_us) {
            Chosen::Uinput => self.type_with_uinput(text),
            Chosen::Clipboard => match self.paste(text) {
                Ok(()) => Ok(()),
                Err(e) => {
                    warn!("Paste failed ({}); typing what uinput can", e);
                    self.type_with_uinput(text)
                },
            },
        }
    }

    fn type_with_uinput(&mut self, text: &str) -> Result<()> {
        wait_for_modifiers();
        self.uinput.set_typing_delay(self.config.typing_delay_ms);
        self.uinput.type_text(text)?;
        info!("Text inserted via uinput ({} chars)", text.chars().count());
        Ok(())
    }

    fn paste(&mut self, text: &str) -> Result<()> {
        let clock = SystemClock;
        let clipboard = self
            .clipboard
            .as_deref_mut()
            .ok_or_else(|| anyhow::anyhow!("clipboard unavailable"))?;
        let transcript = Transcript::new(text);
        let selection = Selection::Clipboard;
        let saved = clipboard::snapshot(clipboard, selection, &clock);
        clipboard
            .offer(selection, transcript.formats())
            .context("failed to set clipboard")?;
        wait_for_modifiers();
        let sent = self.uinput.send_paste_shortcut();
        if sent.is_ok() {
            clock.sleep(AFTER_PASTE);
        }
        let outcome = clipboard::restore(clipboard, selection, saved, &transcript, &clock);
        log_restore(selection, &outcome);
        sent?;
        info!(
            "Text inserted via clipboard ({} chars)",
            text.chars().count()
        );
        Ok(())
    }

    /// Leave text on the clipboard for the user to paste by hand.
    pub fn copy_to_clipboard(&mut self, text: &str) -> Result<()> {
        let clipboard = self
            .clipboard
            .as_deref_mut()
            .ok_or_else(|| anyhow::anyhow!("clipboard unavailable"))?;
        clipboard
            .offer(Selection::Clipboard, clipboard::text_formats(text))
            .context("failed to set clipboard")
    }

    /// Undo the last insertion by sending backspaces.
    pub fn undo_last_insertion(&mut self, char_count: usize) -> Result<()> {
        info!("Undoing last insertion ({} chars)", char_count);
        wait_for_modifiers();
        for _ in 0..char_count {
            self.uinput.send_backspace()?;
            thread::sleep(Duration::from_millis(self.config.typing_delay_ms));
        }
        Ok(())
    }

    pub fn test_insertion(&mut self) -> Result<()> {
        self.insert_text("Hello from Hush voice-to-text!")?;
        thread::sleep(Duration::from_millis(500));
        self.insert_text("\nThis includes: special-chars, numbers123, and symbols!@#")?;
        Ok(())
    }
}

fn wait_for_modifiers() {
    match modifiers::wait_for_release(&PhysicalModifiers, &SystemClock) {
        ModifierWait::Released { waited } if !waited.is_zero() => {
            debug!("Waited {:?} for the modifier keys to come up", waited)
        },
        ModifierWait::Released { .. } => {},
        ModifierWait::GaveUp => warn!(
            "A modifier key is still down after {:?}; inserting anyway",
            modifiers::CAP
        ),
        ModifierWait::Unknown => debug!("Modifier key state unknown"),
    }
}

fn log_restore(selection: Selection, outcome: &Restore) {
    match outcome {
        Restore::Restored => debug!("Restored the previous {}", selection),
        Restore::Cleared => debug!("Cleared the {}, which was empty before", selection),
        Restore::Kept(why) => warn!("The {} keeps the transcript: {}", selection, why),
        Restore::Replaced => info!(
            "The {} changed after the paste; Hush keeps the new content",
            selection
        ),
        Restore::CheckFailed(e) => warn!(
            "Could not check the {} after the paste ({}); it stays as it is",
            selection, e
        ),
        Restore::Failed(e) => warn!("Could not restore the {}: {}", selection, e),
    }
}

/// Explain how device access is granted.
pub fn print_uinput_setup_guidance() {
    println!("\n🔧 Device access for Hush");
    println!("═══════════════════════════");
    println!("Hush types through /dev/uinput and reads hotkeys from /dev/input.");
    println!("`hush setup permissions` installs one udev rule that grants both to the");
    println!("logged-in user through a polkit prompt, with no logout.\n");
    println!("The rule it installs, for reference or manual use:\n");
    print!("{}", crate::permissions::install_script());
}

/// Report the current device access state.
pub fn diagnose_uinput_issues() -> Result<()> {
    crate::permissions::print_status(&crate::permissions::status());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_types_plain_text_and_pastes_the_rest() {
        assert_eq!(
            choose_method(InsertionMethod::Auto, "hello, world!\n", true),
            Chosen::Uinput
        );
        assert_eq!(
            choose_method(InsertionMethod::Auto, "café", true),
            Chosen::Clipboard
        );
        assert_eq!(
            choose_method(InsertionMethod::Auto, "日本語", true),
            Chosen::Clipboard
        );
        let long = "a".repeat(PASTE_THRESHOLD_CHARS + 1);
        assert_eq!(
            choose_method(InsertionMethod::Auto, &long, true),
            Chosen::Clipboard
        );
    }

    #[test]
    fn explicit_methods_win() {
        assert_eq!(
            choose_method(InsertionMethod::Uinput, "café", true),
            Chosen::Uinput
        );
        assert_eq!(
            choose_method(InsertionMethod::Clipboard, "abc", true),
            Chosen::Clipboard
        );
    }

    #[test]
    fn a_non_us_layout_pastes_even_plain_text() {
        assert_eq!(
            choose_method(InsertionMethod::Auto, "hello", false),
            Chosen::Clipboard,
            "key codes would come out as other letters"
        );
        assert_eq!(
            choose_method(InsertionMethod::Uinput, "hello", false),
            Chosen::Uinput,
            "an explicit choice is still honoured"
        );
    }
}
