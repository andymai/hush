/// Test binary for the overlay window
///
/// This tests the basic overlay functionality by cycling through different states.
///
/// Usage: cargo run --bin test-overlay

use hush::overlay::{OverlayWindowBuilder, OverlayState, OverlayPosition};
use std::thread;
use std::time::Duration;
use tracing::{info, Level};
use tracing_subscriber;

fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("Starting overlay test");

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

    // Spawn a thread to cycle through states
    let test_thread = thread::spawn(move || {
        info!("Test thread started - will cycle through states");

        // Wait a bit to see idle state
        thread::sleep(Duration::from_secs(3));
        info!("Transitioning to: Recording");

        // Recording state
        *state_handle.lock().unwrap() = OverlayState::start_recording();
        thread::sleep(Duration::from_secs(5));
        info!("Transitioning to: Processing");

        // Processing state
        *state_handle.lock().unwrap() = OverlayState::processing("Transcribing audio...");
        thread::sleep(Duration::from_secs(3));
        info!("Transitioning to: Success");

        // Success state
        *state_handle.lock().unwrap() = OverlayState::success(
            "Hello world, this is a test transcription!",
            Duration::from_secs(3)
        );
        thread::sleep(Duration::from_secs(4));
        info!("Transitioning to: Error");

        // Error state
        *state_handle.lock().unwrap() = OverlayState::error(
            "Failed to transcribe audio",
            Duration::from_secs(3)
        );
        thread::sleep(Duration::from_secs(4));
        info!("Transitioning back to: Idle");

        // Back to idle
        *state_handle.lock().unwrap() = OverlayState::Idle;
        thread::sleep(Duration::from_secs(3));

        // Recording again
        info!("Second recording cycle");
        *state_handle.lock().unwrap() = OverlayState::start_recording();
        thread::sleep(Duration::from_secs(3));

        *state_handle.lock().unwrap() = OverlayState::processing("Transcribing...");
        thread::sleep(Duration::from_secs(2));

        *state_handle.lock().unwrap() = OverlayState::success(
            "Second transcription successful!",
            Duration::from_secs(2)
        );

        thread::sleep(Duration::from_secs(5));
        info!("Test complete - overlay will remain open until closed manually");
    });

    // Run the overlay (blocking)
    info!("Starting overlay window");
    if let Err(e) = overlay.run() {
        eprintln!("Error running overlay: {}", e);
    }

    // Wait for test thread
    test_thread.join().unwrap();
    info!("Overlay test finished");
}
