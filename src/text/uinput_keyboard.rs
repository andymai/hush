/// Linux uinput-based keyboard emulation using input-linux crate
/// This provides hardware-level keyboard emulation that works with all applications
use crate::Result;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fs::File;
use std::thread;
use std::time::Duration;
use tracing::{debug, info, warn};

use input_linux::{
    sys, EventKind, EventTime, InputId, Key, KeyState, SynchronizeKind, UInputHandle,
};

/// Static character to key mapping for efficient lookup
/// Built once on first access and reused for all keyboard instances
static CHAR_TO_KEY_MAP: Lazy<HashMap<char, (Key, bool)>> = Lazy::new(|| {
    let mut map = HashMap::with_capacity(100);

    // Lowercase letters
    map.insert('a', (Key::A, false));
    map.insert('b', (Key::B, false));
    map.insert('c', (Key::C, false));
    map.insert('d', (Key::D, false));
    map.insert('e', (Key::E, false));
    map.insert('f', (Key::F, false));
    map.insert('g', (Key::G, false));
    map.insert('h', (Key::H, false));
    map.insert('i', (Key::I, false));
    map.insert('j', (Key::J, false));
    map.insert('k', (Key::K, false));
    map.insert('l', (Key::L, false));
    map.insert('m', (Key::M, false));
    map.insert('n', (Key::N, false));
    map.insert('o', (Key::O, false));
    map.insert('p', (Key::P, false));
    map.insert('q', (Key::Q, false));
    map.insert('r', (Key::R, false));
    map.insert('s', (Key::S, false));
    map.insert('t', (Key::T, false));
    map.insert('u', (Key::U, false));
    map.insert('v', (Key::V, false));
    map.insert('w', (Key::W, false));
    map.insert('x', (Key::X, false));
    map.insert('y', (Key::Y, false));
    map.insert('z', (Key::Z, false));

    // Uppercase letters (same keys with shift)
    map.insert('A', (Key::A, true));
    map.insert('B', (Key::B, true));
    map.insert('C', (Key::C, true));
    map.insert('D', (Key::D, true));
    map.insert('E', (Key::E, true));
    map.insert('F', (Key::F, true));
    map.insert('G', (Key::G, true));
    map.insert('H', (Key::H, true));
    map.insert('I', (Key::I, true));
    map.insert('J', (Key::J, true));
    map.insert('K', (Key::K, true));
    map.insert('L', (Key::L, true));
    map.insert('M', (Key::M, true));
    map.insert('N', (Key::N, true));
    map.insert('O', (Key::O, true));
    map.insert('P', (Key::P, true));
    map.insert('Q', (Key::Q, true));
    map.insert('R', (Key::R, true));
    map.insert('S', (Key::S, true));
    map.insert('T', (Key::T, true));
    map.insert('U', (Key::U, true));
    map.insert('V', (Key::V, true));
    map.insert('W', (Key::W, true));
    map.insert('X', (Key::X, true));
    map.insert('Y', (Key::Y, true));
    map.insert('Z', (Key::Z, true));

    // Numbers
    map.insert('1', (Key::Num1, false));
    map.insert('2', (Key::Num2, false));
    map.insert('3', (Key::Num3, false));
    map.insert('4', (Key::Num4, false));
    map.insert('5', (Key::Num5, false));
    map.insert('6', (Key::Num6, false));
    map.insert('7', (Key::Num7, false));
    map.insert('8', (Key::Num8, false));
    map.insert('9', (Key::Num9, false));
    map.insert('0', (Key::Num0, false));

    // Basic symbols
    map.insert(' ', (Key::Space, false));
    map.insert('.', (Key::Dot, false));
    map.insert(',', (Key::Comma, false));
    map.insert(';', (Key::Semicolon, false));
    map.insert('\'', (Key::Apostrophe, false));
    map.insert('/', (Key::Slash, false));
    map.insert('\\', (Key::Backslash, false));
    map.insert('-', (Key::Minus, false));
    map.insert('=', (Key::Equal, false));
    map.insert('[', (Key::LeftBrace, false));
    map.insert(']', (Key::RightBrace, false));
    map.insert('`', (Key::Grave, false));

    // Symbols with shift
    map.insert('!', (Key::Num1, true));
    map.insert('@', (Key::Num2, true));
    map.insert('#', (Key::Num3, true));
    map.insert('$', (Key::Num4, true));
    map.insert('%', (Key::Num5, true));
    map.insert('^', (Key::Num6, true));
    map.insert('&', (Key::Num7, true));
    map.insert('*', (Key::Num8, true));
    map.insert('(', (Key::Num9, true));
    map.insert(')', (Key::Num0, true));
    map.insert('_', (Key::Minus, true));
    map.insert('+', (Key::Equal, true));
    map.insert('{', (Key::LeftBrace, true));
    map.insert('}', (Key::RightBrace, true));
    map.insert('|', (Key::Backslash, true));
    map.insert(':', (Key::Semicolon, true));
    map.insert('"', (Key::Apostrophe, true));
    map.insert('<', (Key::Comma, true));
    map.insert('>', (Key::Dot, true));
    map.insert('?', (Key::Slash, true));
    map.insert('~', (Key::Grave, true));

    map
});

