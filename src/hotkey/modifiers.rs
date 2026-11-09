//! The physical state of Ctrl, Alt, Shift, and Super. The evdev listener
//! registers each device it reads, and a query asks the kernel (EVIOCGKEY)
//! for the keys held on those devices, so no device opens again per query.

use evdev::{Device, KeyCode};
use input_linux::EvdevHandle;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::os::fd::{AsFd, OwnedFd};
use std::path::{Path, PathBuf};
use tracing::debug;

const MODIFIERS: [KeyCode; 8] = [
    KeyCode::KEY_LEFTCTRL,
    KeyCode::KEY_RIGHTCTRL,
    KeyCode::KEY_LEFTSHIFT,
    KeyCode::KEY_RIGHTSHIFT,
    KeyCode::KEY_LEFTALT,
    KeyCode::KEY_RIGHTALT,
    KeyCode::KEY_LEFTMETA,
    KeyCode::KEY_RIGHTMETA,
];

const KEY_BYTES: usize = (input_linux::sys::KEY_CNT as usize).div_ceil(8);

static DEVICES: Lazy<Mutex<HashMap<PathBuf, EvdevHandle<OwnedFd>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Keeps a device in the registry until it drops.
pub struct Registration {
    path: PathBuf,
}

impl Drop for Registration {
    fn drop(&mut self) {
        DEVICES.lock().remove(&self.path);
    }
}

/// Add a device the listener reads, if it has modifier keys. A mouse read
/// for a button hotkey has none, and must not report the keys as up. The
/// registry keeps a duplicate of the descriptor.
pub fn register(path: &Path, device: &Device) -> Option<Registration> {
    let has_modifiers = device
        .supported_keys()
        .is_some_and(|keys| MODIFIERS.iter().any(|key| keys.contains(*key)));
    if !has_modifiers {
        return None;
    }
    match device.as_fd().try_clone_to_owned() {
        Ok(fd) => {
            DEVICES
                .lock()
                .insert(path.to_path_buf(), EvdevHandle::new(fd));
            Some(Registration {
                path: path.to_path_buf(),
            })
        },
        Err(e) => {
            debug!("Cannot track modifiers on {}: {}", path.display(), e);
            None
        },
    }
}

fn is_set(bits: &[u8], key: KeyCode) -> bool {
    let code = usize::from(key.code());
    bits.get(code / 8)
        .is_some_and(|byte| byte & (1 << (code % 8)) != 0)
}

/// Whether a modifier is down on any registered device, or `None` when no
/// device answers.
pub fn held() -> Option<bool> {
    let devices = DEVICES.lock();
    let mut answered = false;
    for handle in devices.values() {
        let mut bits = [0u8; KEY_BYTES];
        if handle.key_state(&mut bits).is_err() {
            continue;
        }
        answered = true;
        if MODIFIERS.iter().any(|key| is_set(&bits, *key)) {
            return Some(true);
        }
    }
    answered.then_some(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bitmap_is_read_per_key_code() {
        let mut bits = [0u8; KEY_BYTES];
        assert!(!MODIFIERS.iter().any(|key| is_set(&bits, *key)));
        let code = usize::from(KeyCode::KEY_RIGHTALT.code());
        bits[code / 8] |= 1 << (code % 8);
        assert!(is_set(&bits, KeyCode::KEY_RIGHTALT));
        assert!(!is_set(&bits, KeyCode::KEY_LEFTALT));
        assert!(!is_set(&[], KeyCode::KEY_LEFTCTRL));
    }
}
