/// Integration tests for the complete voice-to-text pipeline
///
/// These tests verify the full workflow from audio capture through transcription
/// to text output, using mock implementations to avoid hardware dependencies.
use hush::application::{AppMode, HushAppBuilder};
use hush::core::mocks::{MockAudioSource, MockInputTrigger, MockTextOutput, MockTranscriber};
use hush::core::state::AppState;
use hush::core::traits::TriggerEvent;
use std::time::Duration;

/// Test the complete pipeline: audio → transcription → text output
#[tokio::test]
async fn test_complete_voice_to_text_pipeline() {
    // Setup mocks
    let audio = Box::new(MockAudioSource::new());
    let transcriber = Box::new(MockTranscriber::with_responses(vec![
        "Hello world".to_string()
    ]));
    let text_output = Box::new(MockTextOutput::new());

    // Build app
    let mut app = HushAppBuilder::new()
        .with_audio(audio)
        .with_transcriber(transcriber)
        .with_text_output(text_output)
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    // Verify initial state
    assert_eq!(app.current_state(), AppState::Idle);

    // Start recording
    app.handle_recording_start().await.unwrap();
    assert!(app.is_recording());

    // Stop recording and process through full pipeline
    app.handle_recording_stop().await.unwrap();

    // Verify final state
    assert_eq!(app.current_state(), AppState::Idle);
    assert!(!app.is_recording());
}

/// Test one-shot mode execution (record for fixed duration)
#[tokio::test]
async fn test_oneshot_mode_pipeline() {
    let mut audio = MockAudioSource::new();
    audio.set_duration(Duration::from_secs(3));

    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(audio))
        .with_transcriber(Box::new(MockTranscriber::with_responses(vec![
            "One shot transcription".to_string(),
        ])))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .oneshot_mode(3, false)
        .with_notifications(false)
        .build()
        .unwrap();

    // Run one-shot mode
    app.run().await.unwrap();

    // Should end in idle state
    assert_eq!(app.current_state(), AppState::Idle);
}

/// Test one-shot mode with print-only option
#[tokio::test]
async fn test_oneshot_mode_print_only() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::with_responses(vec![
            "Print only transcription".to_string(),
        ])))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .oneshot_mode(1, true) // print_only = true
        .with_notifications(false)
        .build()
        .unwrap();

    app.run().await.unwrap();
    assert_eq!(app.current_state(), AppState::Idle);
}

/// Test multiple recording cycles in sequence
#[tokio::test]
async fn test_multiple_recording_cycles() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::with_responses(vec![
            "First recording".to_string(),
            "Second recording".to_string(),
            "Third recording".to_string(),
        ])))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    // First cycle
    app.handle_recording_start().await.unwrap();
    assert!(app.is_recording());
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

/// Test pipeline with different audio durations
#[tokio::test]
async fn test_different_audio_durations() {
    // Short recording (1 second)
    let mut short_audio = MockAudioSource::new();
    short_audio.set_duration(Duration::from_secs(1));

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

    // Long recording (10 seconds)
    let mut long_audio = MockAudioSource::new();
    long_audio.set_duration(Duration::from_secs(10));

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

/// Test pipeline with high confidence transcription
#[tokio::test]
async fn test_high_confidence_transcription() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(
            MockTranscriber::with_responses(vec!["High confidence text".to_string()])
                .with_confidence(0.98),
        ))
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

/// Test pipeline with low confidence transcription
#[tokio::test]
async fn test_low_confidence_transcription() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(
            MockTranscriber::with_responses(vec!["Low confidence text".to_string()])
                .with_confidence(0.25),
        ))
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

/// Test pipeline with simulated processing delay
#[tokio::test]
async fn test_transcription_with_delay() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(
            MockTranscriber::with_responses(vec!["Delayed transcription".to_string()])
                .with_delay(Duration::from_millis(500)),
        ))
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

/// Test all application modes
#[test]
fn test_all_application_modes() {
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
        .oneshot_mode(5, false)
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

/// Test builder pattern with various configurations
#[test]
fn test_builder_configurations() {
    // With notifications enabled
    let app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(true)
        .build()
        .unwrap();
    assert_eq!(app.current_state(), AppState::Idle);

    // With notifications disabled
    let app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();
    assert_eq!(app.current_state(), AppState::Idle);
}

/// Test that builder validates required components
#[test]
fn test_builder_validation() {
    // Missing audio source
    let result = HushAppBuilder::new()
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .build();
    assert!(result.is_err());

    // Missing transcriber
    let result = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .build();
    assert!(result.is_err());

    // Missing text output
    let result = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .build();
    assert!(result.is_err());

    // Missing input trigger
    let result = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .build();
    assert!(result.is_err());

    // All components provided
    let result = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .build();
    assert!(result.is_ok());
}

/// Test stats collection from the full pipeline
#[test]
fn test_pipeline_stats_collection() {
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
    assert_eq!(stats.audio_device, "Mock Audio Device");
    assert_eq!(stats.transcriber_name, "MockTranscriber");
    assert_eq!(stats.text_output_method, "Mock");
    assert!(!stats.input_trigger_desc.is_empty());
}
