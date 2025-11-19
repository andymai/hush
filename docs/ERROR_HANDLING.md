# Error Handling Guide

This guide explains the error handling patterns and best practices in Hush.

## Error System Architecture

Hush uses a structured, domain-specific error system defined in `src/core/error.rs`. This provides:

- **Type safety** - Compile-time error checking with pattern matching
- **User-friendly messages** - Actionable error messages for end users
- **Severity classification** - Automatic recovery decisions based on error type
- **Domain-specific errors** - Errors grouped by component (Audio, Transcription, etc.)

## Error Types

### Top-Level Error

```rust
pub enum HushError {
    Audio(AudioError),
    Transcription(TranscriptionError),
    TextOutput(TextOutputError),
    InputTrigger(InputTriggerError),
    Config(ConfigError),
    State(StateError),
}
```

### Domain-Specific Errors

Each component has its own error type with specific variants:

#### AudioError
```rust
pub enum AudioError {
    NoDeviceAvailable,
    DeviceNotFound(String),
    RecordingStartFailed(String),
    RecordingStopFailed(String),
    StreamError(String),
    UnsupportedConfig(String),
    InvalidSampleRate { hz: u32, min: u32, max: u32 },
    InvalidChannelCount { count: u32, max: u32 },
    InvalidBufferSize { size: usize, reason: String },
}
```

#### TranscriptionError
```rust
pub enum TranscriptionError {
    ModelNotFound(PathBuf),
    ModelLoadFailed(String),
    CudaUnavailable(String),
    TranscriptionFailed(String),
    AudioTooShort { min: f32, actual: f32 },
    AudioTooLong { max: f32, actual: f32 },
    NoSpeechDetected,
}
```

#### TextOutputError
```rust
pub enum TextOutputError {
    DisplayServerUnavailable(String),
    NoFocusedWindow,
    InsertionFailed(String),
    ClipboardFailed(String),
    UnsupportedDisplayServer(String),
}
```

## Error Severity

Errors are automatically classified by severity for recovery logic:

```rust
pub enum ErrorSeverity {
    /// Recoverable, can retry automatically
    Transient,
    /// Recoverable with user action
    Recoverable,
    /// Unrecoverable, must exit
    Fatal,
}
```

### Severity Examples

**Transient** (auto-retry):
- Audio stream errors
- Transcription failures (temporary)
- Text insertion failures

**Recoverable** (user action needed):
- No audio device available
- Model not found
- CUDA unavailable
- Hotkey registration failed

**Fatal** (must exit):
- Invalid state transitions
- Corruption errors

## Usage Patterns

### Creating Errors

```rust
use crate::core::error::{HushError, AudioError, TranscriptionError};

// Simple error
return Err(AudioError::NoDeviceAvailable.into());

// Error with details
return Err(AudioError::DeviceNotFound("pulse".to_string()).into());

// Error with structured data
return Err(AudioError::InvalidSampleRate {
    hz: 48000,
    min: 8000,
    max: 48000,
}.into());
```

### Handling Errors

```rust
use crate::core::error::{HushError, ErrorSeverity};

fn handle_operation(result: Result<(), HushError>) {
    match result {
        Ok(()) => println!("Success!"),
        Err(e) => {
            // Get severity for recovery decision
            match e.severity() {
                ErrorSeverity::Transient => {
                    // Retry automatically
                    eprintln!("Transient error, retrying: {}", e.user_message());
                }
                ErrorSeverity::Recoverable => {
                    // Show user-friendly message with advice
                    eprintln!("Error: {}", e.user_message());
                }
                ErrorSeverity::Fatal => {
                    // Exit gracefully
                    eprintln!("Fatal error: {}", e.user_message());
                    std::process::exit(1);
                }
            }
        }
    }
}
```

### Propagating Errors

```rust
use crate::core::error::{HushError, AudioError};

fn capture_audio() -> Result<Vec<f32>, HushError> {
    let device = get_audio_device()
        .ok_or_else(|| AudioError::NoDeviceAvailable)?;

    let stream = device.start_recording()
        .map_err(|e| AudioError::RecordingStartFailed(e.to_string()))?;

    Ok(stream.samples())
}
```

### Converting from External Errors

