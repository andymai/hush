# Testing Strategies for Hush

**Last Updated:** 2025-11-19
**Status:** Reference for AI agents working on tests

This document outlines testing patterns, strategies, and best practices for the Hush voice-to-text application.

---

## Core Testing Philosophy

**Mock-First Testing:** All tests should run without hardware dependencies (no microphone, GPU, X11, or network required).

**Why?**
- ✅ Tests run in CI without special hardware
- ✅ Tests are fast and reliable
- ✅ Tests can run in parallel
- ✅ Developers can test without specific hardware setup
- ✅ Tests are deterministic (no flaky tests from hardware timing)

---

## Test Organization

### Directory Structure

```
src/
├── core/
│   ├── traits.rs         # Trait definitions
│   ├── mocks.rs          # ⭐ Mock implementations for all traits
│   └── */                # Other core modules with #[cfg(test)] at bottom
│
├── adapters/
│   └── */                # Adapter modules with unit tests
│
├── application/
│   └── */                # Application modules with unit tests
│
tests/
├── integration_full_pipeline.rs      # Full pipeline integration tests
├── integration_error_scenarios.rs    # Error handling integration tests
├── integration_state_machine.rs      # State machine integration tests
└── integration_concurrent.rs         # Concurrency tests
```

### Test Types

1. **Unit Tests** - In each module's `#[cfg(test)]` block
2. **Integration Tests** - In `tests/` directory
3. **Binary Tests** - In `src/bin/test-*.rs` for manual testing
4. **Doc Tests** - In documentation comments (verified examples)

---

## Mock Infrastructure

### Core Mock Trait Pattern

**Location:** `src/core/mocks.rs`

All traits defined in `src/core/traits.rs` must have corresponding mock implementations:

```rust
// src/core/traits.rs
pub trait AudioSource: Send + Sync {
    fn start_recording(&mut self) -> Result<()>;
    fn stop_recording(&mut self) -> Result<AudioBuffer>;
    fn is_recording(&self) -> bool;
}

// src/core/mocks.rs
pub struct MockAudioSource {
    pub is_recording: bool,
    pub samples: Vec<f32>,
    pub should_error: Option<HushError>,
}

impl MockAudioSource {
    pub fn new() -> Self {
        Self {
            is_recording: false,
            samples: vec![0.0; 16000],  // 1 second at 16kHz
            should_error: None,
        }
    }

    pub fn with_samples(samples: Vec<f32>) -> Self {
        Self {
            samples,
            ..Self::new()
        }
    }

    pub fn with_error(error: HushError) -> Self {
        Self {
            should_error: Some(error),
            ..Self::new()
        }
    }
}

impl AudioSource for MockAudioSource {
    fn start_recording(&mut self) -> Result<()> {
        if let Some(err) = &self.should_error {
            return Err(err.clone());
        }
        self.is_recording = true;
        Ok(())
    }

    fn stop_recording(&mut self) -> Result<AudioBuffer> {
        self.is_recording = false;
        Ok(AudioBuffer::new(self.samples.clone(), 16000))
    }

    fn is_recording(&self) -> bool {
        self.is_recording
    }
}
```

### Mock Builder Patterns

**Pattern 1: Default constructor**
```rust
let mock = MockAudioSource::new();  // Sensible defaults
```

**Pattern 2: With specific data**
```rust
let mock = MockAudioSource::with_samples(vec![0.1, 0.2, 0.3]);
```

**Pattern 3: With error behavior**
```rust
let mock = MockAudioSource::with_error(AudioError::DeviceNotFound("test".into()).into());
```

**Pattern 4: With responses (for stateful mocks)**
```rust
let mock = MockTranscriber::with_responses(vec![
    "First transcription".to_string(),
    "Second transcription".to_string(),
]);
```

### Current Mock Implementations

**Verify before using:**
```bash
# List all mocks
rg "pub struct Mock" src/core/mocks.rs

# Check specific mock
rg "MockAudioSource" src/core/mocks.rs --context=5
```

