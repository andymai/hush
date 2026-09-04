//! Hotkey combinations as written in the config (`Ctrl+Shift+Space`) and their
//! key codes for each backend.

use anyhow::{anyhow, Result};
use evdev::KeyCode;
use global_hotkey::hotkey::{Code, Modifiers as X11Modifiers};
use std::fmt;

/// Modifier classes; left and right variants count the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub super_key: bool,
}

/// A parsed hotkey: a set of modifiers plus exactly one key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyCombination {
    pub modifiers: Modifiers,
    key: &'static KeyDef,
}

struct KeyDef {
    names: &'static [&'static str],
    evdev: KeyCode,
    x11: Code,
}

impl fmt::Debug for KeyDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.names[0])
    }
}

impl PartialEq for KeyDef {
    fn eq(&self, other: &Self) -> bool {
        self.evdev == other.evdev
    }
}
impl Eq for KeyDef {}

macro_rules! keys {
    ($( [$($name:literal),+] => $ev:ident / $x11:ident ),* $(,)?) => {
        &[$( KeyDef { names: &[$($name),+], evdev: KeyCode::$ev, x11: Code::$x11 } ),*]
    };
}

const KEYS: &[KeyDef] = keys![
    ["a"] => KEY_A / KeyA, ["b"] => KEY_B / KeyB, ["c"] => KEY_C / KeyC, ["d"] => KEY_D / KeyD,
    ["e"] => KEY_E / KeyE, ["f"] => KEY_F / KeyF, ["g"] => KEY_G / KeyG, ["h"] => KEY_H / KeyH,
    ["i"] => KEY_I / KeyI, ["j"] => KEY_J / KeyJ, ["k"] => KEY_K / KeyK, ["l"] => KEY_L / KeyL,
    ["m"] => KEY_M / KeyM, ["n"] => KEY_N / KeyN, ["o"] => KEY_O / KeyO, ["p"] => KEY_P / KeyP,
    ["q"] => KEY_Q / KeyQ, ["r"] => KEY_R / KeyR, ["s"] => KEY_S / KeyS, ["t"] => KEY_T / KeyT,
    ["u"] => KEY_U / KeyU, ["v"] => KEY_V / KeyV, ["w"] => KEY_W / KeyW, ["x"] => KEY_X / KeyX,
    ["y"] => KEY_Y / KeyY, ["z"] => KEY_Z / KeyZ,
    ["0"] => KEY_0 / Digit0, ["1"] => KEY_1 / Digit1, ["2"] => KEY_2 / Digit2,
    ["3"] => KEY_3 / Digit3, ["4"] => KEY_4 / Digit4, ["5"] => KEY_5 / Digit5,
    ["6"] => KEY_6 / Digit6, ["7"] => KEY_7 / Digit7, ["8"] => KEY_8 / Digit8,
    ["9"] => KEY_9 / Digit9,
    ["f1"] => KEY_F1 / F1, ["f2"] => KEY_F2 / F2, ["f3"] => KEY_F3 / F3, ["f4"] => KEY_F4 / F4,
    ["f5"] => KEY_F5 / F5, ["f6"] => KEY_F6 / F6, ["f7"] => KEY_F7 / F7, ["f8"] => KEY_F8 / F8,
    ["f9"] => KEY_F9 / F9, ["f10"] => KEY_F10 / F10, ["f11"] => KEY_F11 / F11,
    ["f12"] => KEY_F12 / F12,
    // Keys a programmable keyboard can emit that no application binds.
    ["f13"] => KEY_F13 / F13, ["f14"] => KEY_F14 / F14, ["f15"] => KEY_F15 / F15,
    ["f16"] => KEY_F16 / F16, ["f17"] => KEY_F17 / F17, ["f18"] => KEY_F18 / F18,
    ["f19"] => KEY_F19 / F19, ["f20"] => KEY_F20 / F20, ["f21"] => KEY_F21 / F21,
    ["f22"] => KEY_F22 / F22, ["f23"] => KEY_F23 / F23, ["f24"] => KEY_F24 / F24,
    // A bare modifier as the hotkey: applications ignore a modifier pressed on its own.
    ["rightalt", "altgr"] => KEY_RIGHTALT / AltRight, ["leftalt"] => KEY_LEFTALT / AltLeft,
    ["rightctrl", "rightcontrol"] => KEY_RIGHTCTRL / ControlRight,
    ["leftctrl", "leftcontrol"] => KEY_LEFTCTRL / ControlLeft,
    ["rightshift"] => KEY_RIGHTSHIFT / ShiftRight, ["leftshift"] => KEY_LEFTSHIFT / ShiftLeft,
    ["rightsuper", "rightmeta", "rightwin"] => KEY_RIGHTMETA / MetaRight,
    ["leftsuper", "leftmeta", "leftwin"] => KEY_LEFTMETA / MetaLeft,
    ["space"] => KEY_SPACE / Space, ["enter", "return"] => KEY_ENTER / Enter,
    ["escape", "esc"] => KEY_ESC / Escape, ["backspace"] => KEY_BACKSPACE / Backspace,
    ["delete", "del"] => KEY_DELETE / Delete, ["tab"] => KEY_TAB / Tab,
    ["home"] => KEY_HOME / Home, ["end"] => KEY_END / End,
    ["pageup", "pgup"] => KEY_PAGEUP / PageUp, ["pagedown", "pgdn"] => KEY_PAGEDOWN / PageDown,
    ["insert", "ins"] => KEY_INSERT / Insert,
    ["up", "arrowup"] => KEY_UP / ArrowUp, ["down", "arrowdown"] => KEY_DOWN / ArrowDown,
    ["left", "arrowleft"] => KEY_LEFT / ArrowLeft, ["right", "arrowright"] => KEY_RIGHT / ArrowRight,
    [",", "comma"] => KEY_COMMA / Comma, [".", "period"] => KEY_DOT / Period,
    ["/", "slash"] => KEY_SLASH / Slash, [";", "semicolon"] => KEY_SEMICOLON / Semicolon,
    ["'", "quote"] => KEY_APOSTROPHE / Quote, ["[", "bracketleft"] => KEY_LEFTBRACE / BracketLeft,
    ["]", "bracketright"] => KEY_RIGHTBRACE / BracketRight,
    ["\\", "backslash"] => KEY_BACKSLASH / Backslash, ["-", "minus"] => KEY_MINUS / Minus,
    ["=", "equal"] => KEY_EQUAL / Equal, ["`", "backquote", "grave"] => KEY_GRAVE / Backquote,
    ["capslock"] => KEY_CAPSLOCK / CapsLock, ["pause"] => KEY_PAUSE / Pause,
    ["scrolllock"] => KEY_SCROLLLOCK / ScrollLock, ["printscreen", "print"] => KEY_SYSRQ / PrintScreen,
    ["menu"] => KEY_COMPOSE / ContextMenu, ["numlock"] => KEY_NUMLOCK / NumLock,
];

