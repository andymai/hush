/// Overlay window management for Hush voice-to-text
///
/// This module provides a floating overlay window with visual feedback
/// for voice recording, transcription, and text insertion operations.
///
/// # Example
///
/// ```no_run
/// use hush::overlay::{OverlayWindow, OverlayWindowBuilder, OverlayState};
/// use std::time::Duration;
///
/// // Create an overlay window
/// let overlay = OverlayWindowBuilder::new()
///     .width(300.0)
///     .height(100.0)
///     .show_button_when_idle(true)
///     .build();
///
/// // Get state handle for updates
/// let state_handle = overlay.state();
///
/// // Run the overlay (blocking)
/// std::thread::spawn(move || {
///     overlay.run().expect("Failed to run overlay");
/// });
///
/// // Update state from another thread
/// *state_handle.lock().unwrap() = OverlayState::start_recording();
/// std::thread::sleep(Duration::from_secs(3));
/// *state_handle.lock().unwrap() = OverlayState::processing("Transcribing...");
/// std::thread::sleep(Duration::from_secs(2));
/// *state_handle.lock().unwrap() = OverlayState::success(
///     "Hello world",
///     Duration::from_secs(2)
/// );
/// ```

mod state;
mod ui;
mod window;

pub use state::{OverlayState, OverlayConfig, OverlayPosition, OverlayTheme};
pub use window::{OverlayWindow, OverlayWindowBuilder};