**Available mocks (as of 2025-11-19):**
- `MockAudioSource` - Audio capture
- `MockTranscriber` - Transcription
- `MockTextOutput` - Text insertion
- `MockInputTrigger` - Hotkey/trigger
- `MockStateObserver` - State change observation

---

## Unit Testing Patterns

### Location and Structure

**Unit tests live at the bottom of each module:**

```rust
// src/some_module.rs

pub struct Component {
    // Implementation
}

impl Component {
    pub fn new() -> Self { /* ... */ }
    pub fn process(&self) -> Result<()> { /* ... */ }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_creation() {
        let component = Component::new();
        assert!(component.is_valid());
    }

    #[test]
    fn test_process_success() {
        let component = Component::new();
        let result = component.process();
        assert!(result.is_ok());
    }

    #[test]
    fn test_process_error() {
        let component = Component::with_error_condition();
        let result = component.process();
        assert!(result.is_err());
    }
}
```

### Async Unit Tests

**Use `#[tokio::test]` for async functions:**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_async_operation() {
        let component = Component::new();
        let result = component.async_process().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_async_error_handling() {
        let component = Component::with_error();
        let result = component.async_process().await;

        match result {
            Err(HushError::Audio(AudioError::DeviceNotFound(_))) => {
                // Expected error
            }
            _ => panic!("Expected DeviceNotFound error"),
        }
    }
}
```

### Testing Error Paths

**Always test both success and failure scenarios:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::error::{HushError, AudioError};

    #[test]
    fn test_success_path() {
        let result = function_that_can_fail(valid_input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_error_path_invalid_input() {
        let result = function_that_can_fail(invalid_input);
        assert!(matches!(
            result,
            Err(HushError::Audio(AudioError::InvalidSampleRate { .. }))
        ));
    }

    #[test]
    fn test_error_path_device_not_found() {
        let result = function_with_device("nonexistent");
        assert!(matches!(
            result,
            Err(HushError::Audio(AudioError::DeviceNotFound(_)))
        ));
    }
}
```

---

## Integration Testing Patterns

### Full Pipeline Test

**Pattern:** Test complete user workflow end-to-end

```rust
// tests/integration_full_pipeline.rs

use hush::application::HushAppBuilder;
use hush::core::mocks::*;

#[tokio::test]
async fn test_complete_voice_to_text_pipeline() {
    // Arrange: Setup mocks with predictable behavior
    let audio = Box::new(MockAudioSource::with_samples(
        vec![0.1; 48000]  // 3 seconds at 16kHz
    ));

    let transcriber = Box::new(MockTranscriber::with_responses(vec![
        "Hello world".to_string()
    ]));

    let output = Box::new(MockTextOutput::new());

    // Act: Build app and run pipeline
    let app = HushAppBuilder::new()
        .with_audio(audio)
        .with_transcriber(transcriber)
        .with_text_output(output)
        .oneshot_mode(3, false)
        .build()
        .expect("Failed to build app");

    let result = app.run_once().await;

    // Assert: Verify expected behavior
    assert!(result.is_ok());
    let transcription = result.unwrap();
    assert_eq!(transcription.text, "Hello world");
}
```

### Error Scenario Testing

**Pattern:** Test system behavior under error conditions

