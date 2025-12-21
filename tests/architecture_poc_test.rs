use hush::core::mocks::{MockAudioSource, MockTextOutput, MockTranscriber};
/// Proof of Concept Test for Trait-Based Architecture
///
/// This test demonstrates the benefits of the new trait-based architecture:
/// 1. Dependency injection with mock implementations
/// 2. Unit testing without hardware dependencies
/// 3. Easy component substitution
/// 4. Clear separation of concerns
use hush::core::traits::{AudioBuffer, AudioSource, TextOutput, Transcriber};
use hush::Result;
use std::time::Duration;

/// Simple voice-to-text pipeline using trait objects
struct SimplePipeline {
    audio: Box<dyn AudioSource>,
    transcriber: Box<dyn Transcriber>,
    output: Box<dyn TextOutput>,
}

impl SimplePipeline {
    fn new(
        audio: Box<dyn AudioSource>,
        transcriber: Box<dyn Transcriber>,
        output: Box<dyn TextOutput>,
    ) -> Self {
        Self {
            audio,
            transcriber,
            output,
        }
    }

    async fn process_recording(&mut self) -> Result<String> {
        // Start recording
        self.audio.start_recording()?;

        // Simulate recording time
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Stop and get audio
        let audio_buffer = self.audio.stop_recording()?;

        // Transcribe
        let result = self.transcriber.transcribe(&audio_buffer).await?;

        // Insert text
        self.output.insert_text(&result.text).await?;

        Ok(result.text)
    }
}

#[tokio::test]
async fn test_pipeline_with_mock_components() {
    // Create mock components - NO HARDWARE REQUIRED! ✅
    let audio = Box::new(MockAudioSource::new());
    let transcriber = Box::new(MockTranscriber::with_responses(vec![
        "Hello world".to_string()
    ]));
    let output = Box::new(MockTextOutput::new());

    // Create pipeline with mocks
    let mut pipeline = SimplePipeline::new(audio, transcriber, output);

    // Process recording
    let result = pipeline.process_recording().await.unwrap();

    assert_eq!(result, "Hello world");
}

#[tokio::test]
async fn test_pipeline_with_different_transcriber() {
    // DEMONSTRATES EXTENSIBILITY: Easy to swap implementations
    let audio = Box::new(MockAudioSource::new());

    // Different transcriber with different behavior
    let transcriber = Box::new(
        MockTranscriber::with_responses(vec![
            "First".to_string(),
            "Second".to_string(),
            "Third".to_string(),
        ])
        .with_confidence(0.99)
        .with_delay(Duration::from_millis(50)),
    );

    let output = Box::new(MockTextOutput::new());

    let mut pipeline = SimplePipeline::new(audio, transcriber, output);

    // Process multiple recordings
    let result1 = pipeline.process_recording().await.unwrap();
    let result2 = pipeline.process_recording().await.unwrap();
    let result3 = pipeline.process_recording().await.unwrap();

    assert_eq!(result1, "First");
    assert_eq!(result2, "Second");
    assert_eq!(result3, "Third");
}

#[tokio::test]
async fn test_error_handling_in_pipeline() {
    // DEMONSTRATES ERROR HANDLING: Easy to test error scenarios
    let audio = Box::new(MockAudioSource::new());
    let transcriber = Box::new(MockTranscriber::new());
    let output = Box::new(MockTextOutput::new().with_failure()); // Will fail!

    let mut pipeline = SimplePipeline::new(audio, transcriber, output);

    // Should fail at text insertion
    let result = pipeline.process_recording().await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("insertion failure"));
}

#[test]
fn test_audio_buffer_creation() {
    let samples = vec![0.1f32; 16000]; // 1 second at 16kHz
    let buffer = AudioBuffer::new(samples.clone(), 16000, 1);

    assert_eq!(buffer.len(), 16000);
    assert_eq!(buffer.sample_rate, 16000);
    assert_eq!(buffer.channels, 1);
    assert!(buffer.duration.as_secs_f32() >= 0.99 && buffer.duration.as_secs_f32() <= 1.01);
}

#[tokio::test]
async fn test_transcriber_info() {
    let transcriber = MockTranscriber::new();
    let info = transcriber.info();

    assert_eq!(info.name, "MockTranscriber");
    assert!(info.supports_languages.contains(&"en".to_string()));
    assert!(!info.requires_network);
}

