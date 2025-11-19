# ADR-002: Structured Error Handling with Custom Error Types

---
**Last Updated**: 2025-11-19
**Original Date**: 2025-10-06
**Status**: Partially Implemented (Oct 2025)
**Deciders**: Architecture Review
**Purpose**: Documents decision to use structured error handling with thiserror
**Related Documents**: [ADR-001](ADR-001-trait-based-architecture.md) | [Architecture](../../ARCHITECTURE.md) | [Documentation Index](../../../DOCUMENTATION_INDEX.md)
---

**Implementation Status**: Core error types implemented in October 2025. Additional error handling added for desktop integration and text processing features in November 2025.

## Context

The current error handling in Hush is inconsistent:

1. **Mixed Strategies**: Uses `anyhow::Result<T>` everywhere, but also defines custom `HushError` enum that's never used
2. **No Error Recovery**: Cannot pattern match on error types, making recovery logic impossible
3. **Generic Error Messages**: User-facing errors are generic and unhelpful
4. **No Error Context**: Errors lack structured context for debugging

### Current State

```rust
// lib.rs - Everything uses anyhow
pub type Result<T> = anyhow::Result<T>;

// error/handler.rs - Custom errors defined but UNUSED
pub enum HushError {
    AudioCapture(String),
    Transcription(String),
    TextInsertion(String),
    Config(String),
    Hotkey(String),
}

pub struct ErrorHandler {
    // Will be implemented with proper error handling
}
// Stubbed with todo!()
```

### Problems

