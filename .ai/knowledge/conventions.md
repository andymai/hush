# Hush Project Conventions

**Last Updated:** 2025-11-19
**Status:** Reference for AI agents working on Hush

This document outlines the conventions, patterns, and standards used in the Hush voice-to-text project.

---

## Code Organization

### Directory Structure

```
src/
├── core/              # Core abstractions (traits, errors, state, types)
├── adapters/          # Platform-specific implementations
│   ├── audio/        # Audio capture adapters (CPAL, etc.)
│   ├── hotkey/       # Hotkey adapters
│   ├── text/         # Text insertion adapters (X11, UInput)
│   └── transcription/# Transcription adapters (Whisper)
├── application/       # Application orchestration and builder
├── audio/             # Audio capture and feedback
├── cli/               # Command-line interface
│   └── commands/     # CLI subcommands
├── config/            # Configuration management
├── hotkey/            # Hotkey handling
├── overlay/           # Visual overlay UI
├── text/              # Text insertion
├── text_processing/   # Intelligent text processing (LLM, filler words)
├── transcription/     # Whisper integration
└── tray/              # System tray integration
```

### Module Organization Principles

1. **Core First**: All traits and types go in `src/core/`
2. **Adapters Pattern**: Platform-specific code in `src/adapters/`
3. **Single Responsibility**: Each module has one clear purpose
4. **Flat Structure**: Avoid deep nesting (max 3 levels)

---

## Rust Conventions

### Trait-Based Architecture

**Pattern**: Define traits in `src/core/traits.rs`, implement in adapters.

```rust
// src/core/traits.rs
#[async_trait::async_trait]
pub trait AudioSource: Send + Sync {
    fn start_recording(&mut self) -> Result<()>;
    fn stop_recording(&mut self) -> Result<AudioBuffer>;
    fn is_recording(&self) -> bool;
}

// src/adapters/audio/cpal_adapter.rs
pub struct CpalAudioSource { /* ... */ }

impl AudioSource for CpalAudioSource {
    fn start_recording(&mut self) -> Result<()> { /* ... */ }
    // ...
}
```

**Key Points:**
- All traits must be `Send + Sync` for async/concurrent usage
- Use `#[async_trait]` for async methods in traits
- Trait objects: `Box<dyn Trait>` or `Arc<dyn Trait>`

### Error Handling

**Pattern**: Use structured error types with `thiserror`, not generic `anyhow!()`.

```rust
// src/core/error.rs
#[derive(Debug, thiserror::Error)]
pub enum HushError {
    #[error("Audio error: {0}")]
    Audio(#[from] AudioError),

    #[error("Transcription error: {0}")]
    Transcription(#[from] TranscriptionError),

    #[error("Text output error: {0}")]
    TextOutput(#[from] TextOutputError),
}

// Usage: Pattern match for recovery
match result {
    Err(HushError::Audio(AudioError::DeviceNotFound)) => {
        // Retry with default device
    }
    Err(e) => return Err(e),
}
```

**Error Handling Rules:**
1. Use `HushError` variants at boundaries
2. Use `anyhow` only for quick prototyping
3. Provide user-friendly error messages
4. Include context with `.context()` or `.with_context()`
5. Errors should have severity classification: Transient, Recoverable, Fatal

### State Management

**Pattern**: Centralized state machine in `src/core/state.rs`.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Idle,
    Recording { started_at: Instant },
    Transcribing { audio_duration: Duration },
    Inserting { text_length: usize },
    Error { recoverable: bool },
}

// Single source of truth
let mut state = StateMachine::new();
state.transition(AppState::Recording { started_at: Instant::now() })?;
```

**State Rules:**
1. Never duplicate state (e.g., multiple `is_recording` flags)
2. All transitions go through `StateMachine::transition()`
3. Validate transitions before applying
4. Use Observer pattern for state change notifications

### Async/Await

**Pattern**: Use `tokio` runtime, `#[async_trait]` for traits.

```rust
#[async_trait::async_trait]
pub trait Transcriber: Send + Sync {
    async fn transcribe(&self, audio: &AudioBuffer) -> Result<TranscriptionResult>;
}

// Usage
let result = transcriber.transcribe(&audio_buffer).await?;
```

**Async Rules:**
1. Use `#[tokio::test]` for async tests
2. All async traits must have `#[async_trait]` attribute
3. Avoid blocking operations in async context (use `spawn_blocking`)
4. Use `Arc<Mutex<T>>` or `Arc<RwLock<T>>` for shared mutable state

