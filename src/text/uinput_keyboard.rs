/// Linux uinput-based keyboard emulation using input-linux crate
/// This provides hardware-level keyboard emulation that works with all applications
use crate::Result;
use std::collections::HashMap;
use std::fs::File;
use std::thread;
use std::time::Duration;
use tracing::{debug, info, warn};

use input_linux::{
    sys, EventKind, EventTime, InputId, Key, KeyState, SynchronizeKind, UInputHandle,
};

pub struct UinputKeyboard {
    device: Option<UInputHandle<File>>,
    char_to_key: HashMap<char, (Key, bool)>, // (key, needs_shift)
    typing_delay_ms: u64,
    ready: bool,
}

impl UinputKeyboard {
    pub fn new() -> Result<Self> {
        info!("🎹 Initializing uinput keyboard emulation");

        let mut keyboard = UinputKeyboard {
            device: None,
            char_to_key: HashMap::new(),
            typing_delay_ms: 20,
            ready: false,
        };

        // Build character mapping
        keyboard.build_char_mapping();

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

        // Create device info
        let device_id = InputId {
            bustype: 0x03, // USB
            vendor: 0x1234,
            product: 0x5678,
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
        if let Some((key, needs_shift)) = self.char_to_key.get(&ch) {
            self.send_key(*key, *needs_shift)?;
        } else {
            debug!("Character '{}' not in basic mapping, skipping", ch);
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

    fn build_char_mapping(&mut self) {
        // Lowercase letters
        let lowercase = [
            ('a', Key::A),
            ('b', Key::B),
            ('c', Key::C),
            ('d', Key::D),
            ('e', Key::E),
            ('f', Key::F),
            ('g', Key::G),
            ('h', Key::H),
            ('i', Key::I),
            ('j', Key::J),
            ('k', Key::K),
            ('l', Key::L),
            ('m', Key::M),
            ('n', Key::N),
            ('o', Key::O),
            ('p', Key::P),
            ('q', Key::Q),
            ('r', Key::R),
            ('s', Key::S),
            ('t', Key::T),
            ('u', Key::U),
            ('v', Key::V),
            ('w', Key::W),
            ('x', Key::X),
            ('y', Key::Y),
            ('z', Key::Z),
        ];

        for (ch, key) in &lowercase {
            self.char_to_key.insert(*ch, (*key, false));
        }

        // Uppercase letters (same keys with shift)
        let uppercase = [
            ('A', Key::A),
            ('B', Key::B),
            ('C', Key::C),
            ('D', Key::D),
            ('E', Key::E),
            ('F', Key::F),
            ('G', Key::G),
            ('H', Key::H),
            ('I', Key::I),
            ('J', Key::J),
            ('K', Key::K),
            ('L', Key::L),
            ('M', Key::M),
            ('N', Key::N),
            ('O', Key::O),
            ('P', Key::P),
            ('Q', Key::Q),
            ('R', Key::R),
            ('S', Key::S),
            ('T', Key::T),
            ('U', Key::U),
            ('V', Key::V),
            ('W', Key::W),
            ('X', Key::X),
            ('Y', Key::Y),
            ('Z', Key::Z),
        ];

        for (ch, key) in &uppercase {
            self.char_to_key.insert(*ch, (*key, true));
        }

        // Numbers
        let numbers = [
            ('1', Key::Num1),
            ('2', Key::Num2),
            ('3', Key::Num3),
            ('4', Key::Num4),
            ('5', Key::Num5),
            ('6', Key::Num6),
            ('7', Key::Num7),
            ('8', Key::Num8),
            ('9', Key::Num9),
            ('0', Key::Num0),
        ];

        for (ch, key) in &numbers {
            self.char_to_key.insert(*ch, (*key, false));
        }

        // Basic symbols
        self.char_to_key.insert(' ', (Key::Space, false));
        self.char_to_key.insert('.', (Key::Dot, false));
        self.char_to_key.insert(',', (Key::Comma, false));
        self.char_to_key.insert(';', (Key::Semicolon, false));
        self.char_to_key.insert('\'', (Key::Apostrophe, false));
        self.char_to_key.insert('/', (Key::Slash, false));
        self.char_to_key.insert('\\', (Key::Backslash, false));
        self.char_to_key.insert('-', (Key::Minus, false));
        self.char_to_key.insert('=', (Key::Equal, false));
        self.char_to_key.insert('[', (Key::LeftBrace, false));
        self.char_to_key.insert(']', (Key::RightBrace, false));
        self.char_to_key.insert('`', (Key::Grave, false));

        // Symbols with shift
        self.char_to_key.insert('!', (Key::Num1, true));
        self.char_to_key.insert('@', (Key::Num2, true));
        self.char_to_key.insert('#', (Key::Num3, true));
        self.char_to_key.insert('$', (Key::Num4, true));
        self.char_to_key.insert('%', (Key::Num5, true));
        self.char_to_key.insert('^', (Key::Num6, true));
        self.char_to_key.insert('&', (Key::Num7, true));
        self.char_to_key.insert('*', (Key::Num8, true));
        self.char_to_key.insert('(', (Key::Num9, true));
        self.char_to_key.insert(')', (Key::Num0, true));
        self.char_to_key.insert('_', (Key::Minus, true));
        self.char_to_key.insert('+', (Key::Equal, true));
        self.char_to_key.insert('{', (Key::LeftBrace, true));
        self.char_to_key.insert('}', (Key::RightBrace, true));
        self.char_to_key.insert('|', (Key::Backslash, true));
        self.char_to_key.insert(':', (Key::Semicolon, true));
        self.char_to_key.insert('"', (Key::Apostrophe, true));
        self.char_to_key.insert('<', (Key::Comma, true));
        self.char_to_key.insert('>', (Key::Dot, true));
        self.char_to_key.insert('?', (Key::Slash, true));
        self.char_to_key.insert('~', (Key::Grave, true));
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
        let mut keyboard = UinputKeyboard {
            device: None,
            char_to_key: HashMap::new(),
            typing_delay_ms: 0,
            ready: false,
        };

        keyboard.build_char_mapping();

        assert_eq!(keyboard.char_to_key.get(&'a'), Some(&(Key::A, false)));
        assert_eq!(keyboard.char_to_key.get(&'A'), Some(&(Key::A, true)));
        assert_eq!(keyboard.char_to_key.get(&'1'), Some(&(Key::Num1, false)));
        assert_eq!(keyboard.char_to_key.get(&'!'), Some(&(Key::Num1, true)));
        assert_eq!(keyboard.char_to_key.get(&' '), Some(&(Key::Space, false)));
    }
}
