/// Integration tests for concurrent operations in the voice-to-text pipeline
///
/// These tests verify that the application handles concurrent access correctly
/// and maintains thread safety.
use hush::application::HushAppBuilder;
use hush::core::mocks::{MockAudioSource, MockInputTrigger, MockTextOutput, MockTranscriber};
use hush::core::state::AppState;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// Test multiple sequential recordings (not truly concurrent, but rapid succession)
#[tokio::test]
async fn test_sequential_recordings() {
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

    // Rapid sequential recordings
    for _ in 0..3 {
        app.handle_recording_start().await.unwrap();
        app.handle_recording_stop().await.unwrap();
        assert_eq!(app.current_state(), AppState::Idle);
    }
}

/// Test that stats can be queried while not recording
#[tokio::test]
async fn test_stats_query_during_idle() {
    let app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .build()
        .unwrap();

    // Query stats multiple times
    for _ in 0..10 {
        let stats = app.get_stats();
        assert_eq!(stats.state, AppState::Idle);
    }
}

/// Test stats query during recording
#[tokio::test]
async fn test_stats_query_during_recording() {
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

    // Query stats while recording
    let stats = app.get_stats();
    assert!(stats.state.is_recording());

    app.handle_recording_stop().await.unwrap();
}

/// Test rapid start/stop cycles
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

    // Very rapid cycles
    for _ in 0..20 {
        app.handle_recording_start().await.unwrap();
        app.handle_recording_stop().await.unwrap();
    }

    assert_eq!(app.current_state(), AppState::Idle);
}

/// Test multiple apps can be created and used independently
#[tokio::test]
async fn test_multiple_independent_apps() {
    let mut app1 = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::with_responses(vec![
            "App 1".to_string()
        ])))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    let mut app2 = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::with_responses(vec![
            "App 2".to_string()
        ])))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    // Use both apps
    app1.handle_recording_start().await.unwrap();
    app2.handle_recording_start().await.unwrap();

    app1.handle_recording_stop().await.unwrap();
    app2.handle_recording_stop().await.unwrap();

    assert_eq!(app1.current_state(), AppState::Idle);
    assert_eq!(app2.current_state(), AppState::Idle);
}

/// Test that mocks handle concurrent calls correctly
#[tokio::test]
async fn test_mock_transcriber_concurrent_calls() {
    let transcriber = Arc::new(MockTranscriber::with_responses(vec![
        "First".to_string(),
        "Second".to_string(),
        "Third".to_string(),
    ]));

    // Create multiple tasks that use the same transcriber
    let mut handles = vec![];

    for _ in 0..3 {
        let transcriber_clone = Arc::clone(&transcriber);
        let handle = tokio::spawn(async move {
            let audio = hush::core::traits::AudioBuffer::new(vec![0.1; 16000], 16000, 1);
            transcriber_clone.transcribe(&audio).await.unwrap()
        });
        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(!result.text.is_empty());
    }
}

/// Test rapid builder creation (stress test)
#[test]
fn test_rapid_builder_creation() {
    for _ in 0..100 {
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
}

/// Test that mock audio source handles concurrent recording checks
#[test]
fn test_mock_audio_concurrent_state_checks() {
    let audio = Arc::new(Mutex::new(MockAudioSource::new()));

    // Multiple threads checking if recording
    let mut handles = vec![];

    for _ in 0..10 {
        let audio_clone = Arc::clone(&audio);
        let handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let audio = audio_clone.lock().await;
                audio.is_recording()
            })
        });
        handles.push(handle);
    }

    for handle in handles {
        let is_recording = handle.join().unwrap();
        assert!(!is_recording);
    }
}

/// Test mock text output handles concurrent insertions
#[tokio::test]
async fn test_mock_text_output_concurrent_insertions() {
    let output = Arc::new(Mutex::new(MockTextOutput::new()));

    let mut handles = vec![];

    for i in 0..5 {
        let output_clone = Arc::clone(&output);
        let handle = tokio::spawn(async move {
            let mut output = output_clone.lock().await;
            output.insert_text(&format!("Text {}", i)).await.unwrap();
        });
        handles.push(handle);
    }

    // Wait for all insertions
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all texts were inserted
    let output = output.lock().await;
    let texts = output.get_inserted_texts();
    assert_eq!(texts.len(), 5);
}