impl KeyCombination {
    /// Parse `Ctrl+Shift+Space` style text. Case and surrounding spaces are ignored.
    pub fn parse(text: &str) -> Result<Self> {
        let mut modifiers = Modifiers::default();
        let mut key = None;

        for part in text.split('+').map(str::trim).filter(|p| !p.is_empty()) {
            match part.to_lowercase().as_str() {
                "ctrl" | "control" => modifiers.ctrl = true,
                "shift" => modifiers.shift = true,
                "alt" => modifiers.alt = true,
                "super" | "cmd" | "win" | "meta" => modifiers.super_key = true,
                name => {
                    if key.is_some() {
                        return Err(anyhow!("Multiple keys in hotkey '{}'", text));
                    }
                    key = Some(
                        KEYS.iter()
                            .find(|def| def.names.contains(&name))
                            .ok_or_else(|| {
                                anyhow!("Unknown key '{}' in hotkey '{}'", part, text)
                            })?,
                    );
                },
            }
        }

        let key = key.ok_or_else(|| anyhow!("Hotkey '{}' has no key, only modifiers", text))?;
        Ok(Self { modifiers, key })
    }

    /// The main key as an evdev code.
    pub fn evdev_key(&self) -> KeyCode {
        self.key.evdev
    }

    /// The main key as an X11 key code.
    pub fn x11_code(&self) -> Code {
        self.key.x11
    }

