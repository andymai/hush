//! Hotkeys read straight from `/dev/input`, so they work on Wayland, X11, and
//! the console alike. Needs read access to the event devices, which the same
//! `input` group membership as uinput provides.

use super::combination::KeyCombination;
use super::HotkeyEvent;
use anyhow::{anyhow, Result};
use evdev::{Device, EventSummary, KeyCode};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::io;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;
use tracing::{debug, info, warn};

const POLL_INTERVAL_MS: i32 = 200;
const RESCAN_INTERVAL: Duration = Duration::from_secs(2);

/// Tracks held keys and reports when the combination becomes active or
/// inactive. Left and right modifier variants are interchangeable; extra
/// modifiers block the match so a bare `Space` hotkey never fires on
/// `Ctrl+Space`.
pub struct HotkeyMatcher {
    combination: KeyCombination,
    held: HashSet<KeyCode>,
    active: bool,
}

const MODIFIER_PAIRS: [(KeyCode, KeyCode); 4] = [
    (KeyCode::KEY_LEFTCTRL, KeyCode::KEY_RIGHTCTRL),
    (KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_RIGHTSHIFT),
    (KeyCode::KEY_LEFTALT, KeyCode::KEY_RIGHTALT),
    (KeyCode::KEY_LEFTMETA, KeyCode::KEY_RIGHTMETA),
];

impl HotkeyMatcher {
    pub fn new(combination: KeyCombination) -> Self {
        Self {
            combination,
            held: HashSet::new(),
            active: false,
        }
    }

    /// Feed one key event (`value`: 1 press, 0 release, 2 repeat).
    pub fn feed(&mut self, code: KeyCode, value: i32) -> Option<HotkeyEvent> {
        match value {
            1 => {
                self.held.insert(code);
            },
            0 => {
                self.held.remove(&code);
            },
            _ => return None,
        }

        let key = self.combination.evdev_key();
        if !self.active {
            if value == 1 && code == key && self.modifiers_match() {
                self.active = true;
                return Some(HotkeyEvent::Pressed);
            }
        } else if value == 0 && (code == key || !self.modifiers_match()) {
            self.active = false;
            return Some(HotkeyEvent::Released);
        }
        None
    }

    /// The hotkey's own key never counts as a held modifier, so a bare
    /// `RightAlt` hotkey matches even though Right Alt is a modifier.
    fn modifiers_match(&self) -> bool {
        let wanted = self.combination.modifiers;
        let key = self.combination.evdev_key();
        let required = [wanted.ctrl, wanted.shift, wanted.alt, wanted.super_key];
        MODIFIER_PAIRS
            .iter()
            .zip(required)
            .all(|((left, right), needed)| {
                let held = (*left != key && self.held.contains(left))
                    || (*right != key && self.held.contains(right));
                held == needed
            })
    }
}

fn is_keyboard(device: &Device, key: KeyCode) -> bool {
    device
        .supported_keys()
        .map(|keys| keys.contains(key) && keys.contains(KeyCode::KEY_A))
        .unwrap_or(false)
}

/// Paths and devices of every readable keyboard that has the hotkey's key.
pub fn keyboards(key: KeyCode) -> Vec<(PathBuf, Device)> {
    evdev::enumerate()
        .filter(|(_, device)| is_keyboard(device, key))
        .collect()
}

fn explain_no_keyboard() -> anyhow::Error {
    let mut denied = false;
    let mut nodes = 0;
    if let Ok(entries) = std::fs::read_dir("/dev/input") {
        for entry in entries.flatten() {
            if !entry.file_name().to_string_lossy().starts_with("event") {
                continue;
            }
            nodes += 1;
            if let Err(e) = std::fs::File::open(entry.path()) {
                if e.kind() == io::ErrorKind::PermissionDenied {
                    denied = true;
                }
            }
        }
    }
    if denied {
        anyhow!(
            "No readable keyboard under /dev/input (permission denied). \
             Run `hush setup permissions`."
        )
    } else if nodes == 0 {
        anyhow!("No input devices under /dev/input")
    } else {
        anyhow!("No keyboard found under /dev/input")
    }
}

pub struct EvdevHotkey {
    combination: KeyCombination,
    running: Arc<AtomicBool>,
    matcher: Arc<Mutex<HotkeyMatcher>>,
    sender: mpsc::Sender<HotkeyEvent>,
}

