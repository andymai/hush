/// Core trait abstractions for Hush components
///
/// This module defines the interfaces for all major components,
/// enabling dependency injection, testing, and extensibility.

use crate::Result;
use crate::core::types::{SampleRate, Channels, BufferSize};
use crate::config::settings::{TranscriptionConfig, HotkeyConfig};
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
        let duration = Duration::from_secs_f32(
            samples.len() as f32 / (sample_rate as f32 * channels as f32),
        );
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
    fn start_recording(&mut self) -> Result<()>;

    /// Stop capturing and return all recorded audio samples
    fn stop_recording(&mut self) -> Result<AudioBuffer>;

    /// Check if currently recording
    fn is_recording(&self) -> bool;

    /// Get human-readable device name
    fn device_name(&self) -> &str;

    /// Get audio configuration (sample rate, channels)
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
    async fn transcribe(&self, audio: &AudioBuffer) -> Result<TranscriptionResult>;

    /// Get transcriber capabilities/info
    fn info(&self) -> TranscriberInfo;

    /// Check if transcriber is ready (model loaded, etc.)
    ///
    /// Implementations must explicitly define readiness logic rather than
    /// defaulting to always-ready, which could mask initialization issues.
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
    async fn insert_text(&mut self, text: &str) -> Result<()>;

    /// Get currently focused window (if available)
    async fn focused_window(&self) -> Result<Option<WindowInfo>>;

    /// Check if text output is available/ready
    fn is_available(&self) -> bool;

    /// Get output method name
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
    async fn start_listening(&mut self) -> Result<()>;

    /// Stop listening
    async fn stop_listening(&mut self) -> Result<()>;

    /// Wait for next trigger event
    async fn next_event(&mut self) -> Option<TriggerEvent>;

    /// Get trigger description (e.g., "Ctrl+Shift+Space")
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
