use crate::core::traits::{TextOutput, WindowInfo};
use crate::Result;
use async_trait::async_trait;
use core_graphics::event::{CGEvent, CGEventTapLocation, CGKeyCode};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tracing::{debug, info, warn};

mod accessibility;
pub use accessibility::{
    char_to_keycode, check_accessibility_permissions, get_focused_window_info,
    prompt_accessibility_permissions,
};

/// macOS text insertion adapter using CGEvent API
///
/// This adapter uses CoreGraphics CGEvent API for keyboard simulation,
/// which provides system-level keyboard events that work in most applications.
///
/// # Permissions
///
/// Requires Accessibility permissions to be granted in System Preferences.
/// Will prompt user on first run if permissions are not available.
///
/// # Fallback
///
/// If CGEvent insertion fails, the adapter will fall back to the enigo
/// library for cross-platform keyboard simulation.
///
/// # Thread Safety
///
/// The CGEventSource is wrapped in Arc to make it Send + Sync, which is
/// required for async trait methods that may be called across threads.
pub struct MacOSTextAdapter {
    event_source: Arc<CGEventSource>,
    enigo_fallback: enigo::Enigo,
    accessibility_enabled: bool,
    typing_delay_ms: u64,
}

// SAFETY: CGEventSource is thread-safe when wrapped in Arc, and enigo::Enigo
// is safe to send across threads as it doesn't hold thread-local state on macOS.
// The macOS CGEvent API can be called from any thread.
unsafe impl Send for MacOSTextAdapter {}
unsafe impl Sync for MacOSTextAdapter {}

impl MacOSTextAdapter {
    /// Create a new macOS text adapter
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - CGEventSource cannot be created
    /// - enigo fallback cannot be initialized
    pub fn new() -> Result<Self> {
        info!("Initializing macOS text insertion adapter");

        // Check Accessibility permissions
        let accessibility_enabled = check_accessibility_permissions();
        if !accessibility_enabled {
            warn!("Accessibility permissions not granted");
            warn!("Text insertion will use fallback method");
            prompt_accessibility_permissions();
        } else {
            info!("Accessibility permissions granted");
        }

        // Create CGEvent source (system-level events)
        let event_source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|e| anyhow::anyhow!("Failed to create CGEventSource: {:?}", e))?;

        // Initialize enigo as fallback
        let enigo_fallback = enigo::Enigo::new(&enigo::Settings::default())
            .map_err(|e| anyhow::anyhow!("Failed to initialize enigo fallback: {:?}", e))?;

        Ok(Self {
            event_source: Arc::new(event_source),
            enigo_fallback,
            accessibility_enabled,
            typing_delay_ms: 10,
        })
    }

    /// Insert text using CGEvent API
    fn insert_with_cgevent(&mut self, text: &str) -> Result<()> {
        debug!("Inserting text using CGEvent API: {} chars", text.len());

        for ch in text.chars() {
            self.type_character(ch)?;
            if self.typing_delay_ms > 0 {
                thread::sleep(Duration::from_millis(self.typing_delay_ms));
            }
        }

        Ok(())
    }

    /// Type a single character using CGEvent
    fn type_character(&mut self, ch: char) -> Result<()> {
        // Convert character to keycode and modifiers
        let (keycode, needs_shift) = char_to_keycode(ch)?;

        // Press shift if needed
        if needs_shift {
            self.press_key(56, true)?; // 56 = Shift
        }

        // Press and release key
        self.press_key(keycode, true)?;
        self.press_key(keycode, false)?;

        // Release shift
        if needs_shift {
            self.press_key(56, false)?;
        }

        Ok(())
    }

    /// Press or release a key
    fn press_key(&self, keycode: CGKeyCode, key_down: bool) -> Result<()> {
        let event = CGEvent::new_keyboard_event((*self.event_source).clone(), keycode, key_down)
            .map_err(|e| anyhow::anyhow!("Failed to create keyboard event: {:?}", e))?;

        event.post(CGEventTapLocation::HID);
        Ok(())
    }

    /// Insert text using enigo fallback
    fn insert_with_enigo(&mut self, text: &str) -> Result<()> {
        use enigo::{Enigo, Keyboard, Settings};

        debug!("Inserting text using enigo fallback: {} chars", text.len());

        self.enigo_fallback
            .text(text)
            .map_err(|e| anyhow::anyhow!("Failed to insert text with enigo: {:?}", e))?;

        Ok(())
    }
}

#[async_trait]
impl TextOutput for MacOSTextAdapter {
    async fn insert_text(&mut self, text: &str) -> Result<()> {
        info!("Inserting text (macOS): {} chars", text.len());

        // Try CGEvent first if permissions are granted
        if self.accessibility_enabled {
            match self.insert_with_cgevent(text) {
                Ok(()) => {
                    debug!("Text inserted successfully via CGEvent");
                    return Ok(());
                },
                Err(e) => {
                    warn!("CGEvent insertion failed: {}, falling back to enigo", e);
                },
            }
        }

        // Fallback to enigo
        debug!("Using enigo fallback");
        self.insert_with_enigo(text)
    }

    async fn focused_window(&self) -> Result<Option<WindowInfo>> {
        match get_focused_window_info() {
            Ok(window_info) => Ok(Some(WindowInfo {
                title: window_info.title,
                class: window_info.bundle_id.clone(),
                app_name: window_info.app_name,
            })),
            Err(e) => {
                debug!("Failed to get focused window info: {}", e);
                Ok(None)
            },
        }
    }

    fn is_available(&self) -> bool {
        // Adapter is available if we successfully created it
        true
    }

    fn output_method(&self) -> &str {
        if self.accessibility_enabled {
            "macOS CGEvent"
        } else {
            "macOS enigo fallback"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macos_adapter_creation() {
        // Test that we can create the adapter
        if let Ok(adapter) = MacOSTextAdapter::new() {
            assert!(adapter.is_available());
            assert!(
                adapter.output_method() == "macOS CGEvent"
                    || adapter.output_method() == "macOS enigo fallback"
            );
        }
    }

    #[test]
    fn test_adapter_implements_trait() {
        // Verify the adapter compiles as trait object
        let _can_box: Result<Box<dyn TextOutput>> =
            MacOSTextAdapter::new().map(|adapter| Box::new(adapter) as Box<dyn TextOutput>);
    }
}
