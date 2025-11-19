/// Test binary for the overlay window
///
/// This tests the basic overlay functionality by cycling through different states.
///
/// Usage: cargo run --bin test-overlay
use hush::overlay::{OverlayState, OverlayWindowBuilder};
use std::thread;
use std::time::Duration;
use tracing::{info, Level};
use tracing_subscriber;

fn main() {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting overlay test");

    // Create the overlay window with new minimal defaults
    let overlay = OverlayWindowBuilder::new().build(); // Use defaults: 120x60, BottomCenter

    // Get a handle to update the state
    let state_handle = overlay.state();

    // Spawn a thread to cycle through states and simulate amplitude
    let test_thread = thread::spawn(move || {
        info!("Test thread started - will cycle through states");

        // Wait a bit to see idle state (horizontal line)
        thread::sleep(Duration::from_secs(3));
        info!("Transitioning to: Recording with simulated amplitude");

        // Recording state with amplitude animation
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            let t = start.elapsed().as_secs_f32();
            // Simulate varying amplitude (like someone speaking)
            let amplitude = ((t * 2.0).sin() * 0.5 + 0.5) * 0.8; // 0.0 to 0.8
            *state_handle.lock() = OverlayState::start_recording().with_amplitude(amplitude);
            thread::sleep(Duration::from_millis(50)); // Update 20 times per second
        }
        info!("Transitioning to: Processing");

        // Processing state
        *state_handle.lock() = OverlayState::processing("Transcribing audio...");
        thread::sleep(Duration::from_secs(3));
        info!("Transitioning to: Success");

        // Success state
        *state_handle.lock() = OverlayState::success(
            "Hello world, this is a test transcription!",
            Duration::from_secs(3),
        );
        thread::sleep(Duration::from_secs(4));
        info!("Transitioning to: Error");

        // Error state
        *state_handle.lock() =
            OverlayState::error("Failed to transcribe audio", Duration::from_secs(3));
        thread::sleep(Duration::from_secs(4));
        info!("Transitioning back to: Idle");

        // Back to idle
        *state_handle.lock() = OverlayState::Idle;
        thread::sleep(Duration::from_secs(3));

        // Recording again with different amplitude pattern
        info!("Second recording cycle with faster speech simulation");
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(3) {
            let t = start.elapsed().as_secs_f32();
            // Faster, more variable amplitude (excited speech)
            let amplitude = ((t * 5.0).sin().abs() * 0.9).max(0.2); // 0.2 to 0.9
            *state_handle.lock() = OverlayState::start_recording().with_amplitude(amplitude);
            thread::sleep(Duration::from_millis(50));
        }

        *state_handle.lock() = OverlayState::processing("Transcribing...");
        thread::sleep(Duration::from_secs(2));

        *state_handle.lock() =
            OverlayState::success("Second transcription successful!", Duration::from_secs(2));

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
