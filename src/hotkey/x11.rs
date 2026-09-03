//! X11 hotkeys through XGrabKey, via the global-hotkey crate. The key is
//! grabbed exclusively, so the focused application never sees it. Used when
//! `/dev/input` is not readable and an X11 display is available.

use super::combination::KeyCombination;
use super::HotkeyEvent;
use crate::Result;
use global_hotkey::{hotkey::HotKey, GlobalHotKeyManager};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;
use tracing::{info, warn};

pub struct X11Hotkey {
    manager: Arc<GlobalHotKeyManager>,
    hotkey: HotKey,
    combination: KeyCombination,
    running: Arc<AtomicBool>,
    sender: mpsc::Sender<HotkeyEvent>,
}

impl X11Hotkey {
    pub fn new(combination: KeyCombination) -> Result<(Self, mpsc::Receiver<HotkeyEvent>)> {
        let hotkey = HotKey::new(Some(combination.x11_modifiers()), combination.x11_code());
        let manager = GlobalHotKeyManager::new()
            .map_err(|e| anyhow::anyhow!("Failed to create X11 hotkey manager: {:?}", e))?;
        manager.register(hotkey).map_err(|e| {
            anyhow::anyhow!("Failed to register X11 hotkey {}: {:?}", combination, e)
        })?;
        info!("X11 hotkey '{}' registered", combination);

        let (sender, receiver) = mpsc::channel();
        Ok((
            Self {
                manager: Arc::new(manager),
                hotkey,
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

    pub fn start_listening(&self) -> Result<()> {
        if self.running.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let running = Arc::clone(&self.running);
        let sender = self.sender.clone();
        let combination = self.combination.to_string();
        thread::Builder::new()
            .name("hush-hotkey-x11".into())
            .spawn(move || event_loop(running, combination, sender))
            .map_err(|e| anyhow::anyhow!("Failed to start hotkey thread: {}", e))?;
        Ok(())
    }

    pub fn stop_listening(&self) -> Result<()> {
        self.running.store(false, Ordering::Release);
        Ok(())
    }
}

fn event_loop(running: Arc<AtomicBool>, combination: String, sender: mpsc::Sender<HotkeyEvent>) {
    let receiver = global_hotkey::GlobalHotKeyEvent::receiver();
    while running.load(Ordering::Acquire) {
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(event) => {
                let hotkey_event = match event.state {
                    global_hotkey::HotKeyState::Pressed => HotkeyEvent::Pressed,
                    global_hotkey::HotKeyState::Released => HotkeyEvent::Released,
                };
                if sender.send(hotkey_event).is_err() {
                    warn!("Hotkey receiver dropped; stopping '{}'", combination);
                    break;
                }
            },
            Err(e) if e.is_timeout() => continue,
            Err(_) => {
                info!("X11 hotkey receiver disconnected for '{}'", combination);
                break;
            },
        }
    }
}

impl Drop for X11Hotkey {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        if let Err(e) = self.manager.unregister(self.hotkey) {
            warn!(
                "Failed to unregister X11 hotkey '{}': {:?}",
                self.combination, e
            );
        }
    }
}