/// Test multiple one-shot mode executions in parallel
#[tokio::test]
async fn test_parallel_oneshot_executions() {
    let mut handles = vec![];

    for i in 0..3 {
        let handle = tokio::spawn(async move {
            let mut app = HushAppBuilder::new()
                .with_audio(Box::new(MockAudioSource::new()))
                .with_transcriber(Box::new(MockTranscriber::with_responses(vec![format!(
                    "Parallel {}",
                    i
                )])))
                .with_text_output(Box::new(MockTextOutput::new()))
                .with_input_trigger(Box::new(MockInputTrigger::new()))
                .oneshot_mode(1, false)
                .with_notifications(false)
                .build()
                .unwrap();

            app.run().await.unwrap();
            assert_eq!(app.current_state(), AppState::Idle);
        });
        handles.push(handle);
    }

    // Wait for all to complete
    for handle in handles {
        handle.await.unwrap();
    }
}

/// Test that transcriber with delay works correctly in concurrent scenarios
#[tokio::test]
async fn test_concurrent_transcription_with_delays() {
    let transcriber = Arc::new(
        MockTranscriber::with_responses(vec![
            "Delayed 1".to_string(),
            "Delayed 2".to_string(),
            "Delayed 3".to_string(),
        ])
        .with_delay(Duration::from_millis(50)),
    );

    let mut handles = vec![];

    for _ in 0..3 {
        let transcriber_clone = Arc::clone(&transcriber);
        let handle = tokio::spawn(async move {
            let audio = hush::core::traits::AudioBuffer::new(vec![0.1; 16000], 16000, 1);
            let result = transcriber_clone.transcribe(&audio).await.unwrap();
            assert!(!result.text.is_empty());
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }
}

/// Test state queries during rapid transitions
#[tokio::test]
async fn test_state_queries_during_transitions() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    for _ in 0..5 {
        // Query state
        let _ = app.current_state();

        // Transition
        app.handle_recording_start().await.unwrap();

        // Query state during recording
        let state = app.current_state();
        assert!(state.is_recording());

        // Transition back
        app.handle_recording_stop().await.unwrap();

        // Query state
        assert_eq!(app.current_state(), AppState::Idle);
    }
}

/// Test that app handles interleaved operations correctly
#[tokio::test]
async fn test_interleaved_operations() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    // Interleave stats queries with operations
    app.handle_recording_start().await.unwrap();
    let _ = app.get_stats();
    let _ = app.current_state();
    app.handle_recording_stop().await.unwrap();
    let _ = app.get_stats();

    assert_eq!(app.current_state(), AppState::Idle);
}

/// Test that is_recording check is thread-safe
#[tokio::test]
async fn test_is_recording_thread_safety() {
    let mut app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .with_input_trigger(Box::new(MockInputTrigger::new()))
        .manual_mode()
        .with_notifications(false)
        .build()
        .unwrap();

    // Multiple checks
    for _ in 0..100 {
        let _ = app.is_recording();
    }

    // Start recording and check multiple times
    app.handle_recording_start().await.unwrap();
    for _ in 0..100 {
        assert!(app.is_recording());
    }

    app.handle_recording_stop().await.unwrap();
}

/// Test concurrent app creation and destruction
#[tokio::test]
async fn test_concurrent_app_lifecycle() {
    let mut handles = vec![];

    for _ in 0..10 {
        let handle = tokio::spawn(async {
            let app = HushAppBuilder::new()
                .with_audio(Box::new(MockAudioSource::new()))
                .with_transcriber(Box::new(MockTranscriber::new()))
                .with_text_output(Box::new(MockTextOutput::new()))
                .with_input_trigger(Box::new(MockInputTrigger::new()))
                .manual_mode()
                .build()
                .unwrap();

            assert_eq!(app.current_state(), AppState::Idle);
            // App is dropped here
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }
}
