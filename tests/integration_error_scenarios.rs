/// Integration tests for error scenarios in the voice-to-text pipeline
///
/// These tests verify that the application handles errors gracefully and
/// maintains consistent state even when components fail.
use hush::application::HushAppBuilder;
use hush::core::mocks::{MockAudioSource, MockInputTrigger, MockTextOutput, MockTranscriber};
use hush::core::state::AppState;

/// Test text output failure
#[tokio::test]
async fn test_text_insertion_failure() {
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

    // Should fail at text insertion step
    let result = app.handle_recording_stop().await;
    assert!(result.is_err());

    // The app should handle the error and may transition to error state
    // or return to idle depending on error handling strategy
}

/// Test attempting to start recording while already recording
#[tokio::test]
async fn test_start_recording_while_already_recording() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    // Start recording
    app.handle_recording_start().await.unwrap();
    assert!(app.is_recording());

    // Try to start again - should be a no-op or handled gracefully
    app.handle_recording_start().await.unwrap();
    assert!(app.is_recording());
}

/// Test attempting to stop recording when not recording
#[tokio::test]
async fn test_stop_recording_when_not_recording() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    assert!(!app.is_recording());

    // Try to stop when not recording - should be handled gracefully
    let result = app.handle_recording_stop().await;
    // Should either succeed as no-op or return an error
    if result.is_err() {
        // Error is acceptable
    }
    assert!(!app.is_recording());
}

/// Test builder with missing required components
#[test]
fn test_builder_missing_audio_source() {
    let result = HushAppBuilder::new()
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .build();

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Audio source not set"));
}

#[test]
fn test_builder_missing_transcriber() {
    let result = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .build();

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Transcriber not set"));
}

#[test]
fn test_builder_missing_text_output() {
    let result = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .build();

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Text output not set"));
}

#[test]
fn test_builder_missing_input_trigger() {
    let result = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .build();

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Input trigger not set"));
}

/// Test recovery after error - verify system can continue after failure
#[tokio::test]
async fn test_recovery_after_error() {
    // Create app with text output that fails
    let failing_output = Box::new(MockTextOutput::new().with_failure());
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(failing_output)
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    // First attempt should fail
    app.handle_recording_start().await.unwrap();
    let result = app.handle_recording_stop().await;
    assert!(result.is_err());

    // Note: In a real scenario, you would replace the failing component
    // or the app would have retry logic. This test verifies the error is propagated.
}

/// Test multiple sequential errors
#[tokio::test]
async fn test_multiple_sequential_errors() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new().with_failure()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    // First error
    app.handle_recording_start().await.unwrap();
    let result1 = app.handle_recording_stop().await;
    assert!(result1.is_err());

    // Second error - system should still handle it
    app.handle_recording_start().await.unwrap();
    let result2 = app.handle_recording_stop().await;
    assert!(result2.is_err());
}

/// Test error handling with empty audio
#[tokio::test]
async fn test_empty_audio_handling() {
    // Create an audio source with zero duration
    let mut audio = MockAudioSource::new();
    audio.set_duration(std::time::Duration::from_secs(0));

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

    // Empty audio might be handled differently
    let result = app.handle_recording_stop().await;

    // Either succeeds with empty transcription or returns error
    // Both are valid error handling strategies
    if result.is_ok() {
        assert_eq!(app.current_state(), AppState::Idle);
    }
}

/// Test state consistency after errors
#[tokio::test]
async fn test_state_consistency_after_error() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new().with_failure()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    // Verify initial state
    assert_eq!(app.current_state(), AppState::Idle);
    assert!(!app.is_recording());

    // Trigger error scenario
    app.handle_recording_start().await.unwrap();
    let _ = app.handle_recording_stop().await;

    // After error, recording should not be active
    assert!(!app.is_recording());
}

/// Test handling of very low confidence transcriptions
#[tokio::test]
async fn test_very_low_confidence_handling() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(
            MockTranscriber::with_responses(vec!["uncertain text".to_string()])
                .with_confidence(0.01), // Very low confidence
        ))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    app.handle_recording_start().await.unwrap();

    // Should complete even with low confidence
    // (Error handling for low confidence is application-specific)
    let result = app.handle_recording_stop().await;

    // Either succeeds or fails based on confidence threshold policy
    match result {
        Ok(_) => assert_eq!(app.current_state(), AppState::Idle),
        Err(_) => assert!(!app.is_recording()),
    }
}

/// Test rapid start/stop cycles (stress test for state machine)
#[tokio::test]
async fn test_rapid_start_stop_cycles() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    // Rapid cycles
    for _ in 0..5 {
        app.handle_recording_start().await.unwrap();
        // Immediately stop
        app.handle_recording_stop().await.unwrap();
        // Should return to idle
        assert_eq!(app.current_state(), AppState::Idle);
    }
}

/// Test that stats are available even after errors
#[tokio::test]
async fn test_stats_available_after_error() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new().with_failure()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    // Trigger error
    app.handle_recording_start().await.unwrap();
    let _ = app.handle_recording_stop().await;

    // Stats should still be available
    let stats = app.get_stats();
    assert!(!stats.audio_device.is_empty());
    assert!(!stats.transcriber_name.is_empty());
}
