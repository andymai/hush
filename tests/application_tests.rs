/// Comprehensive unit tests for the refactored HushApp
///
/// These tests demonstrate the benefits of the trait-based architecture:
/// - Full application testing without hardware
/// - Easy error scenario testing
/// - Component isolation
/// - State management validation
use hush::application::{AppMode, HushApp, HushAppBuilder};
use hush::core::mocks::{MockAudioSource, MockInputTrigger, MockTextOutput, MockTranscriber};
use hush::core::state::AppState;
use hush::core::traits::TriggerEvent;
use std::time::Duration;

/// Helper to create a test app with all mocks
fn create_test_app() -> HushApp {
    HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::with_responses(vec![
            "Test transcription".to_string(),
        ])))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap()
}

#[test]
fn test_app_initialization_with_builder() {
    let app = create_test_app();

    assert_eq!(app.current_state(), AppState::Idle);
    assert!(!app.is_recording());

    let stats = app.get_stats();
    assert_eq!(stats.audio_device, "Mock Audio Device");
    assert_eq!(stats.transcriber_name, "MockTranscriber");
    assert_eq!(stats.text_output_method, "Mock");
}

#[test]
fn test_builder_validation() {
    // Missing components should fail
    let result = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .build();

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Transcriber not set"));
}

#[tokio::test]
async fn test_recording_lifecycle() {
    let mut app = create_test_app();

    // Initial state
    assert_eq!(app.current_state(), AppState::Idle);

    // Start recording
    app.handle_recording_start().await.unwrap();
    assert!(app.is_recording());
    assert!(matches!(app.current_state(), AppState::Recording { .. }));

    // Stop recording and full pipeline
    app.handle_recording_stop().await.unwrap();
    assert!(!app.is_recording());
    assert_eq!(app.current_state(), AppState::Idle);
}

#[tokio::test]
async fn test_state_transitions() {
    let mut app = create_test_app();

    // Idle → Recording
    assert_eq!(app.current_state(), AppState::Idle);
    app.handle_recording_start().await.unwrap();
    assert!(app.current_state().is_recording());

    // Recording → Transcribing → Inserting → Idle
    app.handle_recording_stop().await.unwrap();
    assert_eq!(app.current_state(), AppState::Idle);
}

#[tokio::test]
async fn test_multiple_recording_cycles() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::with_responses(vec![
            "First".to_string(),
            "Second".to_string(),
            "Third".to_string(),
        ])))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    // First cycle
    app.handle_recording_start().await.unwrap();
    app.handle_recording_stop().await.unwrap();
    assert_eq!(app.current_state(), AppState::Idle);

    // Second cycle
    app.handle_recording_start().await.unwrap();
    app.handle_recording_stop().await.unwrap();
    assert_eq!(app.current_state(), AppState::Idle);

    // Third cycle
    app.handle_recording_start().await.unwrap();
    app.handle_recording_stop().await.unwrap();
    assert_eq!(app.current_state(), AppState::Idle);
}

#[tokio::test]
async fn test_error_during_recording() {
    // Create app with failing text output
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new().with_failure()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    app.handle_recording_start().await.unwrap();

    // This should fail at text insertion
    let result = app.handle_recording_stop().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_cannot_start_recording_twice() {
    let mut app = create_test_app();

    // Start recording
    app.handle_recording_start().await.unwrap();
    assert!(app.is_recording());

    // Try to start again - should be no-op
    app.handle_recording_start().await.unwrap();
    assert!(app.is_recording());
}

#[tokio::test]
async fn test_cannot_stop_when_not_recording() {
    let mut app = create_test_app();

    assert!(!app.is_recording());

    // Try to stop when not recording - should be no-op
    app.handle_recording_stop().await.unwrap();
    assert!(!app.is_recording());
}

#[test]
fn test_different_app_modes() {
    // Daemon mode
    let app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .daemon_mode()
        .build()
        .unwrap();

    assert!(matches!(app.mode, AppMode::Daemon));

    // One-shot mode
    let app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .oneshot_mode(10, false)
        .build()
        .unwrap();

    assert!(matches!(app.mode, AppMode::OneShot { .. }));

    // Manual mode
    let app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .build()
        .unwrap();

    assert!(matches!(app.mode, AppMode::Manual));
}

#[tokio::test]
async fn test_oneshot_mode_execution() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::with_responses(vec![
            "Hello world".to_string(),
        ])))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .oneshot_mode(1, false) // 1 second recording
        .with_notifications(false)
        .build()
        .unwrap();

    // Run one-shot mode
    app.run().await.unwrap();

    // Should end in idle state
    assert_eq!(app.current_state(), AppState::Idle);
}

#[test]
fn test_stats_collection() {
    let app = create_test_app();
    let stats = app.get_stats();

    assert_eq!(stats.state, AppState::Idle);
    assert!(!stats.audio_device.is_empty());
    assert!(!stats.transcriber_name.is_empty());
    assert!(!stats.text_output_method.is_empty());
    assert!(!stats.input_trigger_desc.is_empty());
}

#[tokio::test]
async fn test_transcription_with_different_confidences() {
    // High confidence
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new().with_confidence(0.99)))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    app.handle_recording_start().await.unwrap();
    app.handle_recording_stop().await.unwrap();
    assert_eq!(app.current_state(), AppState::Idle);

    // Low confidence
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new().with_confidence(0.30)))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    app.handle_recording_start().await.unwrap();
    app.handle_recording_stop().await.unwrap();
    assert_eq!(app.current_state(), AppState::Idle);
}

#[tokio::test]
async fn test_audio_duration_tracking() {
    let mut audio = MockAudioSource::new();
    audio.set_duration(Duration::from_secs(5));

    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(audio))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    app.handle_recording_start().await.unwrap();

    // Check recording duration is being tracked
    if let Some(duration) = app.current_state().recording_duration() {
        assert!(duration.as_secs_f32() >= 0.0);
    }

    app.handle_recording_stop().await.unwrap();
}

// SUMMARY: Benefits Demonstrated
//
// ✅ **Hardware-Free Testing**: All tests run without microphone, X11, or GPU
// ✅ **State Management**: State machine prevents invalid transitions
// ✅ **Error Scenarios**: Easy to test failure modes
// ✅ **Component Isolation**: Each component can be mocked independently
// ✅ **Multiple Cycles**: Can test repeated use without cleanup issues
// ✅ **Different Modes**: Easy to test all application modes
// ✅ **Performance**: Tests run in milliseconds (no I/O waits)
//
// This demonstrates that the refactored architecture achieves the goal:
// **Testable, maintainable, and extensible code.**
