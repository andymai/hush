use crate::config::settings::{HotkeyConfig, TranscriptionConfig};
use crate::core::types::{BufferSize, Channels, SampleRate};
/// Core trait abstractions for Hush components
///
/// This module defines the interfaces for all major components,
/// enabling dependency injection, testing, and extensibility.
use crate::Result;
use async_trait::async_trait;
use std::time::Duration;

// ============================================================================
// Audio Abstraction
// ============================================================================

/// Audio buffer with metadata
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration: Duration,
}

impl AudioBuffer {
    /// Create audio buffer from raw values (for backward compatibility)
    pub fn new(samples: Vec<f32>, sample_rate: u32, channels: u16) -> Self {
        let duration =
            Duration::from_secs_f32(samples.len() as f32 / (sample_rate as f32 * channels as f32));
        Self {
            samples,
            sample_rate,
            channels,
            duration,
        }
    }

    /// Create audio buffer from type-safe values
    pub fn from_config(samples: Vec<f32>, sample_rate: SampleRate, channels: Channels) -> Self {
        Self::new(samples, sample_rate.as_u32(), channels.as_u32() as u16)
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }
}

/// Audio device information
#[derive(Debug, Clone)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// Audio configuration with type-safe newtypes
#[derive(Debug, Clone, Copy)]
pub struct AudioConfig {
    pub sample_rate: SampleRate,
    pub channels: Channels,
    pub buffer_size: BufferSize,
}

/// Core trait for audio capture systems
///
/// Implementations: CpalAudioSource, SimulatedAudioSource, PulseAudioSource
pub trait AudioSource: Send + Sync {
    /// Start capturing audio from the source
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockAudioSource;
    /// use hush::core::traits::AudioSource;
    ///
    /// let mut audio = MockAudioSource::new();
    /// audio.start_recording()?;
    /// assert!(audio.is_recording());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Already recording
    /// - Audio device unavailable
    /// - Device configuration invalid
    fn start_recording(&mut self) -> Result<()>;

    /// Stop capturing and return all recorded audio samples
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockAudioSource;
    /// use hush::core::traits::AudioSource;
    ///
    /// let mut audio = MockAudioSource::new();
    /// audio.start_recording()?;
    ///
    /// // Record for some time...
    /// std::thread::sleep(std::time::Duration::from_millis(100));
    ///
    /// let buffer = audio.stop_recording()?;
    /// assert!(!buffer.is_empty());
    /// assert!(buffer.sample_rate > 0);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Not currently recording
    /// - Audio capture failed
    fn stop_recording(&mut self) -> Result<AudioBuffer>;

    /// Check if currently recording
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockAudioSource;
    /// use hush::core::traits::AudioSource;
    ///
    /// let mut audio = MockAudioSource::new();
    /// assert!(!audio.is_recording());
    ///
    /// audio.start_recording()?;
    /// assert!(audio.is_recording());
    ///
    /// audio.stop_recording()?;
    /// assert!(!audio.is_recording());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    fn is_recording(&self) -> bool;

    /// Get human-readable device name
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockAudioSource;
    /// use hush::core::traits::AudioSource;
    ///
    /// let audio = MockAudioSource::new();
    /// let name = audio.device_name();
    /// assert!(!name.is_empty());
    /// ```
    fn device_name(&self) -> &str;

    /// Get audio configuration (sample rate, channels)
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockAudioSource;
    /// use hush::core::traits::AudioSource;
    ///
    /// let audio = MockAudioSource::new();
    /// let config = audio.config();
    /// assert!(config.sample_rate.as_u32() > 0);
    /// assert!(config.channels.as_u32() > 0);
    /// ```
    fn config(&self) -> AudioConfig;
}

// ============================================================================
// Transcription Abstraction
// ============================================================================

/// Transcription result with metadata
#[derive(Debug, Clone)]
pub struct TranscriptionResult {
    pub text: String,
    pub confidence: f32,
    pub language: Option<String>,
    pub processing_time: Duration,
}

impl TranscriptionResult {
    pub fn simple(text: String, confidence: f32, duration: Duration) -> Self {
        Self {
            text,
            confidence,
            language: None,
            processing_time: duration,
        }
    }
}

/// Transcriber capabilities and info
#[derive(Debug, Clone)]
pub struct TranscriberInfo {
    pub name: String,
    pub version: String,
    pub supports_languages: Vec<String>,
    pub max_audio_duration: Option<Duration>,
    pub requires_network: bool,
    pub hardware_accelerated: bool,
}

/// Speech-to-text transcription engine
///
/// Implementations: WhisperAdapter (local Whisper models), MockTranscriber (testing)
#[async_trait]
pub trait Transcriber: Send + Sync {
    /// Transcribe audio to text asynchronously
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockTranscriber;
    /// use hush::core::traits::{Transcriber, AudioBuffer};
    ///
    /// # tokio_test::block_on(async {
    /// let transcriber = MockTranscriber::new();
    /// let audio = AudioBuffer::new(vec![0.1f32; 16000], 16000, 1);
    ///
    /// let result = transcriber.transcribe(&audio).await?;
    /// assert!(!result.text.is_empty());
    /// assert!(result.confidence > 0.0);
    /// # Ok::<(), anyhow::Error>(())
    /// # })
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Audio format is incompatible
    /// - Transcription model fails
    /// - Audio duration exceeds limits
    async fn transcribe(&self, audio: &AudioBuffer) -> Result<TranscriptionResult>;

    /// Get transcriber capabilities/info
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockTranscriber;
    /// use hush::core::traits::Transcriber;
    ///
    /// let transcriber = MockTranscriber::new();
    /// let info = transcriber.info();
    /// assert!(!info.name.is_empty());
    /// assert!(!info.version.is_empty());
    /// ```
    fn info(&self) -> TranscriberInfo;

