//! Reads the next key or button from `/dev/input` for a hotkey field, so the
//! chord is captured exactly as the daemon will see it: bare modifiers, F13
//! and above, and mouse buttons included.

use crate::hotkey::combination::Modifiers;
use crate::hotkey::evdev::is_hush_virtual;
use crate::hotkey::KeyCombination;
use anyhow::{anyhow, Result};
use evdev::{Device, EventSummary, KeyCode};
use parking_lot::Mutex;
use std::collections::HashSet;
use std::io;
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const POLL_MS: i32 = 50;
const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Listening,
    Chord(String),
    Ended,
}

pub struct Capture {
    outcome: Arc<Mutex<Outcome>>,
    stop: Arc<AtomicBool>,
}

impl Capture {
    pub fn start() -> Result<Self> {
        let devices: Vec<Device> = evdev::enumerate()
            .map(|(_, device)| device)
            .filter(|device| !is_hush_virtual(device.input_id()))
            .filter(|device| device.supported_keys().is_some())
            .collect();
        if devices.is_empty() {
            return Err(anyhow!("No readable keyboard or mouse under /dev/input"));
        }
        let outcome = Arc::new(Mutex::new(Outcome::Listening));
        let stop = Arc::new(AtomicBool::new(false));
        let shared = Arc::clone(&outcome);
        let stop_flag = Arc::clone(&stop);
        thread::Builder::new()
            .name("hush-key-capture".into())
            .spawn(move || {
                let chord = listen(devices, &stop_flag);
                *shared.lock() = match chord {
                    Some(chord) => Outcome::Chord(chord.to_string()),
                    None => Outcome::Ended,
                };
            })?;
        Ok(Self { outcome, stop })
    }

    /// The result so far; a chord is handed over once.
    pub fn outcome(&self) -> Outcome {
        let mut outcome = self.outcome.lock();
        match &*outcome {
            Outcome::Listening => Outcome::Listening,
            Outcome::Ended => Outcome::Ended,
            Outcome::Chord(_) => std::mem::replace(&mut *outcome, Outcome::Ended),
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

/// The chord is the last key pressed plus the modifiers held at that moment,
/// reported when that key is released. A modifier released on its own is a
/// chord too, which is how a bare `RightAlt` is captured.
fn listen(mut devices: Vec<Device>, stop: &AtomicBool) -> Option<KeyCombination> {
    let started = Instant::now();
    let mut held: HashSet<KeyCode> = HashSet::new();
    let mut candidate: Option<(KeyCode, Modifiers)> = None;
    while !stop.load(Ordering::Acquire) && started.elapsed() < TIMEOUT && !devices.is_empty() {
        let ready = poll_all(&devices, POLL_MS).ok()?;
        let mut gone = Vec::new();
        for index in ready {
            let events = match devices[index].fetch_events() {
                Ok(events) => events,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(_) => {
                    gone.push(index);
                    continue;
                },
            };
            for event in events {
                let EventSummary::Key(_, code, value) = event.destructure() else {
                    continue;
                };
                match value {
                    1 => {
                        let mut modifiers = Modifiers::default();
                        for key in &held {
                            modifiers.add_key(*key);
                        }
                        held.insert(code);
                        candidate = Some((code, modifiers));
                    },
                    0 => {
                        held.remove(&code);
                        if let Some((key, modifiers)) = candidate {
                            if key == code {
                                candidate = None;
                                if let Some(chord) = KeyCombination::from_evdev(key, modifiers) {
                                    return Some(chord);
                                }
                            }
                        }
                    },
                    _ => {},
                }
            }
        }
        for index in gone.into_iter().rev() {
            devices.remove(index);
        }
    }
    None
}

/// Indices of the devices with something to read, after at most `timeout_ms`.
fn poll_all(devices: &[Device], timeout_ms: i32) -> io::Result<Vec<usize>> {
    let mut fds: Vec<libc::pollfd> = devices
        .iter()
        .map(|device| libc::pollfd {
            fd: device.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        })
        .collect();
    // SAFETY: fds is a valid slice of initialised pollfd structs and nfds is its length.
    let ready = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout_ms) };
    if ready < 0 {
        let err = io::Error::last_os_error();
        return if err.kind() == io::ErrorKind::Interrupted {
            Ok(Vec::new())
        } else {
            Err(err)
        };
    }
    Ok(fds
        .iter()
        .enumerate()
        .filter(|(_, fd)| fd.revents & (libc::POLLIN | libc::POLLERR | libc::POLLHUP) != 0)
        .map(|(index, _)| index)
        .collect())
}