---

## Testing Conventions

### Test Organization

```rust
// Unit tests at bottom of module
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        // ...
    }

    #[tokio::test]
    async fn test_async_something() {
        // ...
    }
}

// Integration tests in tests/ directory
// tests/integration_test.rs
#[tokio::test]
async fn test_full_pipeline() {
    // ...
}
```

### Mock Implementations

**Pattern**: Mocks in `src/core/mocks.rs` for all traits.

```rust
// src/core/mocks.rs
pub struct MockAudioSource {
    pub samples: Vec<f32>,
    pub is_recording: bool,
}

impl AudioSource for MockAudioSource {
    fn start_recording(&mut self) -> Result<()> {
        self.is_recording = true;
        Ok(())
    }
    // ...
}
```

**Testing Rules:**
1. All public traits must have mock implementations
2. Use mocks for hardware-free testing (no mic, GPU, X11 required)
3. Test error scenarios with mocks
4. Integration tests use real components sparingly

### Test Commands

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run with output
cargo test -- --nocapture

# Run tests in specific file
cargo test --test integration_test

# Run benchmarks
cargo bench
```

---

## Code Quality

### Required Checks Before Commit

```bash
# 1. Type check
cargo check

# 2. Run tests
cargo test

# 3. Lint (no warnings allowed)
cargo clippy -- -D warnings

# 4. Format
cargo fmt

# 5. Check formatting
cargo fmt --check
```

### Clippy Rules

- **No warnings in CI**: Use `-- -D warnings`
- Common lints to watch:
  - `clippy::unwrap_used` - Don't use `unwrap()` in production
  - `clippy::expect_used` - Avoid `expect()` except in tests
  - `clippy::panic` - No panics in library code
  - `clippy::missing_docs_in_private_items` - Document private APIs too

### Documentation

**Pattern**: Doc comments on all public items.

```rust
/// Captures audio from a microphone or audio input device.
///
/// # Example
///
/// ```rust
/// let mut audio = CpalAudioSource::new(None)?;
/// audio.start_recording()?;
/// std::thread::sleep(Duration::from_secs(3));
/// let buffer = audio.stop_recording()?;
/// ```
pub struct CpalAudioSource { /* ... */ }
```

**Documentation Rules:**
1. All `pub` items must have doc comments
2. Include examples in doc comments when helpful
3. Document error conditions
4. Document trait requirements (Send, Sync, etc.)
5. Keep docs up to date with code changes

---

## Dependencies

### Managing Dependencies

**In Cargo.toml:**

```toml
[dependencies]
# Audio
cpal = "0.15"  # Keep at 0.15 - 0.16 has breaking changes

# Async
tokio = { version = "1.38", features = ["full"] }
async-trait = "0.1"

# Error handling
anyhow = "1.0"
thiserror = "1.0"
```

**Dependency Rules:**
1. Pin major versions with comments explaining why
2. Document breaking changes in version comments
3. Minimize dependencies when possible
4. Prefer pure Rust crates (avoid C bindings unless necessary)
5. Check licenses (MIT/Apache-2.0 preferred)

### Common Dependencies

- **Audio**: `cpal`, `hound`, `rodio`
- **Transcription**: `whisper-rs`, `candle-*` (CUDA support)
- **GUI**: `egui`, `egui_overlay`
- **System**: `x11rb`, `input-linux` (UInput)
- **Async**: `tokio`, `async-trait`
- **Error Handling**: `anyhow`, `thiserror`
- **Logging**: `tracing`, `tracing-subscriber`
- **CLI**: `clap`
- **Config**: `toml`, `serde`

---

## Git Conventions

### Branch Naming

```bash
# Feature branches
feature/add-wayland-support

# Bug fixes
fix/audio-device-crash

# Agent branches
agent/{agent-id}/{task-id}
```

### Commit Messages

**Format:**

```
Step N: Brief description of what changed

Longer explanation of why this change was needed.
What problem does it solve?
```

**Examples:**

```
Step 1: Add AudioSource trait definition

Defines the core trait for audio capture abstraction.
This enables mock implementations for testing.

Step 2: Implement CpalAudioSource adapter

