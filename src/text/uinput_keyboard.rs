/// Linux uinput-based keyboard emulation using input-linux crate
/// This provides hardware-level keyboard emulation that works with all applications
use super::keymap::{KeyTable, Stroke, Tap, EXTRA_KEYS, MAIN_KEYS, MODIFIER_KEYS};
use super::PasteChord;
use crate::Result;
use std::fs::File;
use std::thread;
use std::time::Duration;
use tracing::{debug, info, warn};

use input_linux::{
    sys, EventKind, EventTime, InputId, Key, KeyState, SynchronizeKind, UInputHandle,
};

/// The keys a typed line break presses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineBreak {
    #[default]
    Enter,
    /// Chat applications send the message on Enter and start a new line on
    /// Shift+Enter.
    ShiftEnter,
}

/// Every key the device can press: the keys that type characters on some
/// layout, the keys that can hold Shift or AltGr, and the editing and paste
/// keys.
fn device_keys() -> Vec<Key> {
    let mut keys: Vec<Key> = MAIN_KEYS
        .iter()
        .chain(&EXTRA_KEYS)
        .chain(&MODIFIER_KEYS)
        .chain(&[
            Key::Enter,
            Key::Tab,
            Key::Backspace,
            Key::Insert,
            Key::LeftCtrl,
            Key::RightCtrl,
        ])
        .copied()
        .collect();
    keys.sort_by_key(|key| key.code());
    keys.dedup();
    keys
}

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

    /// Hold the chord's modifiers, tap its key, and release the modifiers.
    pub fn send_paste_shortcut(&mut self, chord: PasteChord) -> Result<()> {
        if !self.ready {
            return Err(anyhow::anyhow!("Uinput keyboard not ready"));
        }
        let (modifiers, key) = chord_keys(chord);
        for modifier in modifiers {
            self.send_key_event(*modifier, KeyState::PRESSED)?;
            self.send_sync()?;
        }
        self.send_tap(Tap::plain(key))?;
        for modifier in modifiers.iter().rev() {
            self.send_key_event(*modifier, KeyState::RELEASED)?;
        }
        self.send_sync()
    }

    pub fn set_typing_delay(&mut self, delay_ms: u64) {
        self.typing_delay_ms = delay_ms;
        debug!("Typing delay set to {}ms", delay_ms);
    }

    /// Type a text with the table's keys. A character the table has no
    /// keys for is skipped.
    pub fn type_text(&mut self, text: &str, table: &KeyTable, line_break: LineBreak) -> Result<()> {
        if !self.ready {
            return Err(anyhow::anyhow!("Uinput keyboard not ready"));
        }

        let count = text.chars().count();
        info!("⌨️ Typing {} characters via uinput", count);

        let mut skipped = 0;
        for (i, stroke) in table.strokes(text, line_break).into_iter().enumerate() {
            if i > 0 && i % 100 == 0 {
                debug!("Typed {} keys so far", i);
            }

            match stroke {
                Stroke::Tap(tap) => self.send_tap(tap)?,
                Stroke::Unmapped(ch) => {
                    skipped += 1;
                    debug!("No key types '{}' on this layout; skipping it", ch);
                },
            }

            if self.typing_delay_ms > 0 {
                thread::sleep(Duration::from_millis(self.typing_delay_ms));
            }
        }

        if skipped > 0 {
            warn!(
                "Skipped {} characters that no key on this layout types",
                skipped
            );
        }
        info!(
            "✅ Text typed successfully via uinput ({} characters)",
            count
        );
        Ok(())
    }

    /// Send a backspace keystroke
    pub fn send_backspace(&mut self) -> Result<()> {
        if !self.ready {
            return Err(anyhow::anyhow!("Uinput keyboard not ready"));
        }

        self.send_tap(Tap::plain(Key::Backspace))
    }

    pub fn send_enter(&mut self) -> Result<()> {
        if !self.ready {
            return Err(anyhow::anyhow!("Uinput keyboard not ready"));
        }

        self.send_tap(Tap::plain(Key::Enter))
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

        for key in device_keys() {
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

    /// Hold the tap's modifiers, press and release its key, and release the
    /// modifiers, so no modifier stays down after a character.
    fn send_tap(&mut self, tap: Tap) -> Result<()> {
        if self.device.is_none() {
            return Ok(());
        }
        let modifiers = tap.modifiers();
        let mut held = 0;
        let mut sent = (|| {
            for &modifier in &modifiers {
                self.send_key_event(modifier, KeyState::PRESSED)?;
                held += 1;
                self.send_sync()?;
            }
            self.send_key_event(tap.key, KeyState::PRESSED)?;
            self.send_sync()?;
            thread::sleep(Duration::from_millis(1));
            self.send_key_event(tap.key, KeyState::RELEASED)
        })();
        for &modifier in modifiers[..held].iter().rev() {
            let released = self.send_key_event(modifier, KeyState::RELEASED);
            sent = sent.and(released);
        }
        sent.and(self.send_sync())
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

/// The modifiers and the key of a paste chord.
fn chord_keys(chord: PasteChord) -> (&'static [Key], Key) {
    match chord {
        PasteChord::CtrlV => (&[Key::LeftCtrl], Key::V),
        PasteChord::CtrlShiftV => (&[Key::LeftCtrl, Key::LeftShift], Key::V),
        PasteChord::ShiftInsert => (&[Key::LeftShift], Key::Insert),
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
    fn the_device_has_every_key_a_table_presses() {
        let keys = device_keys();
        for key in MAIN_KEYS.iter().chain(&EXTRA_KEYS).chain(&MODIFIER_KEYS) {
            assert!(keys.contains(key), "{key:?}");
        }
        for key in [
            Key::Enter,
            Key::Tab,
            Key::Backspace,
            Key::Insert,
            Key::LeftCtrl,
        ] {
            assert!(keys.contains(&key), "{key:?}");
        }
        let mut sorted = keys.clone();
        sorted.dedup();
        assert_eq!(sorted, keys, "no key twice");
    }

    #[test]
    fn each_paste_chord_presses_its_keys() {
        assert_eq!(
            chord_keys(PasteChord::CtrlV),
            (&[Key::LeftCtrl][..], Key::V)
        );
        assert_eq!(
            chord_keys(PasteChord::CtrlShiftV),
            (&[Key::LeftCtrl, Key::LeftShift][..], Key::V)
        );
        assert_eq!(
            chord_keys(PasteChord::ShiftInsert),
            (&[Key::LeftShift][..], Key::Insert)
        );
    }
}
