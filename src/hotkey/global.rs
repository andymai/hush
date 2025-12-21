use crate::Result;
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyManager,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub enum HotkeyEvent {
    Pressed,
    Released,
}

pub struct HotkeyManager {
    manager: Arc<GlobalHotKeyManager>,
    hotkey: HotKey,
    combination: String,
    running: Arc<AtomicBool>,
    start_signal: Arc<(Mutex<bool>, Condvar)>,
}

impl HotkeyManager {
    pub fn new(combination: &str) -> Result<(Self, mpsc::Receiver<HotkeyEvent>)> {
        info!(
            "Initializing global hotkey manager with combination: {}",
            combination
        );

        // Parse the hotkey combination
        let (modifiers, key_code) = Self::parse_combination(combination)?;

        // Create the hotkey
        let hotkey = HotKey::new(Some(modifiers), key_code);

        // Initialize the global hotkey manager
        let manager = GlobalHotKeyManager::new()
            .map_err(|e| anyhow::anyhow!("Failed to create hotkey manager: {:?}", e))?;

        // Register the hotkey
        manager
            .register(hotkey)
            .map_err(|e| anyhow::anyhow!("Failed to register hotkey {}: {:?}", combination, e))?;

        info!("Hotkey '{}' registered successfully", combination);

        let (tx, rx) = mpsc::channel();

        let start_signal = Arc::new((Mutex::new(false), Condvar::new()));

        let hotkey_manager = HotkeyManager {
            manager: Arc::new(manager),
            hotkey,
            combination: combination.to_string(),
            running: Arc::new(AtomicBool::new(false)), // Start as false, will be set by start_listening
            start_signal: start_signal.clone(),
        };

        // Start the event loop in a separate thread
        let manager_clone = hotkey_manager.manager.clone();
        let running_clone = hotkey_manager.running.clone();
        let combination_clone = combination.to_string();
        let start_signal_clone = start_signal.clone();

        thread::spawn(move || {
            Self::event_loop(
                manager_clone,
                running_clone,
                combination_clone,
                start_signal_clone,
                tx,
            );
        });

        Ok((hotkey_manager, rx))
    }

    pub fn start_listening(&self) -> Result<()> {
        info!("Starting hotkey listener for '{}'", self.combination);

        // Set the running flag to true
        self.running.store(true, Ordering::Relaxed);

        // Signal the waiting thread to start
        let (lock, cvar) = &*self.start_signal;
        let mut started = lock.lock().unwrap();
        *started = true;
        cvar.notify_one();

        info!("Hotkey listener started successfully (signaled thread)");
        Ok(())
    }

    pub fn stop_listening(&self) -> Result<()> {
        info!("Stopping hotkey listener for '{}'", self.combination);
        self.running.store(false, Ordering::Relaxed);

        // If the thread is still waiting for start signal, wake it up so it can exit
        let (lock, cvar) = &*self.start_signal;
        let mut started = lock.lock().unwrap();
        *started = true; // Set to true so the waiting thread wakes up
        cvar.notify_one();

        Ok(())
    }

    pub fn get_combination(&self) -> &str {
        &self.combination
    }

    fn parse_combination(combination: &str) -> Result<(Modifiers, Code)> {
        let parts: Vec<&str> = combination.split('+').map(|s| s.trim()).collect();

        if parts.is_empty() {
            return Err(anyhow::anyhow!("Empty hotkey combination"));
        }

        let mut modifiers = Modifiers::empty();
        let mut key_code = None;

        for part in &parts {
            match part.to_lowercase().as_str() {
                "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
                "alt" => modifiers |= Modifiers::ALT,
                "shift" => modifiers |= Modifiers::SHIFT,
                "super" | "cmd" | "win" => modifiers |= Modifiers::SUPER,
                key => {
                    if key_code.is_some() {
                        return Err(anyhow::anyhow!(
                            "Multiple key codes specified: {}",
                            combination
                        ));
                    }
                    key_code = Some(Self::parse_key_code(key)?);
                },
            }
        }

        let key_code =
            key_code.ok_or_else(|| anyhow::anyhow!("No key code specified in: {}", combination))?;

        Ok((modifiers, key_code))
    }