impl EvdevHotkey {
    pub fn new(combination: KeyCombination) -> Result<(Self, mpsc::Receiver<HotkeyEvent>)> {
        let found = keyboards(combination.evdev_key());
        if found.is_empty() {
            return Err(explain_no_keyboard());
        }
        for (path, device) in &found {
            debug!(
                "Keyboard {} at {}",
                device.name().unwrap_or("unknown"),
                path.display()
            );
        }
        info!(
            "evdev hotkey '{}' watching {} keyboard(s)",
            combination,
            found.len()
        );

        let (sender, receiver) = mpsc::channel();
        Ok((
            Self {
                matcher: Arc::new(Mutex::new(HotkeyMatcher::new(combination.clone()))),
                combination,
                running: Arc::new(AtomicBool::new(false)),
                sender,
            },
            receiver,
        ))
    }

    pub fn combination(&self) -> &KeyCombination {
        &self.combination
    }

    /// Start reader threads for every keyboard and keep watching for new ones.
    pub fn start_listening(&self) -> Result<()> {
        if self.running.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let running = Arc::clone(&self.running);
        let matcher = Arc::clone(&self.matcher);
        let sender = self.sender.clone();
        let key = self.combination.evdev_key();
        thread::Builder::new()
            .name("hush-hotkey-scan".into())
            .spawn(move || supervise(running, matcher, sender, key))
            .map_err(|e| anyhow!("Failed to start hotkey thread: {}", e))?;
        Ok(())
    }

    pub fn stop_listening(&self) -> Result<()> {
        self.running.store(false, Ordering::Release);
        Ok(())
    }
}

impl Drop for EvdevHotkey {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
    }
}

fn supervise(
    running: Arc<AtomicBool>,
    matcher: Arc<Mutex<HotkeyMatcher>>,
    sender: mpsc::Sender<HotkeyEvent>,
    key: KeyCode,
) {
    let mut readers: HashMap<PathBuf, thread::JoinHandle<()>> = HashMap::new();
    while running.load(Ordering::Acquire) {
        readers.retain(|_, handle| !handle.is_finished());
        for (path, device) in keyboards(key) {
            if readers.contains_key(&path) {
                continue;
            }
            let running = Arc::clone(&running);
            let matcher = Arc::clone(&matcher);
            let sender = sender.clone();
            let name = format!("hush-hotkey-{}", path.display());
            let spawn_path = path.clone();
            match thread::Builder::new()
                .name(name)
                .spawn(move || read_device(&spawn_path, device, running, matcher, sender))
            {
                Ok(handle) => {
                    readers.insert(path, handle);
                },
                Err(e) => warn!("Failed to start reader for {}: {}", path.display(), e),
            }
        }
        thread::sleep(RESCAN_INTERVAL);
    }
}

fn read_device(
    path: &Path,
    mut device: Device,
    running: Arc<AtomicBool>,
    matcher: Arc<Mutex<HotkeyMatcher>>,
    sender: mpsc::Sender<HotkeyEvent>,
) {
    while running.load(Ordering::Acquire) {
        match wait_readable(device.as_raw_fd(), POLL_INTERVAL_MS) {
            Ok(false) => continue,
            Ok(true) => {},
            Err(e) => {
                info!("Keyboard {} went away: {}", path.display(), e);
                return;
            },
        }
        let events = match device.fetch_events() {
            Ok(events) => events,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
            Err(e) => {
                info!("Keyboard {} went away: {}", path.display(), e);
                return;
            },
        };
        for event in events {
            if let EventSummary::Key(_, code, value) = event.destructure() {
                let hotkey_event = matcher.lock().feed(code, value);
                if let Some(hotkey_event) = hotkey_event {
                    debug!("Hotkey {:?} from {}", hotkey_event, path.display());
                    if sender.send(hotkey_event).is_err() {
                        running.store(false, Ordering::Release);
                        return;
                    }
                }
            }
        }
    }
}

