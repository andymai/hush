//! Puts transcribed text into the focused window.
//!
//! uinput types the text as key events, which works in every application,
//! VM, and password field. Text the US-layout key map cannot express (accents,
//! CJK, emoji) and very long text go through the clipboard: the previous
//! clipboard is saved, the text is pasted with a uinput Ctrl+V, and the
//! clipboard is restored.

use super::uinput_keyboard::UinputKeyboard;
use super::window::{WindowInfo, WindowProvider};
use super::InsertionMethod;
use crate::Result;
use std::thread;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Texts longer than this paste even when every character is typeable.
pub const PASTE_THRESHOLD_CHARS: usize = 2000;

/// The path a given text will take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chosen {
    Uinput,
    Clipboard,
}

/// `types_like_us` is false when the session's keyboard layout would turn
/// Hush's key codes into different characters.
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
    clipboard: Option<arboard::Clipboard>,
    windows: WindowProvider,
    method: InsertionMethod,
    typing_delay_ms: u64,
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
        let clipboard = match arboard::Clipboard::new() {
            Ok(clipboard) => Some(clipboard),
            Err(e) => {
                warn!("Clipboard unavailable, paste fallback disabled: {}", e);
                None
            },
        };
        let windows = WindowProvider::detect();
        let types_like_us = super::layout::types_like_us();
        info!(
            "Text insertion ready: {}, clipboard {}, window detection via {}",
            if types_like_us {
                "typed key by key"
            } else {
                "pasted, because the keyboard layout is not US"
            },
            if clipboard.is_some() { "ready" } else { "off" },
            windows.name()
        );
        Ok(Self {
            uinput,
            clipboard,
            windows,
            method: InsertionMethod::Auto,
            typing_delay_ms: 10,
            types_like_us,
        })
    }

    pub fn set_method(&mut self, method: InsertionMethod) {
        self.method = method;
    }

    pub fn set_typing_delay(&mut self, delay_ms: u64) {
        self.typing_delay_ms = delay_ms;
        self.uinput.set_typing_delay(delay_ms);
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
        if let Some(window) = self.windows.focused() {
            debug!("Target window: {} ({})", window.title, window.class);
        }

        match choose_method(self.method, text, self.types_like_us) {
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
        self.uinput.set_typing_delay(self.typing_delay_ms);
        self.uinput.type_text(text)?;
        info!("Text inserted via uinput ({} chars)", text.chars().count());
        Ok(())
    }

    fn paste(&mut self, text: &str) -> Result<()> {
        let clipboard = self
            .clipboard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("clipboard unavailable"))?;
        let previous = clipboard.get_text().ok();
        clipboard
            .set_text(text.to_string())
            .map_err(|e| anyhow::anyhow!("failed to set clipboard: {}", e))?;
        thread::sleep(Duration::from_millis(50));
        self.uinput.send_paste_shortcut()?;
        // Give the application time to read the selection before it changes.
        thread::sleep(Duration::from_millis(150));
        if let Some(previous) = previous {
            if let Err(e) = clipboard.set_text(previous) {
                warn!("Could not restore the previous clipboard: {}", e);
            }
        }
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
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("clipboard unavailable"))?;
        clipboard
            .set_text(text.to_string())
            .map_err(|e| anyhow::anyhow!("failed to set clipboard: {}", e))
    }

    /// Undo the last insertion by sending backspaces.
    pub fn undo_last_insertion(&mut self, char_count: usize) -> Result<()> {
        info!("Undoing last insertion ({} chars)", char_count);
        for _ in 0..char_count {
            self.uinput.send_backspace()?;
            thread::sleep(Duration::from_millis(self.typing_delay_ms));
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
