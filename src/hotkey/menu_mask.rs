//! The menu mask. The evdev reader does not grab the keyboard, so the
//! focused application also sees the hotkey. When a lone Alt key goes down
//! and up, many applications open their menu bar. A lone Super key opens
//! the launcher of many desktops. The typed text then lands in the menu.
//! A Ctrl tap from a small virtual keyboard while the key is down turns the
//! lone modifier into a chord, so nothing opens. The tap presses the key
//! that holds Control in the session keymap. An XKB option can move
//! Control: with `ctrl:swapcaps`, the Left Ctrl key types Caps Lock. When
//! Hush cannot read the options, as on Sway, it sends no tap.
//!
//! The tap comes from a device of its own, not from the typing keyboard.
//! The hotkey reader thread writes the tap at once, also while a take
//! types. The hotkey reader skips the device, as it skips each device of
//! Hush.

use super::evdev::{Processed, VIRTUAL_PRODUCT_MASK, VIRTUAL_VENDOR};
use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, BusType, EventType, InputEvent, InputId, KeyCode};
use std::io;

/// Where the tap goes: the virtual keyboard, or a recorder in the tests.
pub trait Emit {
    /// Write the events, and a `SYN_REPORT` after them.
    fn emit(&mut self, events: &[InputEvent]) -> io::Result<()>;
}

impl Emit for VirtualDevice {
    fn emit(&mut self, events: &[InputEvent]) -> io::Result<()> {
        VirtualDevice::emit(self, events)
    }
}

pub struct MenuMask<D: Emit = VirtualDevice> {
    device: D,
    /// The key that holds Control.
    key: KeyCode,
}

impl MenuMask {
    /// A virtual keyboard with only `key`, the key that holds Control. Make
    /// it when the reader starts, because the desktop needs a moment to find
    /// a new device.
    pub fn new(key: KeyCode) -> io::Result<Self> {
        let mut keys = AttributeSet::<KeyCode>::new();
        keys.insert(key);
        let id = InputId::new(
            BusType::BUS_VIRTUAL,
            VIRTUAL_VENDOR,
            VIRTUAL_PRODUCT_MASK,
            1,
        );
        let device = VirtualDevice::builder()?
            .name("Hush Menu Mask")
            .input_id(id)
            .with_keys(&keys)?
            .build()?;
        Ok(Self { device, key })
    }
}

impl<D: Emit> MenuMask<D> {
    /// Tap Ctrl when the event is the press of a chord that needs the mask.
    pub fn follow(&mut self, processed: &Processed) -> io::Result<()> {
        if processed.mask {
            self.tap()
        } else {
            Ok(())
        }
    }

    /// Press Ctrl, sync, release Ctrl, sync. The release goes out also when
    /// the press fails, so Ctrl never stays down for the text that Hush
    /// types later.
    fn tap(&mut self) -> io::Result<()> {
        let pressed = self.device.emit(&[self.event(1)]);
        let released = self.device.emit(&[self.event(0)]);
        pressed.and(released)
    }