/// Block until the descriptor is readable or the timeout passes. An error
/// means the device is gone.
fn wait_readable(fd: i32, timeout_ms: i32) -> io::Result<bool> {
    let mut pollfd = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: pollfd is a valid, initialized struct and nfds is 1.
    let ready = unsafe { libc::poll(&mut pollfd, 1, timeout_ms) };
    if ready < 0 {
        let err = io::Error::last_os_error();
        return if err.kind() == io::ErrorKind::Interrupted {
            Ok(false)
        } else {
            Err(err)
        };
    }
    if ready == 0 {
        return Ok(false);
    }
    if pollfd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
        return Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "device disconnected",
        ));
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matcher(text: &str) -> HotkeyMatcher {
        HotkeyMatcher::new(KeyCombination::parse(text).unwrap())
    }

    #[test]
    fn press_and_release_with_modifiers() {
        let mut m = matcher("Ctrl+Shift+Space");
        assert_eq!(m.feed(KeyCode::KEY_LEFTCTRL, 1), None);
        assert_eq!(m.feed(KeyCode::KEY_RIGHTSHIFT, 1), None);
        assert_eq!(m.feed(KeyCode::KEY_SPACE, 1), Some(HotkeyEvent::Pressed));
        assert_eq!(m.feed(KeyCode::KEY_SPACE, 2), None, "repeats are ignored");
        assert_eq!(m.feed(KeyCode::KEY_SPACE, 0), Some(HotkeyEvent::Released));
        assert_eq!(m.feed(KeyCode::KEY_LEFTCTRL, 0), None);
    }

    #[test]
    fn releasing_a_modifier_first_ends_the_hold() {
        let mut m = matcher("Ctrl+Space");
        m.feed(KeyCode::KEY_LEFTCTRL, 1);
        assert_eq!(m.feed(KeyCode::KEY_SPACE, 1), Some(HotkeyEvent::Pressed));
        assert_eq!(
            m.feed(KeyCode::KEY_LEFTCTRL, 0),
            Some(HotkeyEvent::Released)
        );
        assert_eq!(m.feed(KeyCode::KEY_SPACE, 0), None, "already released");
    }

    #[test]
    fn extra_modifiers_block_the_match() {
        let mut m = matcher("Space");
        m.feed(KeyCode::KEY_LEFTCTRL, 1);
        assert_eq!(m.feed(KeyCode::KEY_SPACE, 1), None);
        m.feed(KeyCode::KEY_SPACE, 0);
        m.feed(KeyCode::KEY_LEFTCTRL, 0);
        assert_eq!(m.feed(KeyCode::KEY_SPACE, 1), Some(HotkeyEvent::Pressed));
    }

    #[test]
    fn a_bare_modifier_hotkey_fires_on_its_own_and_not_with_others() {
        let mut m = matcher("RightAlt");
        assert_eq!(m.feed(KeyCode::KEY_RIGHTALT, 1), Some(HotkeyEvent::Pressed));
        assert_eq!(m.feed(KeyCode::KEY_RIGHTALT, 2), None);
        assert_eq!(
            m.feed(KeyCode::KEY_RIGHTALT, 0),
            Some(HotkeyEvent::Released)
        );

        m.feed(KeyCode::KEY_LEFTCTRL, 1);
        assert_eq!(
            m.feed(KeyCode::KEY_RIGHTALT, 1),
            None,
            "Ctrl+RightAlt is a different chord"
        );
        m.feed(KeyCode::KEY_RIGHTALT, 0);
        m.feed(KeyCode::KEY_LEFTCTRL, 0);

        let mut m = matcher("Ctrl+RightAlt");
        m.feed(KeyCode::KEY_LEFTCTRL, 1);
        assert_eq!(m.feed(KeyCode::KEY_RIGHTALT, 1), Some(HotkeyEvent::Pressed));
    }

    #[test]
    fn key_without_modifiers_does_not_fire() {
        let mut m = matcher("Ctrl+Shift+Space");
        assert_eq!(m.feed(KeyCode::KEY_SPACE, 1), None);
        assert_eq!(m.feed(KeyCode::KEY_SPACE, 0), None);
    }

    #[test]
    fn keyboard_enumeration_does_not_panic() {
        let found = keyboards(KeyCode::KEY_SPACE);
        for (path, device) in &found {
            assert!(path.starts_with("/dev/input"));
            assert!(is_keyboard(device, KeyCode::KEY_SPACE));
        }
    }
}
