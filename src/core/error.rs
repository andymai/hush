use std::path::PathBuf;
/// Structured error types for Hush domain
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

    #[error("Audio stream error: {0}")]
    StreamError(String),

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
}

/// Configuration errors
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to load config file '{path}': {error}")]
    LoadFailed { path: String, error: String },

    #[error("Invalid configuration: {0}")]
    ValidationFailed(String),

    #[error("Config file not found: {0}")]
    FileNotFound(PathBuf),
}

/// State machine errors
#[derive(Error, Debug)]
pub enum StateError {
    #[error("Invalid state transition: {from} → {to}")]
    InvalidTransition { from: String, to: String },

    #[error("State lock poisoned: {0}")]
    LockPoisoned(String),
}
