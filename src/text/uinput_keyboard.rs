/// Linux uinput-based keyboard emulation using input-linux crate
/// This provides hardware-level keyboard emulation that works with all applications
use super::keymap::{ChordKeys, KeyTable, Stroke, Tap, EXTRA_KEYS, MAIN_KEYS, MODIFIER_KEYS};
use super::typing::CapsLed;
use super::PasteChord;
use crate::Result;
use std::fs::File;
use std::os::unix::fs::OpenOptionsExt;
use std::thread;
use std::time::Duration;
use tracing::{debug, info, warn};

use input_linux::{
    sys, EvdevHandle, EventKind, EventTime, InputId, Key, KeyState, LedKind, SynchronizeKind,
    UInputHandle,
};

/// The lock LEDs the device has. The compositor sets them from the lock
/// state that the device's keys get, and Hush reads Caps Lock back.
const DEVICE_LEDS: [LedKind; 3] = [LedKind::NumLock, LedKind::CapsLock, LedKind::ScrollLock];
const LED_BYTES: usize = (sys::LED_CNT as usize).div_ceil(8);

fn led_on(bits: &[u8], led: LedKind) -> bool {
    let code = led as usize;
    bits.get(code / 8)
        .is_some_and(|byte| byte & (1 << (code % 8)) != 0)
}

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
    /// `table` is the active layout's keys, so the chord presses the key
    /// that types its letter there. `keys` hold Control and Shift in the
    /// session keymap.
    pub fn send_paste_shortcut(
        &mut self,
        chord: PasteChord,
        table: Option<&KeyTable>,
        keys: ChordKeys,
    ) -> Result<()> {
        if !self.ready {
            return Err(anyhow::anyhow!("Uinput keyboard not ready"));
        }
        let (modifiers, key) = chord_keys(chord, table, keys);
        self.send_chord(&modifiers, key)
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
        handle
            .set_evbit(EventKind::Led)
            .map_err(|e| anyhow::anyhow!("Failed to set LED event bit: {}", e))?;
        for led in DEVICE_LEDS {
            handle
                .set_ledbit(led)
                .map_err(|e| anyhow::anyhow!("Failed to enable LED {:?}: {}", led, e))?;
        }

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

    fn send_tap(&mut self, tap: Tap) -> Result<()> {
        self.send_chord(&tap.modifiers(), tap.key)
    }

    fn send_chord(&mut self, modifiers: &[Key], key: Key) -> Result<()> {
        match &mut self.device {
            Some(device) => press_chord(device, modifiers, key),
            None => Ok(()),
        }
    }
}

impl CapsLed for UinputKeyboard {
    /// Asks the kernel for the LEDs of the device's event node, which the
    /// compositor sets. The node opens for each query, so no typed key
    /// waits in its buffer.
    fn caps_lock(&mut self) -> Option<bool> {
        let path = self.device.as_ref()?.evdev_path().ok()?;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&path)
            .map_err(|e| debug!("Cannot read the LEDs of {}: {}", path.display(), e))
            .ok()?;
        let mut bits = [0u8; LED_BYTES];
        EvdevHandle::new(file)
            .led_state(&mut bits)
            .map_err(|e| debug!("Cannot read the LEDs of {}: {}", path.display(), e))
            .ok()?;
        Some(led_on(&bits, LedKind::CapsLock))
    }
}

/// Where key events go: the uinput device, or a recorder in the tests.
trait Events {
    fn key(&mut self, key: Key, state: KeyState) -> Result<()>;
    fn sync(&mut self) -> Result<()>;
}

impl Events for UInputHandle<File> {
    fn key(&mut self, key: Key, state: KeyState) -> Result<()> {
        write_event(self, EventKind::Key as u16, key as u16, state.value)
            .map_err(|e| anyhow::anyhow!("Failed to write input event: {}", e))
    }

    fn sync(&mut self) -> Result<()> {
        write_event(
            self,
            EventKind::Synchronize as u16,
            SynchronizeKind::Report as u16,
            0,
        )
        .map_err(|e| anyhow::anyhow!("Failed to write sync event: {}", e))
    }
}

fn write_event(
    device: &UInputHandle<File>,
    kind: u16,
    code: u16,
    value: i32,
) -> std::io::Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let time = EventTime::new(now.as_secs() as i64, (now.subsec_micros() as i64) * 1000);
    let event = sys::input_event {
        time: time.into_inner(),
        type_: kind,
        code,
        value,
    };
    device.write(&[event]).map(drop)
}