1. Cannot implement error-specific recovery logic
2. All errors treated identically (some are recoverable, some aren't)
3. No user-friendly error messages
4. Cannot distinguish between transient vs permanent failures
5. No structured error reporting/telemetry

## Decision

We will adopt **structured error handling using `thiserror` for domain errors** while **keeping `anyhow` for application-level errors**.

### Strategy: Layered Error Handling

```
┌─────────────────────────────────────┐
│   Application Layer (anyhow)       │  ← Main, CLI, orchestration
│   - Flexible error propagation     │
│   - Rich context chains             │
└──────────────┬──────────────────────┘
               │
┌──────────────▼──────────────────────┐
│   Domain Layer (thiserror)          │  ← Core, traits, components
│   - Structured error types          │
│   - Pattern matchable               │
│   - Convertible to anyhow           │
└──────────────┬──────────────────────┘
               │
┌──────────────▼──────────────────────┐
│   External Errors (library errors)  │  ← cpal, x11rb, etc.
│   - Wrapped in domain errors        │
└─────────────────────────────────────┘
```

### Domain Error Types

```rust
// src/core/error.rs

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
}

/// Transcription-specific errors
#[derive(Error, Debug)]
pub enum TranscriptionError {
    #[error("Model not found at path: {0}")]
    ModelNotFound(std::path::PathBuf),

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
    FileNotFound(std::path::PathBuf),
}

/// State machine errors
#[derive(Error, Debug)]
pub enum StateError {
    #[error("Invalid state transition: {from:?} -> {to:?}")]
    InvalidTransition { from: String, to: String },

    #[error("Operation not allowed in current state: {state:?}")]
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
            }
            HushError::Audio(AudioError::DeviceNotFound(name)) => {
                format!("Microphone '{}' not found. Check your audio settings or remove device name from config.", name)
            }
            HushError::Transcription(TranscriptionError::ModelNotFound(path)) => {
                format!("Whisper model not found at '{}'.\n\
                        Run: ./scripts/download-models.sh", path.display())
            }
            HushError::Transcription(TranscriptionError::CudaUnavailable(msg)) => {
                format!("CUDA not available ({}). Using CPU mode (slower).", msg)
            }
            HushError::Transcription(TranscriptionError::NoSpeechDetected) => {
                "No speech detected in recording. Try speaking louder or closer to the microphone.".to_string()
            }
            HushError::TextOutput(TextOutputError::DisplayServerUnavailable(msg)) => {
                format!("Display server not available: {}.\n\
                        Make sure X11 or Wayland is running and DISPLAY is set.", msg)
            }
            HushError::InputTrigger(InputTriggerError::HotkeyRegistrationFailed(combo, reason)) => {
                format!("Failed to register hotkey '{}': {}.\n\
                        Try using 'hush manual' mode instead.", combo, reason)
            }
            HushError::Config(ConfigError::FileNotFound(path)) => {
                format!("Config file not found: '{}'. Creating default config...", path.display())
            }
            _ => self.to_string(),
        }
    }
}

// Automatic conversion to anyhow for application layer
impl From<HushError> for anyhow::Error {
    fn from(err: HushError) -> Self {
        anyhow::anyhow!(err)
    }
}
```

### Error Handler Implementation

```rust
// src/core/error_handler.rs

use super::error::{HushError, ErrorSeverity};
use crate::core::traits::FeedbackProvider;
use std::sync::Arc;
use tracing::{error, warn, info};

pub struct ErrorHandler {
    feedback: Arc<dyn FeedbackProvider>,
    retry_config: RetryConfig,
}

impl ErrorHandler {
    pub fn new(feedback: Arc<dyn FeedbackProvider>) -> Self {
        Self {
            feedback,
            retry_config: RetryConfig::default(),
        }
    }

    /// Handle error with appropriate recovery strategy
    pub async fn handle(&self, error: HushError) -> ErrorHandlingResult {
        // Log error with full context
        error!("Error occurred: {:?}", error);

        // Determine severity
        let severity = error.severity();

        // Provide user feedback
        let user_msg = error.user_message();
        if let Err(e) = self.feedback.on_error(&user_msg).await {
            warn!("Failed to provide error feedback: {:?}", e);
        }

        // Determine recovery strategy
        match severity {
            ErrorSeverity::Transient => {
                info!("Transient error, will retry");
                ErrorHandlingResult::Retry
            }
            ErrorSeverity::Recoverable => {
                warn!("Recoverable error, user action required");
                ErrorHandlingResult::Continue
            }
            ErrorSeverity::Fatal => {
                error!("Fatal error, must exit");
                ErrorHandlingResult::Exit(1)
            }
        }
    }

    /// Retry operation with exponential backoff
    pub async fn retry<F, T>(&self, operation: F) -> Result<T, HushError>
    where
        F: Fn() -> Result<T, HushError>,
    {
        let mut attempts = 0;
        let mut delay = self.retry_config.initial_delay;

        loop {
            match operation() {
                Ok(result) => return Ok(result),
                Err(error) => {
                    attempts += 1;

                    if attempts >= self.retry_config.max_attempts {
                        error!("Max retry attempts reached ({})", attempts);
                        return Err(error);
                    }

                    if error.severity() != ErrorSeverity::Transient {
                        // Don't retry non-transient errors
                        return Err(error);
                    }

                    warn!("Attempt {} failed, retrying in {:?}", attempts, delay);
                    tokio::time::sleep(delay).await;

                    // Exponential backoff
                    delay = std::cmp::min(
                        delay * 2,
                        self.retry_config.max_delay,
                    );
                }
            }
        }
    }
}

pub enum ErrorHandlingResult {
    Retry,
    Continue,
    Exit(i32),
}

pub struct RetryConfig {
    pub max_attempts: usize,
    pub initial_delay: std::time::Duration,
    pub max_delay: std::time::Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: std::time::Duration::from_millis(100),
            max_delay: std::time::Duration::from_secs(5),
        }
    }
}
```

## Consequences

### Positive

1. **Pattern Matchable**: Can implement specific error handling logic
   ```rust
   match result {
       Err(HushError::Transcription(TranscriptionError::NoSpeechDetected)) => {
           // Silent recording, not an error
       }
       Err(HushError::Audio(AudioError::NoDeviceAvailable)) => {
           // Prompt user to connect microphone
       }
       Err(e) => return Err(e),
   }
   ```

2. **User-Friendly Messages**: Each error has actionable advice
3. **Recovery Logic**: Can retry transient errors automatically
4. **Error Telemetry**: Structured errors can be reported to monitoring
5. **Type Safety**: Compiler ensures all errors are handled

### Negative

1. **More Verbose**: More error types to maintain
2. **Conversion Overhead**: Need to convert library errors to domain errors

## Alternatives Considered

### Alternative 1: Keep anyhow Everywhere
**Rejected**: Cannot pattern match, no recovery logic possible

### Alternative 2: Use eyre Instead of anyhow
**Rejected**: Doesn't solve structured error problem

## Success Metrics

1. All trait methods return `Result<T, HushError>`
2. User-facing errors are actionable (tested with users)
3. Transient errors retry automatically
4. Error recovery demonstrated in tests

## References

- [thiserror documentation](https://docs.rs/thiserror)
- [anyhow documentation](https://docs.rs/anyhow)
- [Rust Error Handling](https://doc.rust-lang.org/book/ch09-00-error-handling.html)

## Changelog

- 2025-10-06: Initial proposal