    fn event(&self, value: i32) -> InputEvent {
        InputEvent::new(EventType::KEY.0, self.key.code(), value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hotkey::evdev::Matchers;
    use crate::hotkey::HotkeyBindings;

    const SYN: (u16, u16, i32) = (0, 0, 0);
    const KEY: KeyCode = KeyCode::KEY_LEFTCTRL;

    /// Records each event as (type, code, value), with a SYN_REPORT after
    /// each emit, and fails the emits in `fail`.
    #[derive(Default)]
    struct Recorder {
        events: Vec<(u16, u16, i32)>,
        emits: usize,
        fail: Vec<usize>,
    }

    impl Emit for Recorder {
        fn emit(&mut self, events: &[InputEvent]) -> io::Result<()> {
            self.emits += 1;
            if self.fail.contains(&self.emits) {
                return Err(io::Error::other("emit failed"));
            }
            for event in events {
                self.events
                    .push((event.event_type().0, event.code(), event.value()));
            }
            self.events.push(SYN);
            Ok(())
        }
    }

    impl Recorder {
        fn taps(&self) -> usize {
            self.events
                .iter()
                .filter(|event| **event == (EventType::KEY.0, KEY.code(), 1))
                .count()
        }
    }

    fn mask() -> MenuMask<Recorder> {
        MenuMask {
            device: Recorder::default(),
            key: KEY,
        }
    }

    fn matchers(primary: &str, mask_menu: bool) -> Matchers {
        Matchers::new(
            &HotkeyBindings::parse(primary, Some("Escape")).unwrap(),
            mask_menu,
        )
    }

    #[test]
    fn a_tap_is_a_ctrl_press_then_a_release() {
        let mut mask = mask();
        mask.tap().unwrap();
        let ctrl = KeyCode::KEY_LEFTCTRL.code();
        assert_eq!(
            mask.device.events,
            [
                (EventType::KEY.0, ctrl, 1),
                SYN,
                (EventType::KEY.0, ctrl, 0),
                SYN
            ]
        );

        let mut moved = MenuMask {
            device: Recorder::default(),
            key: KeyCode::KEY_LEFTMETA,
        };
        moved.tap().unwrap();
        let meta = KeyCode::KEY_LEFTMETA.code();
        assert_eq!(
            moved.device.events,
            [
                (EventType::KEY.0, meta, 1),
                SYN,
                (EventType::KEY.0, meta, 0),
                SYN
            ],
            "the key that holds Control in the keymap"
        );
    }

    #[test]
    fn a_failed_press_still_releases_ctrl() {
        let mut mask = MenuMask {
            device: Recorder {
                fail: vec![1],
                ..Recorder::default()
            },
            key: KEY,
        };
        assert!(mask.tap().is_err());
        assert_eq!(mask.device.events, [(EventType::KEY.0, KEY.code(), 0), SYN]);
    }

    #[test]
    fn the_hotkey_taps_on_its_press_only() {
        let mut m = matchers("RightAlt", true);
        assert!(m.masks());
        let mut mask = mask();
        for (value, taps) in [(1, 1), (2, 1), (2, 1), (0, 1), (1, 2), (0, 2)] {
            let processed = m.process(KeyCode::KEY_RIGHTALT, value);
            mask.follow(&processed).unwrap();
            assert_eq!(mask.device.taps(), taps, "after value {value}");
        }
    }

    #[test]
    fn only_a_chord_of_alt_and_super_keys_gets_the_mask() {
        for (primary, masked) in [
            ("RightAlt", true),
            ("LeftSuper", true),
            ("Super+RightAlt", true),
            ("Ctrl+RightAlt", false),
            ("Shift+LeftAlt", false),
            ("F13", false),
            ("Mouse4", false),
        ] {
            let mut m = matchers(primary, true);
            assert_eq!(m.masks(), masked, "{primary}");
            let combination = crate::hotkey::KeyCombination::parse(primary).unwrap();
            if combination.modifiers.ctrl {
                m.process(KeyCode::KEY_LEFTCTRL, 1);
            }
            if combination.modifiers.shift {
                m.process(KeyCode::KEY_LEFTSHIFT, 1);
            }
            if combination.modifiers.super_key {
                m.process(KeyCode::KEY_LEFTMETA, 1);
            }
            let press = m.process(combination.evdev_key(), 1);
            assert_eq!(press.event, Some(crate::hotkey::HotkeyEvent::Pressed));
            assert_eq!(press.mask, masked, "{primary}");
        }
    }

    #[test]
    fn a_bare_alt_or_super_chord_of_any_kind_gets_the_mask() {
        let german = crate::transcription::Language::parse("de").unwrap();
        let formal = crate::text_processing::StyleName::parse("formal").unwrap();
        let bindings = HotkeyBindings::parse("F13", Some("LeftAlt"))
            .unwrap()
            .with_language(german, "RightAlt")
            .unwrap()
            .with_style(crate::text_processing::styles::VERBATIM, "RightSuper")
            .unwrap()
            .with_action(
                crate::hotkey::HotkeyAction::Rewrite(formal),
                "Super+LeftAlt",
            )
            .unwrap()
            .with_command("Alt+LeftSuper")
            .unwrap();
        let mut m = Matchers::new(&bindings, true);

        assert!(m.process(KeyCode::KEY_RIGHTALT, 1).mask, "language");
        m.process(KeyCode::KEY_RIGHTALT, 0);
        assert!(m.process(KeyCode::KEY_RIGHTMETA, 1).mask, "style");
        m.process(KeyCode::KEY_RIGHTMETA, 0);
        assert!(!m.process(KeyCode::KEY_F13, 1).mask, "F13");
        m.process(KeyCode::KEY_F13, 0);

        m.process(KeyCode::KEY_LEFTMETA, 1);
        let rewrite = m.process(KeyCode::KEY_LEFTALT, 1);
        assert_eq!(
            rewrite.event,
            Some(crate::hotkey::HotkeyEvent::Action(
                crate::hotkey::HotkeyAction::Rewrite(formal)
            ))
        );
        assert!(rewrite.mask, "rewrite");
        m.process(KeyCode::KEY_LEFTALT, 0);
        m.process(KeyCode::KEY_LEFTMETA, 0);

        let cancel = m.process(KeyCode::KEY_LEFTALT, 1);
        assert_eq!(cancel.event, Some(crate::hotkey::HotkeyEvent::Cancel));
        assert!(
            !cancel.mask,
            "the cancel key also reaches the app by design"
        );
        let command = m.process(KeyCode::KEY_LEFTMETA, 1);
        assert_eq!(
            command.event,
            Some(crate::hotkey::HotkeyEvent::Command(true))
        );
        assert!(command.mask, "command");
    }
}