/// Hold the modifiers in order, press and release the key, and release the
/// modifiers in reverse. Every key that went down comes up again, also
/// when a write fails, so no modifier stays held after a character or a
/// paste.
fn press_chord(events: &mut impl Events, modifiers: &[Key], key: Key) -> Result<()> {
    let mut down = Vec::with_capacity(modifiers.len() + 1);
    let pressed = press_all(
        events,
        modifiers.iter().copied().chain(std::iter::once(key)),
        &mut down,
    );
    if pressed.is_ok() {
        thread::sleep(Duration::from_millis(1));
    }
    let mut sent = pressed;
    for &key in down.iter().rev() {
        let released = events.key(key, KeyState::RELEASED);
        sent = sent.and(released);
    }
    let synced = events.sync();
    sent.and(synced)
}

fn press_all(
    events: &mut impl Events,
    keys: impl Iterator<Item = Key>,
    down: &mut Vec<Key>,
) -> Result<()> {
    for key in keys {
        events.key(key, KeyState::PRESSED)?;
        down.push(key);
        events.sync()?;
    }
    Ok(())
}

impl Drop for UinputKeyboard {
    fn drop(&mut self) {
        if self.device.is_some() {
            info!("Cleaning up uinput keyboard device");
        }
    }
}

/// The modifiers and the key of a paste chord. The letter is the key that
/// types `v` on the active layout, so Ctrl+V stays Ctrl+V on Dvorak.
/// Without a plain key for `v`, as on a Cyrillic layout, it is the US V
/// key, which applications match to their Latin shortcuts. The modifiers
/// are the keys that hold Control and Shift, so Ctrl+V stays Ctrl+V when
/// an XKB option swaps Ctrl with another key.
fn chord_keys(chord: PasteChord, table: Option<&KeyTable>, keys: ChordKeys) -> (Vec<Key>, Key) {
    let v = table
        .and_then(|table| table.letter_key('v'))
        .unwrap_or(Key::V);
    match chord {
        PasteChord::CtrlV => (vec![keys.control], v),
        PasteChord::CtrlShiftV => (vec![keys.control, keys.shift], v),
        PasteChord::ShiftInsert => (vec![keys.shift], Key::Insert),
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
        for key in crate::text::xkb::CHORD_MODIFIER_KEYS {
            assert!(keys.contains(&key), "a paste chord can hold {key:?}");
        }
    }

    fn plain(chord: PasteChord, table: Option<&KeyTable>) -> (Vec<Key>, Key) {
        chord_keys(chord, table, ChordKeys::default())
    }

    #[test]
    fn each_paste_chord_presses_its_keys() {
        let us = Some(KeyTable::us());
        assert_eq!(plain(PasteChord::CtrlV, us), (vec![Key::LeftCtrl], Key::V));
        assert_eq!(
            plain(PasteChord::CtrlShiftV, None),
            (vec![Key::LeftCtrl, Key::LeftShift], Key::V)
        );
        assert_eq!(
            plain(PasteChord::ShiftInsert, us),
            (vec![Key::LeftShift], Key::Insert)
        );
    }

    fn table(v: Option<Tap>) -> KeyTable {
        let chars = v.map(|tap| ('v', vec![tap])).into_iter().collect();
        KeyTable::new(chars, Key::LeftShift)
    }

    #[test]
    fn the_led_bitmap_is_read_per_led() {
        let mut bits = [0u8; LED_BYTES];
        assert!(!led_on(&bits, LedKind::CapsLock));
        bits[0] = 1 << (LedKind::CapsLock as usize);
        assert!(led_on(&bits, LedKind::CapsLock));
        assert!(!led_on(&bits, LedKind::NumLock));
        assert!(!led_on(&[], LedKind::CapsLock));
    }

    #[test]
    fn with_caps_lock_on_the_chord_keeps_the_key_of_v() {
        let mut chars = std::collections::HashMap::new();
        chars.insert('V', vec![Tap::plain(Key::Dot)]);
        chars.insert('v', vec![Tap::shifted(Key::Dot)]);
        let dvorak_locked = KeyTable::new(chars, Key::LeftShift);
        assert_eq!(
            plain(PasteChord::CtrlV, Some(&dvorak_locked)),
            (vec![Key::LeftCtrl], Key::Dot),
            "with Caps Lock on, the plain key types the capital V"
        );
    }

    #[test]
    fn the_paste_chord_presses_the_key_that_types_v() {
        let dvorak = table(Some(Tap::plain(Key::Dot)));
        assert_eq!(
            plain(PasteChord::CtrlV, Some(&dvorak)),
            (vec![Key::LeftCtrl], Key::Dot),
            "V is on the US period key in Dvorak"
        );
        assert_eq!(plain(PasteChord::CtrlShiftV, Some(&dvorak)).1, Key::Dot);
        assert_eq!(plain(PasteChord::ShiftInsert, Some(&dvorak)).1, Key::Insert);
        let cyrillic = table(None);
        assert_eq!(plain(PasteChord::CtrlV, Some(&cyrillic)).1, Key::V);
        let shifted_only = table(Some(Tap::shifted(Key::Dot)));
        assert_eq!(
            plain(PasteChord::CtrlV, Some(&shifted_only)).1,
            Key::V,
            "only a plain key keeps the chord as it is"
        );
    }

    #[test]
    fn the_paste_chord_presses_the_keys_that_hold_control_and_shift() {
        let swapped = ChordKeys {
            control: Key::LeftMeta,
            shift: Key::RightShift,
        };
        let us = Some(KeyTable::us());
        assert_eq!(
            chord_keys(PasteChord::CtrlV, us, swapped),
            (vec![Key::LeftMeta], Key::V)
        );
        assert_eq!(
            chord_keys(PasteChord::CtrlShiftV, None, swapped),
            (vec![Key::LeftMeta, Key::RightShift], Key::V)
        );
        assert_eq!(
            chord_keys(PasteChord::ShiftInsert, us, swapped),
            (vec![Key::RightShift], Key::Insert)
        );
    }

    #[test]
    fn with_ctrl_and_win_swapped_ctrl_v_is_left_win_and_v() {
        use crate::text::xkb::{self, tests::xkb_ready, Names};
        if !xkb_ready() {
            return;
        }
        let names = Names {
            layout: "us".into(),
            options: "ctrl:swap_lwin_lctl,ctrl:swap_rwin_rctl".into(),
            ..Names::default()
        };
        let holders = xkb::holders(&[names], 0).unwrap();
        let keys = ChordKeys {
            control: holders.control.expect("a key holds Control"),
            shift: holders.shift.expect("a key holds Shift"),
        };
        let (modifiers, key) = chord_keys(PasteChord::CtrlV, Some(KeyTable::us()), keys);
        assert_eq!(
            (modifiers.as_slice(), key),
            (&[Key::LeftMeta][..], Key::V),
            "Left Ctrl types Super there, and Super+V opens Klipper"
        );
        let mut recorder = Recorder::default();
        press_chord(&mut recorder, &modifiers, key).expect("every write works");
        assert_eq!(
            recorder.events.first(),
            Some(&Some((Key::LeftMeta, KeyState::PRESSED.value)))
        );
        assert!(recorder.held().is_empty());
        let (modifiers, _) = chord_keys(PasteChord::CtrlShiftV, None, keys);
        assert_eq!(modifiers, [Key::LeftMeta, Key::LeftShift]);
    }

    /// Records events, and fails the write with the given number.
    #[derive(Default)]
    struct Recorder {
        tried: Vec<Option<(Key, i32)>>,
        events: Vec<Option<(Key, i32)>>,
        fail_at: Option<usize>,
    }

    impl Recorder {
        fn write(&mut self, event: Option<(Key, i32)>) -> Result<()> {
            self.tried.push(event);
            if self.fail_at == Some(self.tried.len()) {
                return Err(anyhow::anyhow!("write {} failed", self.tried.len()));
            }
            self.events.push(event);
            Ok(())
        }

        fn held(&self) -> Vec<Key> {
            let mut held = Vec::new();
            for &(key, value) in self.events.iter().flatten() {
                if value == KeyState::PRESSED.value {
                    held.push(key);
                } else {
                    held.retain(|&down| down != key);
                }
            }
            held
        }
    }

    impl Events for Recorder {
        fn key(&mut self, key: Key, state: KeyState) -> Result<()> {
            self.write(Some((key, state.value)))
        }

        fn sync(&mut self) -> Result<()> {
            self.write(None)
        }
    }

    #[test]
    fn a_chord_holds_its_modifiers_around_the_key() {
        let mut recorder = Recorder::default();
        press_chord(&mut recorder, &[Key::LeftCtrl, Key::LeftShift], Key::V)
            .expect("every write works");
        let down = KeyState::PRESSED.value;
        let up = KeyState::RELEASED.value;
        assert_eq!(
            recorder.events,
            [
                Some((Key::LeftCtrl, down)),
                None,
                Some((Key::LeftShift, down)),
                None,
                Some((Key::V, down)),
                None,
                Some((Key::V, up)),
                Some((Key::LeftShift, up)),
                Some((Key::LeftCtrl, up)),
                None,
            ]
        );
    }

    #[test]
    fn a_failed_write_still_releases_every_key() {
        let up = KeyState::RELEASED.value;
        for fail_at in 1..=10 {
            let mut recorder = Recorder {
                fail_at: Some(fail_at),
                ..Recorder::default()
            };
            let sent = press_chord(&mut recorder, &[Key::RightAlt, Key::LeftShift], Key::Q);
            assert!(sent.is_err(), "write {fail_at} fails");
            let failed = recorder.tried[fail_at - 1];
            for key in recorder.held() {
                assert_eq!(
                    failed,
                    Some((key, up)),
                    "write {fail_at} failed and left {key:?} held"
                );
            }
            for &(key, value) in recorder.events.iter().flatten() {
                if value != up {
                    assert!(
                        recorder.tried.contains(&Some((key, up))),
                        "write {fail_at} failed and {key:?} got no release"
                    );
                }
            }
        }
    }
}