```rust
use crate::core::error::{HushError, TranscriptionError};
use anyhow::Context;

fn load_model(path: &Path) -> Result<Model, HushError> {
    // Using map_err for specific error types
    std::fs::read(path)
        .map_err(|_| TranscriptionError::ModelNotFound(path.to_path_buf()))?;

    // Or using anyhow's context for more details
    Model::from_file(path)
        .context(format!("Failed to load model from {}", path.display()))
        .map_err(|e| TranscriptionError::ModelLoadFailed(e.to_string()).into())
}
```

## Best Practices

### ✅ DO

**Use specific error types:**
```rust
// Good
return Err(AudioError::DeviceNotFound(device_name).into());

// Not ideal
return Err(anyhow!("Device not found: {}", device_name));
```

**Provide actionable messages:**
```rust
// Good - tells user what to do
TranscriptionError::ModelNotFound => {
    "Model not found. Run: ./hush models download base"
}

// Bad - just states the problem
"Model file missing"
```

**Use structured error data:**
```rust
// Good - structured data
AudioError::InvalidSampleRate { hz: 48000, min: 8000, max: 48000 }

// Not ideal - string formatting
AudioError::InvalidConfig("Sample rate 48000 not in range 8000-48000".to_string())
```

**Handle errors at appropriate levels:**
```rust
// Low-level: Return domain errors
fn capture() -> Result<Vec<f32>, AudioError> { ... }

// Mid-level: Convert to HushError
fn record() -> Result<Recording, HushError> { ... }

// High-level: Show user-friendly messages
fn handle_record_command() {
    match record() {
        Ok(rec) => println!("Recorded!"),
        Err(e) => eprintln!("{}", e.user_message()),
    }
}
```

### ❌ DON'T

**Don't use unwrap() or expect():**
```rust
// Bad - will panic
let device = get_device().unwrap();

// Good - handle error
let device = get_device()
    .ok_or_else(|| AudioError::NoDeviceAvailable)?;
```

**Don't use generic error strings:**
```rust
// Bad
return Err(anyhow!("Something went wrong"));

// Good
return Err(TranscriptionError::NoSpeechDetected.into());
```

**Don't lose error context:**
```rust
// Bad - loses original error
.map_err(|_| AudioError::StreamError("Failed".to_string()))?

// Good - preserves context
.map_err(|e| AudioError::StreamError(e.to_string()))?
```

**Don't ignore transient errors:**
```rust
// Bad - gives up immediately
if let Err(e) = operation() {
    return Err(e);
}

// Good - retry transient errors
let mut result = operation();
if let Err(e) = &result {
    if e.severity() == ErrorSeverity::Transient {
        // Retry once
        result = operation();
    }
}
result?
```

## Testing Errors

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::error::{AudioError, ErrorSeverity};

    #[test]
    fn test_audio_error_severity() {
        let err = HushError::Audio(AudioError::NoDeviceAvailable);
        assert_eq!(err.severity(), ErrorSeverity::Recoverable);
    }

    #[test]
    fn test_error_message() {
        let err = HushError::Audio(AudioError::NoDeviceAvailable);
        assert!(err.user_message().contains("microphone"));
    }

    #[test]
    fn test_error_conversion() {
        let audio_err = AudioError::StreamError("test".to_string());
        let hush_err: HushError = audio_err.into();

        matches!(hush_err, HushError::Audio(_));
    }
}
```

### Integration Tests

```rust
#[test]
fn test_error_recovery() {
    let app = create_test_app();

    // Simulate transient error
    let result = app.record_with_simulated_error();

    // Should have retried automatically
    assert!(result.is_ok());
}
```

## Migration from Legacy Errors

The legacy `src/error/` module has been removed. All code should use `src/core/error::HushError`.

### Before (Legacy)
```rust
use crate::error::HushError;

HushError::AudioCapture("Device not found".to_string())
```

### After (Current)
```rust
use crate::core::error::{HushError, AudioError};

HushError::Audio(AudioError::DeviceNotFound("pulse".to_string()))
```

## Summary

1. **Use domain-specific errors** from `core::error` module
2. **Provide user-friendly messages** via `user_message()`
3. **Leverage severity classification** for automatic recovery
4. **Never use unwrap()** - always handle errors properly
5. **Test error paths** just like success paths
6. **Preserve error context** when converting between error types

For examples, see:
- `src/core/error.rs` - Error definitions
- `src/application/voice_app.rs` - Error handling in practice
- `tests/application_tests.rs` - Error testing patterns
