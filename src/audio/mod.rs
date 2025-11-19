/// Audio capture and feedback system for Hush voice-to-text
///
/// This module provides real-time audio recording with automatic device selection,
/// pre-allocated buffers for performance, and RMS amplitude calculation for UI feedback.
///
/// # Components
///
/// - [`AudioCapture`] - Records audio from microphone with optimized buffering
/// - [`AudioFeedback`] - Provides audio feedback (beeps, tones) to the user
///
/// # Examples
///
/// ```no_run
/// use hush::audio::AudioCapture;
/// use std::time::Duration;
///
/// // Create audio capture with default device
/// let mut capture = AudioCapture::new(None).expect("Failed to initialize audio");
///
/// // Start recording
/// capture.start_recording().expect("Failed to start recording");
///
/// // Record for 3 seconds
/// std::thread::sleep(Duration::from_secs(3));
///
/// // Stop and get samples
/// let samples = capture.stop_recording().expect("Failed to stop recording");
/// println!("Recorded {} samples", samples.len());
/// ```
///
/// # Performance
///
/// The audio capture system pre-allocates buffers (60 seconds at 16kHz) to avoid
/// allocations during recording, ensuring low-latency capture suitable for
/// real-time voice input.
///
/// # Platform Support
///
/// Currently supports Linux via ALSA/PulseAudio through the `cpal` library.

pub mod capture;
pub mod feedback;

pub use capture::AudioCapture;
pub use feedback::AudioFeedback;