```rust
// tests/integration_error_scenarios.rs

#[tokio::test]
async fn test_audio_device_unavailable_error() {
    // Setup mock that returns error
    let audio = Box::new(MockAudioSource::with_error(
        AudioError::NoDeviceAvailable.into()
    ));

    let transcriber = Box::new(MockTranscriber::new());
    let output = Box::new(MockTextOutput::new());

    let app = HushAppBuilder::new()
        .with_audio(audio)
        .with_transcriber(transcriber)
        .with_text_output(output)
        .build()
        .expect("App builder should succeed");

    // Attempt to run pipeline
    let result = app.run_once().await;

    // Verify appropriate error handling
    assert!(result.is_err());
    match result {
        Err(HushError::Audio(AudioError::NoDeviceAvailable)) => {
            // Expected error
        }
        _ => panic!("Expected NoDeviceAvailable error"),
    }
}

#[tokio::test]
async fn test_transcription_failure_with_retry() {
    let audio = Box::new(MockAudioSource::new());

    // First call fails, second succeeds
    let transcriber = Box::new(MockTranscriber::with_error_then_success(
        TranscriptionError::NoSpeechDetected.into(),
        "Retry succeeded".to_string()
    ));

    let output = Box::new(MockTextOutput::new());

    let app = HushAppBuilder::new()
        .with_audio(audio)
        .with_transcriber(transcriber)
        .with_text_output(output)
        .with_retry(1)  // Enable one retry
        .build()
        .unwrap();

    let result = app.run_once().await;

    // Should succeed on retry
    assert!(result.is_ok());
    assert_eq!(result.unwrap().text, "Retry succeeded");
}
```

### State Machine Testing

**Pattern:** Test valid and invalid state transitions

```rust
// tests/integration_state_machine.rs

use hush::core::state::{StateMachine, AppState};
use std::time::Instant;

#[test]
fn test_valid_state_transitions() {
    let mut sm = StateMachine::new();

    // Idle -> Recording
    assert!(sm.transition(AppState::Recording {
        started_at: Instant::now()
    }).is_ok());

    // Recording -> Transcribing
    assert!(sm.transition(AppState::Transcribing {
        audio_duration: Duration::from_secs(3)
    }).is_ok());

    // Transcribing -> Inserting
    assert!(sm.transition(AppState::Inserting {
        text_length: 100
    }).is_ok());

    // Inserting -> Idle
    assert!(sm.transition(AppState::Idle).is_ok());
}

#[test]
fn test_invalid_state_transitions() {
    let mut sm = StateMachine::new();

    // Cannot go from Idle directly to Transcribing
    let result = sm.transition(AppState::Transcribing {
        audio_duration: Duration::from_secs(3)
    });

    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(StateError::InvalidTransition { from: AppState::Idle, to: AppState::Transcribing { .. } })
    ));
}
```

### Concurrent Operation Testing

**Pattern:** Test thread safety and concurrent access

```rust
// tests/integration_concurrent.rs

use std::sync::Arc;
use tokio::task;

#[tokio::test]
async fn test_concurrent_state_observations() {
    let sm = Arc::new(StateMachine::new());
    let observer1 = Arc::new(MockStateObserver::new());
    let observer2 = Arc::new(MockStateObserver::new());

    sm.add_observer(observer1.clone());
    sm.add_observer(observer2.clone());

    // Spawn multiple tasks making state transitions
    let sm1 = sm.clone();
    let handle1 = task::spawn(async move {
        sm1.transition(AppState::Recording {
            started_at: Instant::now()
        }).unwrap();
    });

    let sm2 = sm.clone();
    let handle2 = task::spawn(async move {
        // Wait a bit then transition
        tokio::time::sleep(Duration::from_millis(10)).await;
        sm2.transition(AppState::Idle).unwrap();
    });

    // Wait for both tasks
    handle1.await.unwrap();
    handle2.await.unwrap();

    // Verify both observers received notifications
    assert!(observer1.state_changes().len() > 0);
    assert!(observer2.state_changes().len() > 0);
}
```

---

## Builder Pattern for Tests

**Use HushAppBuilder for clean test setup:**

```rust
use hush::application::HushAppBuilder;

#[tokio::test]
async fn test_with_builder() {
    let app = HushAppBuilder::new()
        .with_audio(Box::new(MockAudioSource::new()))
        .with_transcriber(Box::new(MockTranscriber::new()))
        .with_text_output(Box::new(MockTextOutput::new()))
        .oneshot_mode(3, false)  // 3 seconds, no save
        .build()
        .expect("Builder should succeed");

    // Test app
}
```