    pub fn x11_modifiers(&self) -> X11Modifiers {
        let mut mods = X11Modifiers::empty();
        if self.modifiers.ctrl {
            mods |= X11Modifiers::CONTROL;
        }
        if self.modifiers.shift {
            mods |= X11Modifiers::SHIFT;
        }
        if self.modifiers.alt {
            mods |= X11Modifiers::ALT;
        }
        if self.modifiers.super_key {
            mods |= X11Modifiers::SUPER;
        }
        mods
    }
}

impl fmt::Display for KeyCombination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.modifiers.ctrl {
            f.write_str("Ctrl+")?;
        }
        if self.modifiers.alt {
            f.write_str("Alt+")?;
        }
        if self.modifiers.shift {
            f.write_str("Shift+")?;
        }
        if self.modifiers.super_key {
            f.write_str("Super+")?;
        }
        f.write_str(&display_name(self.key.names[0]))
    }
}

fn display_name(name: &str) -> String {
    let capitalize = |s: &str| {
        let mut chars = s.chars();
        match chars.next() {
            Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
            None => String::new(),
        }
    };
    for side in ["right", "left"] {
        if let Some(rest) = name.strip_prefix(side) {
            if !rest.is_empty() {
                return capitalize(side) + &capitalize(rest);
            }
        }
    }
    capitalize(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modifiers_and_key_case_insensitively() {
        let combo = KeyCombination::parse(" ctrl + SHIFT + space ").unwrap();
        assert!(combo.modifiers.ctrl && combo.modifiers.shift);
        assert!(!combo.modifiers.alt && !combo.modifiers.super_key);
        assert_eq!(combo.evdev_key(), KeyCode::KEY_SPACE);
        assert_eq!(combo.x11_code(), Code::Space);
        assert_eq!(combo.to_string(), "Ctrl+Shift+Space");
    }

    #[test]
    fn accepts_aliases_and_bare_keys() {
        assert_eq!(
            KeyCombination::parse("F12").unwrap().evdev_key(),
            KeyCode::KEY_F12
        );
        assert_eq!(
            KeyCombination::parse("Return").unwrap().evdev_key(),
            KeyCode::KEY_ENTER
        );
        assert_eq!(
            KeyCombination::parse("Win+,").unwrap().evdev_key(),
            KeyCode::KEY_COMMA
        );
        assert_eq!(KeyCombination::parse("Alt+`").unwrap().to_string(), "Alt+`");
    }

    #[test]
    fn bare_modifiers_and_high_function_keys_are_keys() {
        let alt = KeyCombination::parse("RightAlt").unwrap();
        assert_eq!(alt.evdev_key(), KeyCode::KEY_RIGHTALT);
        assert_eq!(alt.x11_code(), Code::AltRight);
        assert_eq!(alt.modifiers, Modifiers::default());
        assert_eq!(alt.to_string(), "RightAlt");
        assert_eq!(
            KeyCombination::parse("AltGr").unwrap().to_string(),
            "RightAlt"
        );
        assert_eq!(
            KeyCombination::parse("Ctrl+RightSuper")
                .unwrap()
                .to_string(),
            "Ctrl+RightSuper"
        );
        assert_eq!(
            KeyCombination::parse("F13").unwrap().evdev_key(),
            KeyCode::KEY_F13
        );
        assert_eq!(KeyCombination::parse("f24").unwrap().x11_code(), Code::F24);
    }

    #[test]
    fn rejects_invalid_combinations() {
        assert!(KeyCombination::parse("Ctrl+Shift").is_err());
        assert!(KeyCombination::parse("Ctrl+A+B").is_err());
        assert!(KeyCombination::parse("Ctrl+Bogus").is_err());
        assert!(KeyCombination::parse("").is_err());
    }

    #[test]
    fn x11_modifier_flags_match() {
        let combo = KeyCombination::parse("Ctrl+Alt+Super+Shift+A").unwrap();
        assert_eq!(
            combo.x11_modifiers(),
            X11Modifiers::CONTROL | X11Modifiers::ALT | X11Modifiers::SUPER | X11Modifiers::SHIFT
        );
    }
}
