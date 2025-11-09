/// Test integration of overlay with hotkey system
///
/// This demonstrates push-to-talk functionality:
/// - Press Ctrl+Alt+V to start recording
/// - Release to stop recording and show "processing"
///
/// Usage: cargo run --bin test-overlay-hotkey

use hush::hotkey::{HotkeyManager, HotkeyEvent};
use hush::overlay::{OverlayWindowBuilder, OverlayState, OverlayPosition};
use std::thread;
use std::time::Duration;
use tracing::{info, error, Level};
use tracing_subscriber;

fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("🎤 Hush Overlay + Hotkey Integration Test");
    info!("Press Ctrl+Alt+V to test push-to-talk");
    info!("The overlay will show recording state while you hold the key");

    // Create the hotkey manager
    let (hotkey_manager, hotkey_rx) = match HotkeyManager::new("Ctrl+Alt+V") {
        Ok(result) => result,
        Err(e) => {
            error!("Failed to create hotkey manager: {}", e);
            return;
        }
    };

    // Start listening for hotkeys
    if let Err(e) = hotkey_manager.start_listening() {
        error!("Failed to start hotkey listener: {}", e);
        return;
    }

    info!("Hotkey 'Ctrl+Alt+V' registered successfully");

    // Create the overlay window
    let overlay = OverlayWindowBuilder::new()
        .width(300.0)
        .height(100.0)
        .position(OverlayPosition::BottomRight)
        .opacity(0.95)
        .show_button_when_idle(true)
        .build();

    // Get a handle to update the state
    let state_handle = overlay.state();

    // Spawn a thread to handle hotkey events
    let hotkey_thread = thread::spawn(move || {
        info!("Hotkey handler thread started");

        loop {
            match hotkey_rx.recv() {
                Ok(HotkeyEvent::Pressed) => {
                    info!("🔴 Hotkey PRESSED - Starting recording");
                    *state_handle.lock().unwrap() = OverlayState::start_recording();
                }
                Ok(HotkeyEvent::Released) => {
                    info!("⏹️  Hotkey RELEASED - Processing");
                    *state_handle.lock().unwrap() = OverlayState::processing("Transcribing...");

                    // Simulate transcription delay
                    thread::sleep(Duration::from_secs(2));

                    info!("✅ Transcription complete - Showing success");
                    *state_handle.lock().unwrap() = OverlayState::success(
                        "Hello world from hotkey!",
                        Duration::from_secs(3)
                    );
                }
                Err(e) => {
                    error!("Hotkey receiver error: {}", e);
                    break;
                }
            }
        }

        info!("Hotkey handler thread terminated");
    });

    // Run the overlay (blocking)
    info!("Starting overlay window");
    if let Err(e) = overlay.run() {
        error!("Error running overlay: {}", e);
    }

    // Clean up
    info!("Overlay closed, cleaning up...");
    drop(hotkey_manager); // Explicitly drop to unregister hotkey

    // Wait for hotkey thread to finish
    if let Err(e) = hotkey_thread.join() {
        error!("Failed to join hotkey thread: {:?}", e);
    }

    info!("Test complete");
}
