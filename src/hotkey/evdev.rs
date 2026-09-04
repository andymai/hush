//! Hotkeys read straight from `/dev/input`, so they work on Wayland, X11, and
//! the console alike. Needs read access to the event devices, which the same
//! `input` group membership as uinput provides.

use super::combination::KeyCombination;
use super::HotkeyEvent;
use anyhow::{anyhow, Result};
use evdev::uinput::VirtualDevice;
use evdev::{AttributeSetRef, BusType, Device, EventSummary, InputEvent, InputId, KeyCode};
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

/// Outcome of one key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Processed {
    pub event: Option<HotkeyEvent>,
    /// The event belongs to the hotkey and must not reach applications.
    pub swallow: bool,
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
        self.process(code, value).event
    }

    /// Feed one key event and also learn whether an exclusive reader should
    /// keep it from the application: the hotkey's own key while the chord is
    /// active, including its release and repeats.
    pub fn process(&mut self, code: KeyCode, value: i32) -> Processed {
        let key = self.combination.evdev_key();
        let was_active = self.active;
        match value {
            1 => {
                self.held.insert(code);
            },
            0 => {
                self.held.remove(&code);
            },
            _ => {},
        }

        let mut event = None;
        if value == 1 || value == 0 {
            if !self.active {
                if value == 1 && code == key && self.modifiers_match() {
                    self.active = true;
                    event = Some(HotkeyEvent::Pressed);
                }
            } else if value == 0 && (code == key || !self.modifiers_match()) {
                self.active = false;
                event = Some(HotkeyEvent::Released);
            }
        }
        Processed {
            event,
            swallow: code == key && (was_active || self.active),
        }
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

/// Vendor id stamped on every uinput device Hush creates (the typing
/// keyboard and exclusive-mode proxies) so the hotkey reader can skip its
/// own output instead of grabbing a proxy and chaining another onto it.
pub const VIRTUAL_VENDOR: u16 = 0x4855;
pub const VIRTUAL_PRODUCT_KEYBOARD: u16 = 0x0001;
pub const VIRTUAL_PRODUCT_PROXY: u16 = 0x0002;

/// Whether an input identity belongs to one of Hush's own virtual devices.
pub fn is_hush_virtual(id: InputId) -> bool {
    id.bus_type() == BusType::BUS_VIRTUAL && id.vendor() == VIRTUAL_VENDOR
}

/// A device is interesting when it carries the hotkey's key (a keyboard, or
/// a mouse for a button), or when it is a keyboard and the chord needs
/// modifiers from one.
pub fn device_matches(keys: &AttributeSetRef<KeyCode>, combination: &KeyCombination) -> bool {
    let is_keyboard = keys.contains(KeyCode::KEY_A) && keys.contains(KeyCode::KEY_LEFTCTRL);
    let has_key = keys.contains(combination.evdev_key());
    has_key && (is_keyboard || combination.is_mouse_button())
        || (combination.has_modifiers() && is_keyboard)
}

fn carries_key(device: &Device, key: KeyCode) -> bool {
    device
        .supported_keys()
        .map(|keys| keys.contains(key))
        .unwrap_or(false)
}

/// Paths and devices of every readable device the combination needs.
pub fn devices_for(combination: &KeyCombination) -> Vec<(PathBuf, Device)> {
    evdev::enumerate()
        .filter(|(_, device)| !is_hush_virtual(device.input_id()))
        .filter(|(_, device)| {
            device
                .supported_keys()
                .map(|keys| device_matches(keys, combination))
                .unwrap_or(false)
        })
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
    exclusive: bool,
    running: Arc<AtomicBool>,
    matcher: Arc<Mutex<HotkeyMatcher>>,
    sender: mpsc::Sender<HotkeyEvent>,
}

impl EvdevHotkey {
    /// With `exclusive`, the device that carries the hotkey's key is grabbed
    /// and every other event it produces is replayed through a uinput proxy,
    /// so applications never see the hotkey itself.
    pub fn new(
        combination: KeyCombination,
        exclusive: bool,
    ) -> Result<(Self, mpsc::Receiver<HotkeyEvent>)> {
        let found = devices_for(&combination);
        if found.is_empty()
            || !found
                .iter()
                .any(|(_, d)| carries_key(d, combination.evdev_key()))
        {
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
            "evdev hotkey '{}' watching {} device(s){}",
            combination,
            found.len(),
            if exclusive { ", exclusive" } else { "" }
        );

        let (sender, receiver) = mpsc::channel();
        Ok((
            Self {
                matcher: Arc::new(Mutex::new(HotkeyMatcher::new(combination.clone()))),
                combination,
                exclusive,
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
        let combination = self.combination.clone();
        let exclusive = self.exclusive;
        thread::Builder::new()
            .name("hush-hotkey-scan".into())
            .spawn(move || supervise(running, matcher, sender, combination, exclusive))
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
    combination: KeyCombination,
    exclusive: bool,
) {
    let mut readers: HashMap<PathBuf, thread::JoinHandle<()>> = HashMap::new();
    while running.load(Ordering::Acquire) {
        readers.retain(|_, handle| !handle.is_finished());
        for (path, device) in devices_for(&combination) {
            if readers.contains_key(&path) {
                continue;
            }
            let running = Arc::clone(&running);
            let matcher = Arc::clone(&matcher);
            let sender = sender.clone();
            let name = format!("hush-hotkey-{}", path.display());
            let spawn_path = path.clone();
            // Only the device that carries the hotkey's key is grabbed; a
            // keyboard read for modifiers stays untouched.
            let grab = exclusive && carries_key(&device, combination.evdev_key());
            match thread::Builder::new()
                .name(name)
                .spawn(move || read_device(&spawn_path, device, running, matcher, sender, grab))
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

/// Clone a device's buttons, keys, and relative axes into a uinput device so
/// a grabbed device's other events can be replayed to the system.
fn build_proxy(device: &Device) -> Result<VirtualDevice> {
    // uinput caps names at 80 bytes including the terminator.
    const MAX_NAME: usize = 79;
    let suffix = " (hush)";
    let mut base = device.name().unwrap_or("input").to_string();
    while base.len() + suffix.len() > MAX_NAME {
        base.pop();
    }
    let name = base + suffix;
    let id = InputId::new(
        BusType::BUS_VIRTUAL,
        VIRTUAL_VENDOR,
        VIRTUAL_PRODUCT_PROXY,
        1,
    );
    let mut builder = VirtualDevice::builder()?.name(name.as_str()).input_id(id);
    if let Some(keys) = device.supported_keys() {
        builder = builder.with_keys(keys)?;
    }
    if let Some(axes) = device.supported_relative_axes() {
        builder = builder.with_relative_axes(axes)?;
    }
    Ok(builder.build()?)
}

fn read_device(
    path: &Path,
    mut device: Device,
    running: Arc<AtomicBool>,
    matcher: Arc<Mutex<HotkeyMatcher>>,
    sender: mpsc::Sender<HotkeyEvent>,
    grab: bool,
) {
    let mut proxy = if grab {
        match build_proxy(&device).and_then(|proxy| {
            device.grab()?;
            Ok(proxy)
        }) {
            Ok(proxy) => {
                info!("Grabbed {} exclusively for the hotkey", path.display());
                Some(proxy)
            },
            Err(e) => {
                warn!(
                    "Could not grab {} ({}); the hotkey stays visible to applications",
                    path.display(),
                    e
                );
                None
            },
        }
    } else {
        None
    };
    let mut batch: Vec<InputEvent> = Vec::new();
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
            let raw = InputEvent::new(event.event_type().0, event.code(), event.value());
            let mut forward = true;
            match event.destructure() {
                EventSummary::Key(_, code, value) => {
                    let processed = matcher.lock().process(code, value);
                    if let Some(hotkey_event) = processed.event {
                        debug!("Hotkey {:?} from {}", hotkey_event, path.display());
                        if sender.send(hotkey_event).is_err() {
                            running.store(false, Ordering::Release);
                            return;
                        }
                    }
                    forward = !processed.swallow;
                },
                EventSummary::Synchronization(..) => {
                    // The proxy adds its own SYN_REPORT per emit, so replay each
                    // frame as one batch and drop the original sync event.
                    if let Some(proxy) = proxy.as_mut() {
                        if !batch.is_empty() {
                            if let Err(e) = proxy.emit(&batch) {
                                warn!("Proxy emit failed for {}: {}", path.display(), e);
                            }
                            batch.clear();
                        }
                    }
                    forward = false;
                },
                _ => {},
            }
            if forward && proxy.is_some() {
                batch.push(raw);
            }
        }
    }
    if let Some(proxy) = proxy.as_mut() {
        if !batch.is_empty() {
            let _ = proxy.emit(&batch);
        }
    }
    let _ = device.ungrab();
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
    fn exclusive_readers_swallow_only_the_hotkey() {
        let mut m = matcher("Ctrl+Space");
        assert!(
            !m.process(KeyCode::KEY_SPACE, 1).swallow,
            "plain space is typed"
        );
        assert!(!m.process(KeyCode::KEY_SPACE, 0).swallow);
        m.process(KeyCode::KEY_LEFTCTRL, 1);
        let press = m.process(KeyCode::KEY_SPACE, 1);
        assert_eq!(press.event, Some(HotkeyEvent::Pressed));
        assert!(press.swallow);
        assert!(
            m.process(KeyCode::KEY_SPACE, 2).swallow,
            "repeats while active"
        );
        let release = m.process(KeyCode::KEY_SPACE, 0);
        assert_eq!(release.event, Some(HotkeyEvent::Released));
        assert!(release.swallow);
        assert!(
            !m.process(KeyCode::KEY_LEFTCTRL, 0).swallow,
            "modifiers pass through"
        );
    }

    #[test]
    fn mouse_button_hotkey_matches_mice_and_keyboards_for_chords() {
        use evdev::AttributeSet;
        let mut mouse = AttributeSet::<KeyCode>::new();
        for k in [
            KeyCode::BTN_LEFT,
            KeyCode::BTN_RIGHT,
            KeyCode::BTN_SIDE,
            KeyCode::BTN_EXTRA,
        ] {
            mouse.insert(k);
        }
        let mut keyboard = AttributeSet::<KeyCode>::new();
        for k in [KeyCode::KEY_A, KeyCode::KEY_LEFTCTRL, KeyCode::KEY_SPACE] {
            keyboard.insert(k);
        }
        let mouse4 = KeyCombination::parse("Mouse4").unwrap();
        assert!(device_matches(&mouse, &mouse4));
        assert!(!device_matches(&keyboard, &mouse4));
        let chord = KeyCombination::parse("Ctrl+Mouse4").unwrap();
        assert!(device_matches(&mouse, &chord));
        assert!(
            device_matches(&keyboard, &chord),
            "keyboard supplies the modifier"
        );
        let space = KeyCombination::parse("Space").unwrap();
        assert!(device_matches(&keyboard, &space));
        assert!(!device_matches(&mouse, &space));
    }

    #[test]
    fn own_virtual_devices_are_recognised() {
        assert!(is_hush_virtual(InputId::new(
            BusType::BUS_VIRTUAL,
            VIRTUAL_VENDOR,
            VIRTUAL_PRODUCT_PROXY,
            1
        )));
        assert!(is_hush_virtual(InputId::new(
            BusType::BUS_VIRTUAL,
            VIRTUAL_VENDOR,
            VIRTUAL_PRODUCT_KEYBOARD,
            1
        )));
        assert!(!is_hush_virtual(InputId::new(
            BusType::BUS_USB,
            VIRTUAL_VENDOR,
            1,
            1
        )));
        assert!(!is_hush_virtual(InputId::new(
            BusType::BUS_VIRTUAL,
            0x046d,
            1,
            1
        )));
    }

    #[test]
    fn device_enumeration_does_not_panic() {
        let combination = KeyCombination::parse("Ctrl+Shift+Space").unwrap();
        for (path, device) in devices_for(&combination) {
            assert!(path.starts_with("/dev/input"));
            assert!(device.supported_keys().is_some());
            assert!(!is_hush_virtual(device.input_id()));
        }
    }
}