Adapts existing AudioCapture to new AudioSource trait.
Maintains backward compatibility during migration.
```

**Commit Rules:**
1. Start with "Step N:" for task-based work
2. Brief summary in imperative mood (50 chars max)
3. Detailed explanation if needed (wrap at 72 chars)
4. Reference issue/task IDs when applicable
5. Commit often (after each logical change)

---

## Configuration

### Config Files

**Location:** `~/.config/hush/config.toml` or project root `.env`

```toml
# config.toml
[audio]
device = "default"
sample_rate = 16000

[transcription]
model = "base"
language = "en"

[hotkey]
trigger = "Ctrl+Alt+V"

[text_output]
method = "uinput"  # or "x11", "clipboard"
```

**Environment Variables:** (in `.env`)

```bash
ANTHROPIC_API_KEY=your_api_key_here  # For LLM text polishing
```

---

## Performance Conventions

### GPU Acceleration

**Pattern**: Detect CUDA availability, fall back to CPU.

```rust
// Check GPU availability
let use_gpu = candle_core::cuda_is_available();

// Model loading with GPU
let model = if use_gpu {
    WhisperModel::load_with_cuda(model_path)?
} else {
    WhisperModel::load_cpu(model_path)?
};
```

### Benchmarking

```bash
# Run benchmarks
cargo bench

# Benchmark specific function
cargo bench bench_name
```

**Location:** `benches/architecture_benchmarks.rs`

---

## Logging

### Using Tracing

```rust
use tracing::{debug, info, warn, error};

info!("Starting transcription");
debug!(duration_ms = ?duration.as_millis(), "Audio recorded");
warn!("GPU not available, falling back to CPU");
error!(error = ?e, "Failed to load model");
```

**Log Levels:**
- `error` - Fatal errors
- `warn` - Recoverable issues
- `info` - Important events (user actions)
- `debug` - Detailed diagnostics
- `trace` - Very verbose (rarely used)

**Configuration:**
```bash
# Enable debug logs
RUST_LOG=debug ./hush listen

# Enable trace for specific module
RUST_LOG=hush::audio=trace ./hush listen
```

---

## Platform-Specific Code

### X11 vs Wayland

**Pattern**: Abstract behind `DisplayServer` trait.

```rust
// src/adapters/text/mod.rs
pub trait DisplayServer: Send + Sync {
    fn get_focused_window(&self) -> Result<WindowInfo>;
    fn simulate_keystrokes(&mut self, text: &str) -> Result<()>;
}

// Platform detection
#[cfg(feature = "x11")]
pub type PlatformDisplayServer = X11DisplayServer;

#[cfg(feature = "wayland")]
pub type PlatformDisplayServer = WaylandDisplayServer;
```

### UInput (Linux Kernel)

**Pattern**: Kernel-level keyboard emulation for universal compatibility.

```rust
// src/text/uinput_keyboard.rs
// Requires /dev/uinput access and proper permissions
```

**Setup:**
```bash
# Quick setup
./hush setup uinput --quick

# Diagnose issues
./hush setup diagnose-uinput
```

---

## Common Patterns to Follow

1. **Builder Pattern** for complex initialization (see `src/application/builder.rs`)
2. **Adapter Pattern** for platform abstraction
3. **Observer Pattern** for state change notifications
4. **Strategy Pattern** for text insertion methods
5. **Factory Pattern** for creating trait implementations

---

## Anti-Patterns to Avoid

1. ❌ **Multiple sources of truth** (duplicated state)
2. ❌ **Direct platform dependencies** in core logic
3. ❌ **Generic `anyhow!()` errors** at boundaries
4. ❌ **Blocking operations** in async contexts
5. ❌ **`unwrap()` and `panic!()` in production code
6. ❌ **Deeply nested modules** (keep flat)
7. ❌ **Concrete types in function signatures** (use traits)

---

## Quick Reference

### When implementing a new feature:

1. ✅ Define trait in `src/core/traits.rs`
2. ✅ Add to `HushError` in `src/core/error.rs`
3. ✅ Create adapter in `src/adapters/`
4. ✅ Add mock in `src/core/mocks.rs`
5. ✅ Write tests using mocks
6. ✅ Document with `///` doc comments
7. ✅ Run `cargo check && cargo test && cargo clippy && cargo fmt`

### Before committing:

```bash
cargo check && \
cargo test && \
cargo clippy -- -D warnings && \
cargo fmt && \
echo "✅ Ready to commit"
```

---

**Remember**: This is a **trait-based**, **async**, **Rust** project with strong emphasis on **testability**, **platform independence**, and **user privacy**. When in doubt, check existing patterns in `src/core/` and `src/adapters/`.
