/// Integration tests for state machine transitions in realistic scenarios
///
/// These tests verify that the state machine correctly manages transitions
/// through the full application lifecycle.
use hush::application::HushAppBuilder;
use hush::core::mocks::{MockAudioSource, MockInputTrigger, MockTextOutput, MockTranscriber};
use hush::core::state::AppState;
use std::time::Duration;

/// Test valid state transition: Idle → Recording → Transcribing → Inserting → Idle
#[tokio::test]
async fn test_full_state_transition_cycle() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    // Start: Idle
    assert_eq!(app.current_state(), AppState::Idle);
    assert!(!app.is_recording());

    // Transition: Idle → Recording
    app.handle_recording_start().await.unwrap();
    assert!(app.current_state().is_recording());
    assert!(app.is_recording());

    // Transition: Recording → (Transcribing → Inserting) → Idle
    app.handle_recording_stop().await.unwrap();
    assert_eq!(app.current_state(), AppState::Idle);
    assert!(!app.is_recording());
}

/// Test that state is Idle after initialization
#[test]
fn test_initial_state_is_idle() {
    let app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .build()
        .unwrap();

    assert_eq!(app.current_state(), AppState::Idle);
}

/// Test Recording state includes timestamp
#[tokio::test]
async fn test_recording_state_includes_timestamp() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    app.handle_recording_start().await.unwrap();

    // Recording state should track duration
    if let Some(_duration) = app.current_state().recording_duration() {
        // Duration exists and is being tracked
        assert!(app.is_recording());
    } else {
        panic!("Recording state should track duration");
    }
}

/// Test multiple state transition cycles
#[tokio::test]
async fn test_multiple_state_cycles() {
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

    for _ in 0..3 {
        // Idle → Recording
        assert_eq!(app.current_state(), AppState::Idle);
        app.handle_recording_start().await.unwrap();
        assert!(app.is_recording());

        // Recording → Idle (through transcription and insertion)
        app.handle_recording_stop().await.unwrap();
        assert_eq!(app.current_state(), AppState::Idle);
    }
}

/// Test state consistency with is_recording() method
#[tokio::test]
async fn test_state_consistency_with_is_recording() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    // Idle: not recording
    assert_eq!(app.current_state(), AppState::Idle);
    assert!(!app.is_recording());

    // Recording: is recording
    app.handle_recording_start().await.unwrap();
    assert!(app.current_state().is_recording());
    assert!(app.is_recording());

    // Back to Idle: not recording
    app.handle_recording_stop().await.unwrap();
    assert_eq!(app.current_state(), AppState::Idle);
    assert!(!app.is_recording());
}

/// Test state in one-shot mode
#[tokio::test]
async fn test_state_transitions_in_oneshot_mode() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .oneshot_mode(1, false)
        .with_notifications(false)
        .build()
        .unwrap();

    // Initial state
    assert_eq!(app.current_state(), AppState::Idle);

    // Run one-shot (will go through all states automatically)
    app.run().await.unwrap();

    // Should end in Idle
    assert_eq!(app.current_state(), AppState::Idle);
}

/// Test state after error in text insertion
#[tokio::test]
async fn test_state_after_text_insertion_error() {
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
    assert!(app.is_recording());

    // Error during text insertion
    let result = app.handle_recording_stop().await;
    assert!(result.is_err());

    // Should not be recording after error
    assert!(!app.is_recording());
}

/// Test that attempting to record while already recording is handled
#[tokio::test]
async fn test_cannot_transition_to_recording_while_recording() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    app.handle_recording_start().await.unwrap();
    assert!(app.is_recording());

    // Try to start again
    app.handle_recording_start().await.unwrap();

    // Should still be recording (no invalid transition)
    assert!(app.is_recording());
}

/// Test state machine with different audio durations
#[tokio::test]
async fn test_state_machine_with_different_durations() {
    // Short duration
    let mut short_audio = MockAudioSource::new();
    short_audio.set_duration(Duration::from_millis(500));

    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(short_audio))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    app.handle_recording_start().await.unwrap();
    app.handle_recording_stop().await.unwrap();
    assert_eq!(app.current_state(), AppState::Idle);

    // Long duration
    let mut long_audio = MockAudioSource::new();
    long_audio.set_duration(Duration::from_secs(15));

    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(long_audio))
        .with_transcriber(Box::new(MockTranscriber::new()))
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

/// Test state is reflected in stats
#[test]
fn test_state_reflected_in_stats() {
    let app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .build()
        .unwrap();

    let stats = app.get_stats();
    assert_eq!(stats.state, AppState::Idle);
}

/// Test state transitions with processing delays
#[tokio::test]
async fn test_state_transitions_with_delays() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(
            MockTranscriber::new().with_delay(Duration::from_millis(100)),
        ))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    // Even with delays, state machine should work correctly
    app.handle_recording_start().await.unwrap();
    assert!(app.is_recording());

    app.handle_recording_stop().await.unwrap();
    assert_eq!(app.current_state(), AppState::Idle);
}

/// Test rapid state transitions (stress test)
#[tokio::test]
async fn test_rapid_state_transitions() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    // Rapid transitions
    for _ in 0..10 {
        assert_eq!(app.current_state(), AppState::Idle);
        app.handle_recording_start().await.unwrap();
        assert!(app.is_recording());
        app.handle_recording_stop().await.unwrap();
    }

    // Should end in stable state
    assert_eq!(app.current_state(), AppState::Idle);
}

/// Test state machine handles alternating success and failure
#[tokio::test]
async fn test_state_machine_with_alternating_success_failure() {
    // Success
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    app.handle_recording_start().await.unwrap();
    app.handle_recording_stop().await.unwrap();
    assert_eq!(app.current_state(), AppState::Idle);

    // Failure (create new app with failing component)
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
    let result = app.handle_recording_stop().await;
    assert!(result.is_err());
    assert!(!app.is_recording());
}

/// Test state helpers (is_recording)
#[tokio::test]
async fn test_state_helper_methods() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    // Test is_recording helper
    assert!(!app.is_recording());
    app.handle_recording_start().await.unwrap();
    assert!(app.is_recording());
    app.handle_recording_stop().await.unwrap();
    assert!(!app.is_recording());
}