    /// Check if transcriber is ready (model loaded, etc.)
    ///
    /// Implementations must explicitly define readiness logic rather than
    /// defaulting to always-ready, which could mask initialization issues.
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockTranscriber;
    /// use hush::core::traits::Transcriber;
    ///
    /// # tokio_test::block_on(async {
    /// let transcriber = MockTranscriber::new();
    /// assert!(transcriber.is_ready().await);
    /// # })
    /// ```
    async fn is_ready(&self) -> bool;
}

// ============================================================================
// Text Output Abstraction
// ============================================================================

/// Window information (platform-agnostic)
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub title: String,
    pub class: String,
    pub app_name: String,
}

/// Text insertion and output
///
/// Implementations: X11TextInserter, WaylandTextInserter, ClipboardOutput, StdoutOutput
#[async_trait]
pub trait TextOutput: Send + Sync {
    /// Insert text at current cursor position
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockTextOutput;
    /// use hush::core::traits::TextOutput;
    ///
    /// # tokio_test::block_on(async {
    /// let mut output = MockTextOutput::new();
    /// output.insert_text("Hello, world!").await?;
    ///
    /// let inserted = output.get_inserted_texts();
    /// assert_eq!(inserted.len(), 1);
    /// assert_eq!(inserted[0], "Hello, world!");
    /// # Ok::<(), anyhow::Error>(())
    /// # })
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Text insertion fails
    /// - No window is focused
    /// - Platform-specific insertion unavailable
    async fn insert_text(&mut self, text: &str) -> Result<()>;

    /// Get currently focused window (if available)
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockTextOutput;
    /// use hush::core::traits::TextOutput;
    ///
    /// # tokio_test::block_on(async {
    /// let output = MockTextOutput::new();
    /// let window = output.focused_window().await?;
    ///
    /// if let Some(win) = window {
    ///     assert!(!win.title.is_empty());
    /// }
    /// # Ok::<(), anyhow::Error>(())
    /// # })
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Window information unavailable
    /// - Platform doesn't support window queries
    async fn focused_window(&self) -> Result<Option<WindowInfo>>;

    /// Check if text output is available/ready
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockTextOutput;
    /// use hush::core::traits::TextOutput;
    ///
    /// let output = MockTextOutput::new();
    /// assert!(output.is_available());
    /// ```
    fn is_available(&self) -> bool;

    /// Get output method name
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockTextOutput;
    /// use hush::core::traits::TextOutput;
    ///
    /// let output = MockTextOutput::new();
    /// let method = output.output_method();
    /// assert!(!method.is_empty());
    /// ```
    fn output_method(&self) -> &str;
}

// ============================================================================
// Input Trigger Abstraction
// ============================================================================

/// Trigger events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerEvent {
    /// Start recording (hotkey pressed, wake word detected, etc.)
    StartRecording,
    /// Stop recording (hotkey released, timeout, etc.)
    StopRecording,
    /// Cancel recording
    Cancel,
}

/// User input trigger for recording
///
/// Implementations: HotkeyTrigger, CLITrigger, DBusTrigger, WakeWordTrigger
#[async_trait]
pub trait InputTrigger: Send + Sync {
    /// Start listening for trigger events
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockInputTrigger;
    /// use hush::core::traits::InputTrigger;
    ///
    /// # tokio_test::block_on(async {
    /// let mut trigger = MockInputTrigger::new();
    /// trigger.start_listening().await?;
    /// # Ok::<(), anyhow::Error>(())
    /// # })
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Already listening
    /// - Platform-specific registration fails
    /// - Hotkey conflicts with system hotkeys
    async fn start_listening(&mut self) -> Result<()>;

    /// Stop listening
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockInputTrigger;
    /// use hush::core::traits::InputTrigger;
    ///
    /// # tokio_test::block_on(async {
    /// let mut trigger = MockInputTrigger::new();
    /// trigger.start_listening().await?;
    /// trigger.stop_listening().await?;
    /// # Ok::<(), anyhow::Error>(())
    /// # })
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Not currently listening
    /// - Platform-specific deregistration fails
    async fn stop_listening(&mut self) -> Result<()>;

    /// Wait for next trigger event
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockInputTrigger;
    /// use hush::core::traits::{InputTrigger, TriggerEvent};
    ///
    /// # tokio_test::block_on(async {
    /// let mut trigger = MockInputTrigger::new();
    /// trigger.start_listening().await?;
    ///
    /// // Simulate a trigger event
    /// trigger.trigger(TriggerEvent::StartRecording)?;
    ///
    /// // Wait for the event
    /// let event = trigger.next_event().await;
    /// assert_eq!(event, Some(TriggerEvent::StartRecording));
    /// # Ok::<(), anyhow::Error>(())
    /// # })
    /// ```
    async fn next_event(&mut self) -> Option<TriggerEvent>;

    /// Get trigger description (e.g., "Ctrl+Shift+Space")
    ///
    /// # Example
    ///
    /// ```
    /// use hush::core::mocks::MockInputTrigger;
    /// use hush::core::traits::InputTrigger;
    ///
    /// let trigger = MockInputTrigger::new();
    /// let description = trigger.description();
    /// assert!(!description.is_empty());
    /// ```
    fn description(&self) -> String;
}

// ============================================================================
// Configuration Abstraction
// ============================================================================

/// Configuration access
pub trait ConfigProvider: Send + Sync {
    fn audio_config(&self) -> AudioConfig;
    fn transcription_config(&self) -> &TranscriptionConfig;
    fn hotkey_config(&self) -> &HotkeyConfig;
}