**Builder advantages:**
- Clear, readable test setup
- Compile-time checks for required components
- Flexible configuration
- Easy to modify for different test scenarios

---

## Test Data and Fixtures

### Audio Test Data

```rust
// Helper function for generating test audio
fn create_test_audio(duration_secs: f32, sample_rate: u32) -> Vec<f32> {
    let num_samples = (duration_secs * sample_rate as f32) as usize;

    // Generate sine wave or silence
    (0..num_samples)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5
        })
        .collect()
}

#[test]
fn test_with_audio_fixture() {
    let samples = create_test_audio(3.0, 16000);  // 3 seconds
    let mock = MockAudioSource::with_samples(samples);
    // Use mock in test
}
```

### Transcription Test Data

```rust
// Common test transcriptions
const TEST_TRANSCRIPTIONS: &[&str] = &[
    "Hello world",
    "This is a test",
    "The quick brown fox jumps over the lazy dog",
];

#[tokio::test]
async fn test_multiple_transcriptions() {
    let transcriber = MockTranscriber::with_responses(
        TEST_TRANSCRIPTIONS.iter().map(|s| s.to_string()).collect()
    );

    // Test with multiple transcriptions
}
```

---

## Coverage Measurement

### Using Tarpaulin

```bash
# Install
cargo install cargo-tarpaulin

# Run coverage
cargo tarpaulin --out Html --output-dir coverage/

# View report
firefox coverage/index.html
```

### Coverage Goals

**Target coverage by module:**
- `src/core/` - 80%+ (critical infrastructure)
- `src/adapters/` - 70%+ (platform-specific, some hardware)
- `src/application/` - 75%+ (main logic)
- Overall - 70%+

**Check current coverage:**
```bash
cargo tarpaulin --out Stdout
```

---

## Property-Based Testing

### Using Proptest

**Add to Cargo.toml:**
```toml
[dev-dependencies]
proptest = "1.0"
```

**Example property test:**
```rust
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_audio_buffer_size_matches_duration(
            duration in 1.0f32..10.0,
            sample_rate in 8000u32..48000
        ) {
            let expected_samples = (duration * sample_rate as f32) as usize;
            let buffer = AudioBuffer::new(vec![0.0; expected_samples], sample_rate);

            prop_assert_eq!(buffer.duration().as_secs_f32(), duration, 0.01);
            prop_assert_eq!(buffer.samples().len(), expected_samples);
        }

        #[test]
        fn test_state_transitions_always_valid(
            states in prop::collection::vec(arb_app_state(), 1..10)
        ) {
            let mut sm = StateMachine::new();

            for state in states {
                let result = sm.transition(state);
                // All transitions should either succeed or return InvalidTransition
                prop_assert!(
                    result.is_ok() || matches!(result, Err(StateError::InvalidTransition { .. }))
                );
            }
        }
    }
}
```

---

## Testing Best Practices

### ✅ DO

**Test both success and error paths:**
```rust
#[test]
fn test_success() { /* ... */ }

#[test]
fn test_error_device_not_found() { /* ... */ }

#[test]
fn test_error_invalid_config() { /* ... */ }
```

**Use descriptive test names:**
```rust
#[test]
fn test_audio_source_starts_recording_successfully() { /* ... */ }

// Better than:
#[test]
fn test1() { /* ... */ }
```

**Use mocks instead of real hardware:**
```rust
// Good
let audio = Box::new(MockAudioSource::new());

// Bad (in tests)
let audio = Box::new(CpalAudioSource::new()?);  // Requires microphone
```

**Test one thing per test:**
```rust
// Good - focused test
#[test]
fn test_start_recording_sets_flag() {
    let mut audio = MockAudioSource::new();
    audio.start_recording().unwrap();
    assert!(audio.is_recording());
}

// Bad - testing multiple things
#[test]
fn test_everything() {
    // Tests recording, transcription, output all together
}
```

