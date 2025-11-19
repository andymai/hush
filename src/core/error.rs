use std::path::PathBuf;
/// Structured error types for Hush domain
///
/// This module defines all error types with pattern matching support
/// and user-friendly messages.
use thiserror::Error;

/// Top-level domain errors
#[derive(Error, Debug)]
pub enum HushError {
    #[error("Audio error: {0}")]
    Audio(#[from] AudioError),

    #[error("Transcription error: {0}")]
    Transcription(#[from] TranscriptionError),

    #[error("Text output error: {0}")]
    TextOutput(#[from] TextOutputError),

    #[error("Input trigger error: {0}")]
    InputTrigger(#[from] InputTriggerError),

    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("State error: {0}")]
    State(#[from] StateError),
}

/// Audio-specific errors
#[derive(Error, Debug)]
pub enum AudioError {
    #[error("No audio device available")]
    NoDeviceAvailable,

    #[error("Device '{0}' not found")]
    DeviceNotFound(String),

    #[error("Failed to start recording: {0}")]
    RecordingStartFailed(String),

    #[error("Failed to stop recording: {0}")]
    RecordingStopFailed(String),

    #[error("Audio stream error: {0}")]
    StreamError(String),

    #[error("Unsupported audio configuration: {0}")]
    UnsupportedConfig(String),

    #[error("Invalid sample rate: {hz}Hz (must be between {min}Hz and {max}Hz)")]
    InvalidSampleRate { hz: u32, min: u32, max: u32 },

    #[error("Invalid channel count: {count} (must be between 1 and {max})")]
    InvalidChannelCount { count: u32, max: u32 },

    #[error("Invalid buffer size: {size} ({reason})")]
    InvalidBufferSize { size: usize, reason: String },
}

/// Transcription-specific errors
#[derive(Error, Debug)]
pub enum TranscriptionError {
    #[error("Model not found at path: {0}")]
    ModelNotFound(PathBuf),

    #[error("Failed to load model: {0}")]
    ModelLoadFailed(String),

    #[error("CUDA not available: {0}")]
    CudaUnavailable(String),

    #[error("Transcription failed: {0}")]
    TranscriptionFailed(String),

    #[error("Audio too short (minimum {min}s, got {actual}s)")]
    AudioTooShort { min: f32, actual: f32 },

    #[error("Audio too long (maximum {max}s, got {actual}s)")]
    AudioTooLong { max: f32, actual: f32 },

    #[error("No speech detected in audio")]
    NoSpeechDetected,
}

/// Text output errors
#[derive(Error, Debug)]
pub enum TextOutputError {
    #[error("Display server not available: {0}")]
    DisplayServerUnavailable(String),

    #[error("No focused window found")]
    NoFocusedWindow,

    #[error("Text insertion failed: {0}")]
    InsertionFailed(String),

    #[error("Clipboard operation failed: {0}")]
    ClipboardFailed(String),

    #[error("Unsupported display server: {0}")]
    UnsupportedDisplayServer(String),
}

/// Input trigger errors
#[derive(Error, Debug)]
pub enum InputTriggerError {
    #[error("Failed to register hotkey '{0}': {1}")]
    HotkeyRegistrationFailed(String, String),

    #[error("Hotkey '{0}' already registered")]
    HotkeyAlreadyRegistered(String),

    #[error("Invalid hotkey combination: {0}")]
    InvalidHotkeyCombo(String),

    #[error("Input trigger not available: {0}")]
    TriggerUnavailable(String),
}

/// Configuration errors
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to load config file '{path}': {error}")]
    LoadFailed { path: String, error: String },

    #[error("Invalid configuration: {0}")]
    ValidationFailed(String),

    #[error("Missing required config field: {0}")]
    MissingField(String),

    #[error("Config file not found: {0}")]
    FileNotFound(PathBuf),
}

/// State machine errors
#[derive(Error, Debug)]
pub enum StateError {
    #[error("Invalid state transition: {from} → {to}")]
    InvalidTransition { from: String, to: String },

    #[error("Operation not allowed in current state: {state}")]
    OperationNotAllowed { state: String },
}

/// Error severity for recovery decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    /// Recoverable, can retry
    Transient,
    /// Recoverable with user action
    Recoverable,
    /// Unrecoverable, must exit
    Fatal,
}

impl HushError {
    /// Determine error severity for recovery logic
    pub fn severity(&self) -> ErrorSeverity {
        use ErrorSeverity::*;
        match self {
            // Transient errors - can retry
            HushError::Audio(AudioError::StreamError(_)) => Transient,
            HushError::Transcription(TranscriptionError::TranscriptionFailed(_)) => Transient,
            HushError::TextOutput(TextOutputError::InsertionFailed(_)) => Transient,

            // Recoverable - user action needed
            HushError::Audio(AudioError::NoDeviceAvailable) => Recoverable,
            HushError::Audio(AudioError::DeviceNotFound(_)) => Recoverable,
            HushError::Transcription(TranscriptionError::ModelNotFound(_)) => Recoverable,
            HushError::Transcription(TranscriptionError::CudaUnavailable(_)) => Recoverable,
            HushError::TextOutput(TextOutputError::DisplayServerUnavailable(_)) => Recoverable,
            HushError::InputTrigger(InputTriggerError::HotkeyRegistrationFailed(..)) => Recoverable,
            HushError::Config(_) => Recoverable,

            // Fatal - cannot continue
            HushError::State(_) => Fatal,
            _ => Transient,
        }
    }

    /// Get user-friendly error message with actionable advice
    pub fn user_message(&self) -> String {
        match self {
            HushError::Audio(AudioError::NoDeviceAvailable) => {
                "No microphone detected. Please connect a microphone and try again.".to_string()
            },
            HushError::Audio(AudioError::DeviceNotFound(name)) => {
                format!("Microphone '{}' not found. Check your audio settings or remove device name from config.", name)
            },
            HushError::Transcription(TranscriptionError::ModelNotFound(path)) => {
                format!(
                    "Whisper model not found at '{}'.\nRun: ./scripts/download-models.sh",
                    path.display()
                )
            },
            HushError::Transcription(TranscriptionError::NoSpeechDetected) => {
                "No speech detected in recording. Try speaking louder or closer to the microphone."
                    .to_string()
            },
            _ => self.to_string(),
        }
    }
}

// Automatic conversion to anyhow is provided via the thiserror Error trait