#[tokio::test]
async fn test_text_output_tracking() {
    let mut output = MockTextOutput::new();

    // Insert multiple texts
    output.insert_text("First line").await.unwrap();
    output.insert_text("Second line").await.unwrap();
    output.insert_text("Third line").await.unwrap();

    // Verify all were inserted
    let inserted = output.get_inserted_texts();
    assert_eq!(inserted.len(), 3);
    assert_eq!(inserted[0], "First line");
    assert_eq!(inserted[1], "Second line");
    assert_eq!(inserted[2], "Third line");

    // Clear and verify
    output.clear();
    assert_eq!(output.get_inserted_texts().len(), 0);
}

#[test]
fn test_audio_source_state_management() {
    let mut audio = MockAudioSource::new();

    // Initial state
    assert!(!audio.is_recording());

    // Start recording
    audio.start_recording().unwrap();
    assert!(audio.is_recording());

    // Cannot start twice
    assert!(audio.start_recording().is_err());

    // Stop recording
    let buffer = audio.stop_recording().unwrap();
    assert!(!audio.is_recording());
    assert!(!buffer.is_empty());

    // Cannot stop when not recording
    assert!(audio.stop_recording().is_err());
}

#[tokio::test]
async fn test_transcription_result_metadata() {
    let transcriber = MockTranscriber::new().with_confidence(0.85);

    let audio = AudioBuffer::new(vec![0.1f32; 16000], 16000, 1);
    let result = transcriber.transcribe(&audio).await.unwrap();

    assert!(!result.text.is_empty());
    assert_eq!(result.confidence, 0.85);
    assert!(result.language.is_some());
    assert!(result.processing_time > Duration::from_millis(0));
}

/// DEMONSTRATES: Components can be tested in complete isolation
#[test]
fn test_component_isolation() {
    // Each component can be instantiated and tested independently
    let _audio = MockAudioSource::new();
    let _transcriber = MockTranscriber::new();
    let _output = MockTextOutput::new();

    // No dependencies between them
    // No global state
    // No hardware requirements
    // ✅ Perfect for unit testing!
}

/// DEMONSTRATES: Easy to create different configurations
#[test]
fn test_configuration_flexibility() {
    // Fast transcriber for testing
    let _fast = MockTranscriber::new().with_delay(Duration::from_millis(10));

    // Slow transcriber for performance testing
    let _slow = MockTranscriber::new().with_delay(Duration::from_secs(2));

    // Low confidence for error scenario testing
    let _unreliable = MockTranscriber::new().with_confidence(0.3);

    // All implement the same trait! ✅
}

/// DEMONSTRATES: Trait objects enable runtime polymorphism
#[tokio::test]
async fn test_runtime_polymorphism() {
    let transcribers: Vec<Box<dyn Transcriber>> = vec![
        Box::new(MockTranscriber::with_responses(vec!["A".to_string()])),
        Box::new(MockTranscriber::with_responses(vec!["B".to_string()])),
        Box::new(MockTranscriber::with_responses(vec!["C".to_string()])),
    ];

    let audio = AudioBuffer::new(vec![0.1f32; 8000], 16000, 1);

    let mut results = Vec::new();
    for transcriber in transcribers {
        let result = transcriber.transcribe(&audio).await.unwrap();
        results.push(result.text);
    }

    assert_eq!(results, vec!["A", "B", "C"]);
}

// SUMMARY: Benefits Demonstrated
//
// ✅ **Testability**: All tests run without hardware (microphone, X11, GPU)
// ✅ **Mockability**: Easy to create test doubles for all components
// ✅ **Isolation**: Components can be tested independently
// ✅ **Flexibility**: Easy to swap implementations at runtime
// ✅ **Error Testing**: Easy to simulate error conditions
// ✅ **Extensibility**: New implementations just need to implement traits
// ✅ **Maintainability**: Clear contracts between components
//
// This architecture makes it possible to:
// - Run tests in CI/CD without audio hardware
// - Test error scenarios that are hard to reproduce
// - Develop new features without hardware dependencies
// - Swap implementations (e.g., X11 → Wayland) without breaking code
// - Add new transcription backends easily