pub struct UinputKeyboard {
    device: Option<UInputHandle<File>>,
    typing_delay_ms: u64,
    ready: bool,
}

impl UinputKeyboard {
    pub fn new() -> Result<Self> {
        info!("🎹 Initializing uinput keyboard emulation");

        let mut keyboard = UinputKeyboard {
            device: None,
            typing_delay_ms: 20,
            ready: false,
        };

        // Try to create the device
        match keyboard.create_device() {
            Ok(()) => {
                keyboard.ready = true;
                info!("✅ Uinput keyboard emulation ready");
            },
            Err(e) => {
                warn!("⚠️ Failed to create uinput device: {}", e);
                warn!(
                    "This is likely due to permissions. See documentation for setup instructions."
                );
                // Don't fail completely - we can still be used as a capability check
            },
        }

        Ok(keyboard)
    }

    pub fn is_ready(&self) -> bool {
        self.ready
    }

    /// Whether the US-layout key map covers every character.
    pub fn can_type_all(text: &str) -> bool {
        text.chars()
            .all(|ch| matches!(ch, '\n' | '\t') || CHAR_TO_KEY_MAP.contains_key(&ch))
    }

    /// Ctrl+V
    pub fn send_paste_shortcut(&mut self) -> Result<()> {
        if !self.ready {
            return Err(anyhow::anyhow!("Uinput keyboard not ready"));
        }
        self.send_key_event(Key::LeftCtrl, KeyState::PRESSED)?;
        self.send_sync()?;
        self.send_key(Key::V, false)?;
        self.send_key_event(Key::LeftCtrl, KeyState::RELEASED)?;
        self.send_sync()
    }

    pub fn set_typing_delay(&mut self, delay_ms: u64) {
        self.typing_delay_ms = delay_ms;
        debug!("Typing delay set to {}ms", delay_ms);
    }

    pub fn type_text(&mut self, text: &str) -> Result<()> {
        if !self.ready {
            return Err(anyhow::anyhow!("Uinput keyboard not ready"));
        }

        info!(
            "⌨️ Typing text via uinput: '{}' ({} chars)",
            text.chars().take(50).collect::<String>(),
            text.len()
        );

        for (i, ch) in text.char_indices() {
            if i > 0 && i % 100 == 0 {
                debug!("Typed {} characters so far", i);
            }

            match ch {
                '\n' => self.send_key(Key::Enter, false)?,
                '\t' => self.send_key(Key::Tab, false)?,
                _ => self.type_character(ch)?,
            }

            if self.typing_delay_ms > 0 {
                thread::sleep(Duration::from_millis(self.typing_delay_ms));
            }
        }

        info!(
            "✅ Text typed successfully via uinput ({} characters)",
            text.len()
        );
        Ok(())
    }

    /// Send a backspace keystroke
    pub fn send_backspace(&mut self) -> Result<()> {
        if !self.ready {
            return Err(anyhow::anyhow!("Uinput keyboard not ready"));
        }

        self.send_key(Key::Backspace, false)
    }

    pub fn get_device_info(&self) -> String {
        if self.ready {
            "Linux uinput (kernel-level)".to_string()
        } else {
            "Linux uinput (not available)".to_string()
        }
    }

    fn create_device(&mut self) -> Result<()> {
        info!("Creating uinput keyboard device...");

        let device_id = InputId {
            bustype: sys::BUS_VIRTUAL,
            vendor: crate::hotkey::evdev::VIRTUAL_VENDOR,
            product: crate::hotkey::evdev::VIRTUAL_PRODUCT_KEYBOARD,
            version: 1,
        };

        // Open uinput device
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/uinput")
            .map_err(|e| {
                anyhow::anyhow!("Failed to open /dev/uinput: {}. Check permissions.", e)
            })?;

        // Create UInput handle
        let handle = UInputHandle::new(file);

        // Set up device capabilities
        handle
            .set_evbit(EventKind::Key)
            .map_err(|e| anyhow::anyhow!("Failed to set key event bit: {}", e))?;
        handle
            .set_evbit(EventKind::Synchronize)
            .map_err(|e| anyhow::anyhow!("Failed to set sync event bit: {}", e))?;

        // Enable all the keys we need
        let keys_to_enable = [
            Key::A,
            Key::B,
            Key::C,
            Key::D,
            Key::E,
            Key::F,
            Key::G,
            Key::H,
            Key::I,
            Key::J,
            Key::K,
            Key::L,
            Key::M,
            Key::N,
            Key::O,
            Key::P,
            Key::Q,
            Key::R,
            Key::S,
            Key::T,
            Key::U,
            Key::V,
            Key::W,
            Key::X,
            Key::Y,
            Key::Z,
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
            Key::Space,
            Key::Enter,
            Key::Tab,
            Key::Backspace,
            Key::Dot,
            Key::Comma,
            Key::Semicolon,
            Key::Apostrophe,
            Key::Slash,
            Key::Backslash,
            Key::Minus,
            Key::Equal,
            Key::LeftBrace,
            Key::RightBrace,
            Key::Grave,
            Key::LeftShift,
            Key::RightShift,
            Key::LeftCtrl,
            Key::RightCtrl,
            Key::LeftAlt,
            Key::RightAlt,
        ];

        for &key in &keys_to_enable {
            handle
                .set_keybit(key)
                .map_err(|e| anyhow::anyhow!("Failed to enable key {:?}: {}", key, e))?;
        }

        // Create the device
        let device_name = b"Hush Voice Keyboard";
        handle
            .create(&device_id, device_name, 0, &[])
            .map_err(|e| anyhow::anyhow!("Failed to create device: {}", e))?;

        self.device = Some(handle);
        info!("✅ Uinput device created successfully");

        Ok(())
    }

