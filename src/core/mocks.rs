/// Mock implementations of core traits for testing
///
/// These mocks enable unit testing without hardware dependencies
use super::traits::*;
use super::types::{BufferSize, Channels, SampleRate};
use crate::Result;
use async_trait::async_trait;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};

// ============================================================================
// Mock Audio Source
// ============================================================================

/// Mock audio source for testing (no real audio hardware required)
pub struct MockAudioSource {
    device_name: String,
    config: AudioConfig,
    is_recording: Arc<Mutex<bool>>,
    buffer: Arc<Mutex<Vec<f32>>>,
    simulate_duration: Duration,
}

impl Default for MockAudioSource {
    fn default() -> Self {
        Self::new()
    }
}

impl MockAudioSource {
    pub fn new() -> Self {
        Self::with_config(AudioConfig {
            sample_rate: SampleRate::WHISPER_OPTIMAL,
            channels: Channels::MONO,
            buffer_size: BufferSize::STANDARD,
        })
    }

    pub fn with_config(config: AudioConfig) -> Self {
        Self {
            device_name: "Mock Audio Device".to_string(),
            config,
            is_recording: Arc::new(Mutex::new(false)),
            buffer: Arc::new(Mutex::new(Vec::new())),
            simulate_duration: Duration::from_secs(3),
        }
    }

    /// Set simulated recording duration
    pub fn set_duration(&mut self, duration: Duration) {
        self.simulate_duration = duration;
    }

    /// Create mock audio source that produces empty audio (0 samples)
    pub fn with_empty_audio() -> Self {
        let mut source = Self::new();
        source.simulate_duration = Duration::from_secs(0);
        source
    }
}

impl AudioSource for MockAudioSource {
    fn start_recording(&mut self) -> Result<()> {
        let mut is_recording = self.is_recording.lock();
        if *is_recording {
            return Err(anyhow::anyhow!("Already recording"));
        }

        *is_recording = true;
        self.buffer.lock().clear();
        Ok(())
    }

    fn stop_recording(&mut self) -> Result<AudioBuffer> {
        let mut is_recording = self.is_recording.lock();
        if !*is_recording {
            return Err(anyhow::anyhow!("Not recording"));
        }

        *is_recording = false;

        // Generate mock audio data based on simulated duration
        let num_samples = (self.simulate_duration.as_secs_f32()
            * self.config.sample_rate.as_u32() as f32) as usize;
        let samples = vec![0.1f32; num_samples]; // Simulated audio

        Ok(AudioBuffer::from_config(
            samples,
            self.config.sample_rate,
            self.config.channels,
        ))
    }

    fn is_recording(&self) -> bool {
        *self.is_recording.lock()
    }

    fn device_name(&self) -> &str {
        &self.device_name
    }
}

// ============================================================================
// Mock Transcriber
// ============================================================================

/// Mock transcriber for testing (no ML model required)
pub struct MockTranscriber {
    responses: Arc<Mutex<Vec<String>>>,
    response_index: Arc<Mutex<usize>>,
    confidence: f32,
    processing_delay: Duration,
    should_fail: bool,
    error_message: String,
}

impl Default for MockTranscriber {
    fn default() -> Self {
        Self::new()
    }
}

impl MockTranscriber {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(vec!["Mock transcription result".to_string()])),
            response_index: Arc::new(Mutex::new(0)),
            confidence: 0.95,
            processing_delay: Duration::from_millis(100),
            should_fail: false,
            error_message: "Mock transcription error".to_string(),
        }
    }

    /// Set canned responses for testing
    pub fn with_responses(responses: Vec<String>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses)),
            response_index: Arc::new(Mutex::new(0)),
            confidence: 0.95,
            processing_delay: Duration::from_millis(100),
            should_fail: false,
            error_message: "Mock transcription error".to_string(),
        }
    }

    /// Set confidence score
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence;
        self
    }

    /// Set processing delay to simulate real transcription
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.processing_delay = delay;
        self
    }

    /// Make transcription fail with an error
    pub fn with_error() -> Self {
        Self {
            responses: Arc::new(Mutex::new(vec![])),
            response_index: Arc::new(Mutex::new(0)),
            confidence: 0.0,
            processing_delay: Duration::from_millis(100),
            should_fail: true,
            error_message: "Mock transcription error".to_string(),
        }
    }

    /// Make transcription fail with a custom error message
    pub fn with_custom_error(message: String) -> Self {
        Self {
            responses: Arc::new(Mutex::new(vec![])),
            response_index: Arc::new(Mutex::new(0)),
            confidence: 0.0,
            processing_delay: Duration::from_millis(100),
            should_fail: true,
            error_message: message,
        }
    }
}

#[async_trait]
impl Transcriber for MockTranscriber {
    async fn transcribe(&self, _audio: &AudioBuffer) -> Result<TranscriptionResult> {
        let start = Instant::now();

        // Simulate processing delay
        tokio::time::sleep(self.processing_delay).await;

        // Return error if configured to fail
        if self.should_fail {
            return Err(anyhow::anyhow!("{}", self.error_message));
        }

        // Get next response
        let responses = self.responses.lock();
        let mut index = self.response_index.lock();
        let text = responses[*index % responses.len()].clone();
        *index += 1;

        Ok(TranscriptionResult {
            text,
            confidence: self.confidence,
            language: Some("en".to_string()),
            processing_time: start.elapsed(),
        })
    }

    fn info(&self) -> TranscriberInfo {
        TranscriberInfo {
            name: "MockTranscriber".to_string(),
        }
    }

    async fn is_ready(&self) -> bool {
        // Mock transcriber is always ready (no model loading required)
        true
    }
}