**Use assertions that provide good error messages:**
```rust
// Good
assert_eq!(result.text, "expected");

// Better
assert_eq!(
    result.text,
    "expected",
    "Transcription text mismatch: got '{}', expected '{}'",
    result.text,
    "expected"
);
```

### ❌ DON'T

**Don't use `unwrap()` without reason in tests:**
```rust
// Acceptable in tests for setup
let app = build_app().unwrap();  // OK - setup

// Not great - hides useful error info
let result = app.run().unwrap();  // Use proper assertions instead
assert_eq!(result.text, "expected");
```

**Don't test implementation details:**
```rust
// Bad - testing internal field
assert_eq!(component.internal_counter, 5);

// Good - testing public behavior
assert_eq!(component.count(), 5);
```

**Don't write flaky tests:**
```rust
// Bad - timing-dependent
tokio::time::sleep(Duration::from_millis(100)).await;
assert!(flag_was_set);

// Good - deterministic with mocks
let result = mock.process().await;
assert!(result.is_ok());
```

**Don't skip error scenario tests:**
```rust
// Incomplete
#[test]
fn test_process() {
    let result = component.process();
    assert!(result.is_ok());
}

// Complete
#[test]
fn test_process_success() { /* ... */ }

#[test]
fn test_process_error_no_device() { /* ... */ }

#[test]
fn test_process_error_invalid_config() { /* ... */ }
```

---

## Running Tests

### Basic Commands

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run tests in specific module
cargo test core::mocks

# Run integration tests only
cargo test --test integration_full_pipeline

# Run with output
cargo test -- --nocapture

# Run tests in parallel (default) or sequentially
cargo test -- --test-threads=1
```

### Test Filtering

```bash
# Run tests matching pattern
cargo test audio

# Run tests with specific feature flags
cargo test --no-default-features
cargo test --features notifications

# Run only doctests
cargo test --doc
```

### Test Output

```bash
# Quiet mode (only failures)
cargo test --quiet

# Verbose mode
cargo test --verbose

# Show test execution time
cargo test -- --show-output
```

---

## Debugging Tests

### Print Debugging

```rust
#[test]
fn test_with_debug_output() {
    let component = Component::new();
    println!("Component state: {:?}", component);

    let result = component.process();
    println!("Result: {:?}", result);

    assert!(result.is_ok());
}

// Run with output shown:
// cargo test test_with_debug_output -- --nocapture
```

### Using `dbg!` Macro

```rust
#[test]
fn test_with_dbg() {
    let value = complex_calculation();
    dbg!(&value);  // Prints value with file/line info

    assert_eq!(value, expected);
}
```

### Conditional Compilation for Debug Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]  // Ignored by default, run with: cargo test -- --ignored
    fn expensive_debug_test() {
        // Long-running debug test
    }
}
```

---

## Quick Reference

### Adding a New Test

```bash
# 1. Identify what to test
# 2. Check if mock exists
rg "struct Mock" src/core/mocks.rs

# 3. Add test to module or tests/
# 4. Run test
cargo test test_name -- --nocapture

# 5. Verify passes
cargo test test_name
```

### Finding Existing Tests

```bash
# Find all test functions
rg "#\[test\]|#\[tokio::test\]" --type rust

# Find tests in specific module
rg "#\[test\]" src/core/state.rs

# Find integration tests
fd "integration" tests/

# Find mock usage
rg "Mock(Audio|Transcriber|TextOutput)" --type rust
```

---

## Related Documentation

- `src/core/mocks.rs` - All mock implementations
- `.ai/knowledge/conventions.md` - General coding conventions
- `.ai/knowledge/architecture.md` - System architecture
- `.ai/knowledge/error-handling.md` - Error handling patterns

---

**Remember:** Tests are documentation. They show how the system should work. Write clear, focused tests that future developers (and AI agents) can learn from. When in doubt, test it.