    fn type_character(&mut self, ch: char) -> Result<()> {
        if let Some((key, needs_shift)) = CHAR_TO_KEY_MAP.get(&ch) {
            self.send_key(*key, *needs_shift)?;
        } else {
            warn!("Character '{}' is not in the US key map, skipping", ch);
        }
        Ok(())
    }

    fn send_key(&mut self, key: Key, with_shift: bool) -> Result<()> {
        if let Some(_device) = &mut self.device {
            // Press shift if needed
            if with_shift {
                self.send_key_event(Key::LeftShift, KeyState::PRESSED)?;
                self.send_sync()?;
            }

            // Press the key
            self.send_key_event(key, KeyState::PRESSED)?;
            self.send_sync()?;

            // Small delay
            thread::sleep(Duration::from_millis(1));

            // Release the key
            self.send_key_event(key, KeyState::RELEASED)?;

            // Release shift if we pressed it
            if with_shift {
                self.send_key_event(Key::LeftShift, KeyState::RELEASED)?;
            }

            self.send_sync()?;
        }
        Ok(())
    }

    fn send_key_event(&mut self, key: Key, state: KeyState) -> Result<()> {
        if let Some(device) = &mut self.device {
            // Create event using current timestamp
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            let time = EventTime::new(now.as_secs() as i64, (now.subsec_micros() as i64) * 1000);

            let event = sys::input_event {
                time: time.into_inner(),
                type_: EventKind::Key as u16,
                code: key as u16,
                value: state.value,
            };

            device
                .write(&[event])
                .map_err(|e| anyhow::anyhow!("Failed to write input event: {}", e))?;
        }
        Ok(())
    }

    fn send_sync(&mut self) -> Result<()> {
        if let Some(device) = &mut self.device {
            // Create sync event using current timestamp
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            let time = EventTime::new(now.as_secs() as i64, (now.subsec_micros() as i64) * 1000);

            let event = sys::input_event {
                time: time.into_inner(),
                type_: EventKind::Synchronize as u16,
                code: SynchronizeKind::Report as u16,
                value: 0,
            };

            device
                .write(&[event])
                .map_err(|e| anyhow::anyhow!("Failed to write sync event: {}", e))?;
        }
        Ok(())
    }
}

impl Drop for UinputKeyboard {
    fn drop(&mut self) {
        if self.device.is_some() {
            info!("Cleaning up uinput keyboard device");
        }
    }
}

// Helper function to check uinput availability
pub fn check_uinput_availability() -> Result<()> {
    use std::path::Path;

    if !Path::new("/dev/uinput").exists() {
        return Err(anyhow::anyhow!("uinput device /dev/uinput not found"));
    }

    match std::fs::OpenOptions::new().write(true).open("/dev/uinput") {
        Ok(_) => {
            info!("✅ uinput device is accessible");
            Ok(())
        },
        Err(e) => Err(anyhow::anyhow!(
            "Cannot access /dev/uinput: {}. Check permissions.",
            e
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_char_mapping() {
        // Test static character mapping
        assert_eq!(CHAR_TO_KEY_MAP.get(&'a'), Some(&(Key::A, false)));
        assert_eq!(CHAR_TO_KEY_MAP.get(&'A'), Some(&(Key::A, true)));
        assert_eq!(CHAR_TO_KEY_MAP.get(&'1'), Some(&(Key::Num1, false)));
        assert_eq!(CHAR_TO_KEY_MAP.get(&'!'), Some(&(Key::Num1, true)));
        assert_eq!(CHAR_TO_KEY_MAP.get(&' '), Some(&(Key::Space, false)));
    }
}