// ============================================================================
// Mock Text Output
// ============================================================================

/// Mock text output for testing (no X11/Wayland required)
pub struct MockTextOutput {
    inserted_texts: Arc<Mutex<Vec<String>>>,
    should_fail: bool,
}

impl Default for MockTextOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl MockTextOutput {
    pub fn new() -> Self {
        Self {
            inserted_texts: Arc::new(Mutex::new(Vec::new())),
            should_fail: false,
        }
    }

    /// Make insertion fail (for error testing)
    pub fn with_failure(mut self) -> Self {
        self.should_fail = true;
        self
    }

    /// Get all texts that were inserted
    pub fn get_inserted_texts(&self) -> Vec<String> {
        self.inserted_texts.lock().clone()
    }

    /// Clear insertion history
    pub fn clear(&self) {
        self.inserted_texts.lock().clear();
    }
}

#[async_trait]
impl TextOutput for MockTextOutput {
    async fn insert_text(&mut self, text: &str) -> Result<()> {
        if self.should_fail {
            return Err(anyhow::anyhow!("Mock insertion failure"));
        }

        self.inserted_texts.lock().push(text.to_string());
        Ok(())
    }

    async fn focused_window(&self) -> Result<Option<WindowInfo>> {
        Ok(Some(WindowInfo {
            title: "Mock Window".to_string(),
            class: "MockApp".to_string(),
            app_name: "Mock Application".to_string(),
        }))
    }

    fn output_method(&self) -> &str {
        "Mock"
    }
}

// ============================================================================
// Mock Input Trigger
// ============================================================================

use tokio::sync::mpsc;

/// Mock input trigger for testing (no hotkey registration required)
pub struct MockInputTrigger {
    event_sender: mpsc::UnboundedSender<TriggerEvent>,
    event_receiver: Arc<Mutex<Option<mpsc::UnboundedReceiver<TriggerEvent>>>>,
    description: String,
}

impl Default for MockInputTrigger {
    fn default() -> Self {
        Self::new()
    }
}

impl MockInputTrigger {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            event_sender: tx,
            event_receiver: Arc::new(Mutex::new(Some(rx))),
            description: "Mock Trigger".to_string(),
        }
    }

    /// Simulate a trigger event
    pub fn trigger(&self, event: TriggerEvent) -> Result<()> {
        self.event_sender
            .send(event)
            .map_err(|e| anyhow::anyhow!("Failed to send event: {}", e))
    }

    /// Simulate press and release sequence
    pub fn simulate_press_release(&self) -> Result<()> {
        self.trigger(TriggerEvent::StartRecording)?;
        self.trigger(TriggerEvent::StopRecording)?;
        Ok(())
    }
}

#[async_trait]
impl InputTrigger for MockInputTrigger {
    async fn start_listening(&mut self) -> Result<()> {
        Ok(())
    }

    async fn stop_listening(&mut self) -> Result<()> {
        Ok(())
    }

    async fn next_event(&mut self) -> Option<TriggerEvent> {
        // Don't hold the lock across await points
        // Instead, take the receiver out of the Option temporarily
        let mut receiver_opt = {
            let mut locked = self.event_receiver.lock();
            locked.take()
        };

        match receiver_opt.as_mut() {
            Some(receiver) => {
                let event = receiver.recv().await;
                // Put the receiver back
                *self.event_receiver.lock() = receiver_opt;
                event
            },
            None => {
                // Receiver was already taken - this indicates a logic error
                // Log and return None rather than panicking
                tracing::warn!("MockInputTrigger receiver already consumed");
                None
            },
        }
    }

    fn description(&self) -> String {
        self.description.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_audio_source() {
        let mut audio = MockAudioSource::new();

        assert!(!audio.is_recording());
        assert_eq!(audio.device_name(), "Mock Audio Device");

        audio.start_recording().unwrap();
        assert!(audio.is_recording());

        let buffer = audio.stop_recording().unwrap();
        assert!(!audio.is_recording());
        assert!(!buffer.is_empty());
        assert_eq!(buffer.sample_rate, 16000);
    }

    #[tokio::test]
    async fn test_mock_transcriber() {
        let transcriber = MockTranscriber::with_responses(vec![
            "First response".to_string(),
            "Second response".to_string(),
        ]);

        let audio = AudioBuffer::new(vec![0.1f32; 16000], 16000, 1);

        let result1 = transcriber.transcribe(&audio).await.unwrap();
        assert_eq!(result1.text, "First response");

        let result2 = transcriber.transcribe(&audio).await.unwrap();
        assert_eq!(result2.text, "Second response");

        // Wraps around
        let result3 = transcriber.transcribe(&audio).await.unwrap();
        assert_eq!(result3.text, "First response");
    }

    #[tokio::test]
    async fn test_mock_text_output() {
        let mut output = MockTextOutput::new();

        output.insert_text("Hello").await.unwrap();
        output.insert_text("World").await.unwrap();

        let texts = output.get_inserted_texts();
        assert_eq!(texts, vec!["Hello", "World"]);
    }

    #[tokio::test]
    async fn test_mock_input_trigger() {
        let mut trigger = MockInputTrigger::new();

        trigger.start_listening().await.unwrap();

        // Simulate events
        trigger.trigger(TriggerEvent::StartRecording).unwrap();
        trigger.trigger(TriggerEvent::StopRecording).unwrap();

        // Receive events
        assert_eq!(
            trigger.next_event().await,
            Some(TriggerEvent::StartRecording)
        );
        assert_eq!(
            trigger.next_event().await,
            Some(TriggerEvent::StopRecording)
        );
    }
}