    fn parse_key_code(key: &str) -> Result<Code> {
        let code = match key.to_lowercase().as_str() {
            // Letters
            "a" => Code::KeyA,
            "b" => Code::KeyB,
            "c" => Code::KeyC,
            "d" => Code::KeyD,
            "e" => Code::KeyE,
            "f" => Code::KeyF,
            "g" => Code::KeyG,
            "h" => Code::KeyH,
            "i" => Code::KeyI,
            "j" => Code::KeyJ,
            "k" => Code::KeyK,
            "l" => Code::KeyL,
            "m" => Code::KeyM,
            "n" => Code::KeyN,
            "o" => Code::KeyO,
            "p" => Code::KeyP,
            "q" => Code::KeyQ,
            "r" => Code::KeyR,
            "s" => Code::KeyS,
            "t" => Code::KeyT,
            "u" => Code::KeyU,
            "v" => Code::KeyV,
            "w" => Code::KeyW,
            "x" => Code::KeyX,
            "y" => Code::KeyY,
            "z" => Code::KeyZ,

            // Numbers
            "0" => Code::Digit0,
            "1" => Code::Digit1,
            "2" => Code::Digit2,
            "3" => Code::Digit3,
            "4" => Code::Digit4,
            "5" => Code::Digit5,
            "6" => Code::Digit6,
            "7" => Code::Digit7,
            "8" => Code::Digit8,
            "9" => Code::Digit9,

            // Function keys
            "f1" => Code::F1,
            "f2" => Code::F2,
            "f3" => Code::F3,
            "f4" => Code::F4,
            "f5" => Code::F5,
            "f6" => Code::F6,
            "f7" => Code::F7,
            "f8" => Code::F8,
            "f9" => Code::F9,
            "f10" => Code::F10,
            "f11" => Code::F11,
            "f12" => Code::F12,

            // Special keys
            "space" => Code::Space,
            "enter" | "return" => Code::Enter,
            "escape" | "esc" => Code::Escape,
            "backspace" => Code::Backspace,
            "delete" | "del" => Code::Delete,
            "tab" => Code::Tab,
            "home" => Code::Home,
            "end" => Code::End,
            "pageup" | "pgup" => Code::PageUp,
            "pagedown" | "pgdn" => Code::PageDown,
            "insert" | "ins" => Code::Insert,

            // Arrow keys
            "up" | "arrowup" => Code::ArrowUp,
            "down" | "arrowdown" => Code::ArrowDown,
            "left" | "arrowleft" => Code::ArrowLeft,
            "right" | "arrowright" => Code::ArrowRight,

            // Punctuation
            "," | "comma" => Code::Comma,
            "." | "period" => Code::Period,
            "/" | "slash" => Code::Slash,
            ";" | "semicolon" => Code::Semicolon,
            "'" | "quote" => Code::Quote,
            "[" | "bracketleft" => Code::BracketLeft,
            "]" | "bracketright" => Code::BracketRight,
            "\\" | "backslash" => Code::Backslash,
            "-" | "minus" => Code::Minus,
            "=" | "equal" => Code::Equal,
            "`" | "backquote" => Code::Backquote,

            unknown => {
                return Err(anyhow::anyhow!("Unknown key: {}", unknown));
            },
        };

        Ok(code)
    }

    fn event_loop(
        _manager: Arc<GlobalHotKeyManager>,
        running: Arc<AtomicBool>,
        combination: String,
        start_signal: Arc<(Mutex<bool>, Condvar)>,
        tx: mpsc::Sender<HotkeyEvent>,
    ) {
        info!("Starting hotkey event loop for '{}'", combination);

        // Wait for the start signal from start_listening()
        let (lock, cvar) = &*start_signal;
        let mut started = lock.lock().unwrap();
        while !*started {
            info!("Event loop waiting for start signal for '{}'", combination);
            started = cvar.wait(started).unwrap();
        }
        info!("Event loop received start signal for '{}'", combination);

        // Get the global receiver for hotkey events
        let receiver = global_hotkey::GlobalHotKeyEvent::receiver();
        info!("Got global hotkey receiver for '{}'", combination);

        // Event loop using blocking recv with timeout (no busy-wait)
        loop {
            if !running.load(Ordering::Relaxed) {
                info!(
                    "Hotkey event loop stopping for '{}' (running = false)",
                    combination
                );
                break;
            }

            // Block on recv with timeout - no busy-wait, CPU sleeps until event or timeout
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(event) => match event.state {
                    global_hotkey::HotKeyState::Pressed => {
                        info!("Hotkey '{}' pressed", combination);
                        if tx.send(HotkeyEvent::Pressed).is_err() {
                            warn!("Failed to send hotkey pressed event - receiver dropped");
                            break;
                        }
                    },
                    global_hotkey::HotKeyState::Released => {
                        info!("Hotkey '{}' released", combination);
                        if tx.send(HotkeyEvent::Released).is_err() {
                            warn!("Failed to send hotkey released event - receiver dropped");
                            break;
                        }
                    },
                },
                Err(e) => {
                    // Check if it's a timeout (continue) or disconnect (break)
                    if e.is_timeout() {
                        continue;
                    } else {
                        info!("Hotkey receiver disconnected for '{}'", combination);
                        break;
                    }
                },
            }
        }

        info!("Hotkey event loop terminated for '{}'", combination);
    }
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        info!("Dropping hotkey manager for '{}'", self.combination);

        // Stop the listening thread
        if let Err(e) = self.stop_listening() {
            warn!("Failed to stop hotkey listener during drop: {:?}", e);
        }

        // Unregister the hotkey
        if let Err(e) = self.manager.unregister(self.hotkey) {
            warn!(
                "Failed to unregister hotkey '{}': {:?}",
                self.combination, e
            );
        } else {
            info!("Hotkey '{}' unregistered successfully", self.combination);
        }
    }
}
