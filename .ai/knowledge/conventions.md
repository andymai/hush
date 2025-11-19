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

### Unsafe Code

**Pattern**: Minimize `unsafe`, document thoroughly when necessary.

The Hush codebase aims to minimize `unsafe` code, but it is necessary in limited cases:

1. **Current Usage**: `unsafe impl Send` and `unsafe impl Sync` in `src/adapters/audio/cpal_adapter.rs`
2. **Total Count**: 2 unsafe impls (as of 2025-11-19)

#### When Unsafe is Necessary

Unsafe code is ONLY acceptable when:

1. **No safe alternative exists**: The functionality cannot be achieved with safe Rust
2. **External requirements**: Third-party libraries or OS APIs impose unsafe requirements
3. **Performance-critical with proof**: Benchmarks show significant improvement AND safety is verified
4. **Trait bounds on non-Send/Sync types**: Wrapping platform-specific types for trait objects

**Example: Audio Stream Threading**

```rust
/// SAFETY: This type wraps a non-Send/Sync type (cpal::Stream) but is ONLY
/// accessed through Arc<Mutex<>>, ensuring exclusive access.
///
/// See detailed safety documentation in src/adapters/audio/cpal_adapter.rs
unsafe impl Send for ThreadSafeAudioCapture {}
unsafe impl Sync for ThreadSafeAudioCapture {}
```

#### Unsafe Code Documentation Requirements

Every `unsafe` block or impl MUST include comprehensive SAFETY documentation:

```rust
/// SAFETY: [Short one-line summary]
///
/// ## Why unsafe is necessary
/// [Explanation of why safe Rust cannot achieve this]
///
/// ## Safety invariants
/// 1. [First invariant and how it's maintained]
/// 2. [Second invariant and how it's maintained]
///
/// ## What could go wrong
/// [Scenarios that would violate safety]
///
/// ## Why this is safe
/// [Proof that invariants are maintained]
///
/// ## Alternatives considered
/// [Other approaches and why they weren't used]
///
/// ## Testing
/// [How safety is verified through tests]
unsafe impl Send for Type {}
```

**Required Elements:**

1. **Why unsafe is necessary**: Explain the underlying reason (e.g., "cpal::Stream is not Send because...")
2. **Safety invariants**: List all conditions that MUST hold for safety
3. **What could go wrong**: Enumerate potential safety violations
4. **Why this is safe**: Prove invariants are maintained (private constructors, Arc<Mutex<>>, etc.)
5. **Alternatives considered**: Document safe alternatives and why they were rejected
6. **Testing**: Describe tests that verify safety (multi-threading, stress tests, etc.)

#### Unsafe Code Testing Requirements

All `unsafe` code MUST have:

1. **Multi-threaded tests**: Verify Send/Sync bounds actually work
2. **Stress tests**: Exercise concurrent access patterns
3. **Compile-time tests**: Demonstrate safety invariants (e.g., private constructors)
4. **Documentation tests**: Show safe usage patterns

**Example Tests:**

```rust
#[test]
fn test_send_across_threads() {
    let obj = create_object();
    std::thread::spawn(move || {
        // Use obj safely
    });
}

#[test]
fn test_concurrent_access() {
    let obj = Arc::new(create_object());
    // Spawn multiple threads accessing obj
}
```

#### Unsafe Code Review Checklist

Before approving `unsafe` code:

- [ ] Comprehensive SAFETY documentation with all required sections
- [ ] Multi-threaded tests verify Send/Sync safety
- [ ] No safe alternative exists or safe alternative is documented as rejected
- [ ] Safety invariants are enforced (private constructors, type system, etc.)
- [ ] Technical debt is noted if a better solution exists
- [ ] Architecture documentation mentions the unsafe pattern

#### Current Unsafe Code Inventory

**src/adapters/audio/cpal_adapter.rs** (2 unsafe impls):
- `unsafe impl Send for ThreadSafeAudioCapture`
- `unsafe impl Sync for ThreadSafeAudioCapture`
- **Reason**: cpal::Stream is not Send/Sync, but we need Send+Sync for trait objects
- **Safety**: Private constructor + mandatory Arc<Mutex<>> wrapper ensures exclusive access
- **Technical Debt**: Could be eliminated with message-passing architecture
- **Tests**: Multi-threaded stress tests verify safety assumptions

#### Future Unsafe Code

Any new `unsafe` code must:

1. Be reviewed by at least one other developer (or flagged for human review by agents)
2. Follow all documentation requirements above
3. Include comprehensive tests
4. Be added to the unsafe code inventory in this document
5. Consider if it could be refactored away in the future

**Prefer safe alternatives:**
- Use safe abstractions from `std` and trusted crates
- Use message passing instead of shared mutable state
- Use type system to enforce invariants (phantom types, marker types)
- Consider if functionality is truly necessary

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

### GPU Acceleration and CUDA Feature Flag

**Pattern**: Use CUDA feature flag for compile-time control, detect at runtime for fallback.

#### Build Configurations

```bash
# GPU-accelerated build (default)
cargo build --release

# CPU-only build (no CUDA toolkit required)
cargo build --release --no-default-features --features notifications,system-tray
```

#### Runtime Detection

```rust
use crate::transcription::cuda::CudaAvailability;

// Detect CUDA at runtime (cached)
let cuda = CudaAvailability::detect();

if cuda.available {
    info!(
        "Using GPU: {:?} (CUDA {})",
        cuda.device_name,
        cuda.cuda_version
    );
    // Use GPU acceleration
} else {
    info!("Using CPU (CUDA not available or not compiled)");
    // Fall back to CPU
}

// Check if currently using GPU
if CudaAvailability::is_available() {
    // GPU code path
}
```

#### Feature Flag Configuration

In `Cargo.toml`:

```toml
[features]
default = ["notifications", "system-tray", "cuda"]
cuda = [
    "candle-core/cuda",
    "candle-nn/cuda",
    "candle-transformers/cuda",
    "whisper-rs/cuda"
]

[dependencies]
# CUDA features are optional via the "cuda" feature flag
candle-core = { version = "0.8" }
candle-nn = { version = "0.8" }
candle-transformers = { version = "0.8" }
```

#### Conditional Compilation

```rust
#[cfg(feature = "cuda")]
{
    // CUDA-specific code
    let available = candle_core::utils::cuda_is_available();
}

#[cfg(not(feature = "cuda"))]
{
    // CPU-only fallback
    info!("Built without CUDA support (CPU-only mode)");
}
```

**CUDA Usage Rules:**
1. **Never hardcode GPU usage** - Always use `CudaAvailability::detect()`
2. **Graceful fallback** - Code must work without GPU
3. **Clear logging** - Inform user about GPU/CPU usage
4. **Detection is cached** - Use `CudaAvailability::detect()` freely
5. **Test both modes** - Verify CPU-only builds work

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

## Refactoring Safely

### When to Refactor

**Refactor when:**
- ✅ Code is duplicated in multiple places
- ✅ Function is >100 lines and has multiple responsibilities
- ✅ Following an established pattern (consistency)
- ✅ Tests exist to catch regressions
- ✅ Improving code quality without changing functionality

**Don't refactor when:**
- ❌ No tests exist to verify behavior
- ❌ Code is already well-structured
- ❌ Would be the only refactoring in codebase (breaks consistency)
- ❌ Mixing refactoring with feature changes

### Safe Refactoring Process

**1. Verify Current State**
```bash
# Run all tests before starting
cargo test

# Check current compilation
cargo check

# Verify clippy passes
cargo clippy -- -D warnings
```

**2. Make Small, Incremental Changes**
```bash
# Pattern: Extract function
# Step 1: Extract (keep old code)
# Step 2: Verify tests pass
cargo test
# Step 3: Replace call sites one by one
# Step 4: Verify after each replacement
cargo check
```

**3. Commit Frequently**
```bash
# Commit after each small refactoring step
git add -p  # Stage specific changes
git commit -m "Step 1: Extract helper function"
```

**4. Run Full Verification**
```bash
# After all changes
cargo check && \
  cargo test && \
  cargo clippy -- -D warnings && \
  cargo fmt
```

### Refactoring Patterns

**Extract Function:**
```rust
// Before: Complex function with duplicated logic
fn process_data(data: &[u8]) -> Result<()> {
    // 50 lines of validation
    // 30 lines of processing
    // 20 lines of storage
}

// After: Extracted responsibilities
fn process_data(data: &[u8]) -> Result<()> {
    validate_data(data)?;
    let processed = process_internal(data)?;
    store_result(processed)?;
    Ok(())
}

fn validate_data(data: &[u8]) -> Result<()> { /* ... */ }
fn process_internal(data: &[u8]) -> Result<ProcessedData> { /* ... */ }
fn store_result(data: ProcessedData) -> Result<()> { /* ... */ }
```

**Extract Module:**
```rust
// Before: Large dispatcher.rs with all command logic

// After: Modular structure
// src/cli/dispatcher.rs - Routing only
// src/cli/commands/record.rs - Record command
// src/cli/commands/status.rs - Status command
```

**Introduce Parameter Object:**
```rust
// Before: Too many parameters
fn create_app(
    audio: Box<dyn AudioSource>,
    transcriber: Box<dyn Transcriber>,
    output: Box<dyn TextOutput>,
    trigger: Box<dyn InputTrigger>,
    config: Config,
) -> Result<App> { /* ... */ }

// After: Builder pattern
let app = HushAppBuilder::new()
    .with_audio(audio)
    .with_transcriber(transcriber)
    .with_text_output(output)
    .with_trigger(trigger)
    .with_config(config)
    .build()?;
```

### Rollback Strategy

**If refactoring goes wrong:**
```bash
# Stash current changes
git stash

# Or reset to last commit
git reset --hard HEAD

# Or reset to specific commit
git reset --hard <commit-hash>

# Review what changed
git diff HEAD~1
```

---

## Error Handling Improvements

### Replacing unwrap() and expect()

**Problem:** `unwrap()` and `expect()` cause panics in production.

**Solution:** Replace with proper error handling.

**Pattern 1: Use `?` operator**
```rust
// Before
let value = some_option.unwrap();
let result = some_result.expect("Failed");

// After
let value = some_option
    .ok_or_else(|| HushError::Config("Missing value".into()))?;
let result = some_result
    .context("Failed to process")?;
```

**Pattern 2: Provide context**
```rust
use anyhow::Context;

// Before
let file = std::fs::read(path).unwrap();

// After
let file = std::fs::read(path)
    .context(format!("Failed to read file at {}", path.display()))?;
```

**Pattern 3: Match for different errors**
```rust
// Before
let config = load_config().expect("Config load failed");

// After
let config = load_config().unwrap_or_else(|e| {
    eprintln!("Warning: Could not load config: {}", e);
    eprintln!("Using default configuration");
    Config::default()
});
```

**Pattern 4: Map errors to domain errors**
```rust
// Before
let device = get_device().unwrap();

// After
let device = get_device()
    .map_err(|e| AudioError::DeviceNotFound(e.to_string()))?;
```

### When unwrap() is Acceptable

**Test code:**
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_something() {
        let app = build_test_app().unwrap();  // OK in tests
        // ...
    }
}
```

**Documented invariants:**
```rust
// OK - RwLock poisoning is unrecoverable
let state = self.state.read()
    .expect("State lock poisoned - this is a fatal bug");
```

### Finding unwrap/expect Usage

```bash
# Find all unwrap/expect calls
rg "\.unwrap\(\)|\.expect\(" --type rust src/

# Exclude test code
rg "\.unwrap\(\)|\.expect\(" --type rust src/ -g '!*test*'

# Use clippy to catch
cargo clippy -- \
    -W clippy::unwrap_used \
    -W clippy::expect_used
```

---

## Documentation Standards

### Code Documentation

**All public items must have doc comments:**
```rust
/// Captures audio from the system microphone
///
/// This struct provides a high-level interface for audio recording,
/// abstracting over platform-specific audio APIs.
///
/// # Examples
///
/// ```
/// let mut audio = AudioCapture::new()?;
/// audio.start_recording()?;
/// std::thread::sleep(Duration::from_secs(3));
/// let buffer = audio.stop_recording()?;
/// ```
pub struct AudioCapture { /* ... */ }

/// Starts recording audio from the default input device
///
/// # Errors
///
/// Returns `AudioError::NoDeviceAvailable` if no microphone is detected.
/// Returns `AudioError::RecordingStartFailed` if the device cannot be opened.
///
/// # Examples
///
/// ```
/// let mut audio = AudioCapture::new()?;
/// audio.start_recording()?;
/// ```
pub fn start_recording(&mut self) -> Result<()> {
    // Implementation
}
```

### Documentation Structure

**Public API documentation should include:**

1. **Brief description** - One line summary
2. **Detailed explanation** - What it does and why
3. **Parameters** - What each parameter means
4. **Returns** - What it returns (Ok and Err cases)
5. **Errors** - When and why errors occur
6. **Examples** - How to use it
7. **Panics** - If it can panic, when and why
8. **Safety** - For unsafe code, detailed safety requirements

**Example:**
```rust
/// Transcribes audio buffer to text using Whisper model
///
/// Uses GPU acceleration if available, falls back to CPU.
/// The transcription process typically takes 0.5-5 seconds
/// depending on audio length and hardware.
///
/// # Arguments
///
/// * `audio` - Audio buffer to transcribe (16kHz, mono recommended)
///
/// # Returns
///
/// * `Ok(TranscriptionResult)` - Transcribed text with metadata
/// * `Err(TranscriptionError::NoSpeechDetected)` - If audio is silent
/// * `Err(TranscriptionError::AudioTooShort)` - If audio < 0.1 seconds
///
/// # Examples
///
/// ```
/// let audio = record_audio()?;
/// let result = transcriber.transcribe(&audio).await?;
/// println!("Transcribed: {}", result.text);
/// ```
pub async fn transcribe(&self, audio: &AudioBuffer) -> Result<TranscriptionResult> {
    // Implementation
}
```

### Module Documentation

**Add module-level docs to `mod.rs` or top of file:**
```rust
//! Audio capture and feedback module
//!
//! This module provides audio recording functionality using CPAL,
//! with support for multiple audio backends (ALSA, PulseAudio, etc.)
//! on Linux.
//!
//! # Examples
//!
//! ```
//! use hush::audio::AudioCapture;
//!
//! let mut capture = AudioCapture::new()?;
//! capture.start_recording()?;
//! ```

pub mod capture;
pub mod feedback;
```

### Security Documentation

**For security-sensitive code, document:**

```rust
/// Inserts text into the focused window using UInput
///
/// # Security Considerations
///
/// This function creates a virtual keyboard device that can send
/// keystrokes to any application. It requires:
/// - Access to /dev/uinput (root or input group)
/// - User must trust Hush with keyboard input capability
///
/// The text is NOT logged or stored anywhere after insertion.
///
/// # Privacy
///
/// All transcription happens locally. No data is sent to external servers
/// unless LLM polishing is explicitly enabled.
pub async fn insert_text(&mut self, text: &str) -> Result<()> {
    // Implementation
}
```

### AI Agent Documentation

**When creating knowledge files for AI agents:**

1. **Focus on patterns, not details** - Teach how to fish
2. **Include grep/search commands** - Help agents verify
3. **Provide code examples** - Show the right way
4. **Link to actual files** - Point to source of truth
5. **Explain the "why"** - Not just the "what"

**Example:**
```markdown
## Finding Trait Definitions

Before implementing a trait, always verify it exists:

\`\`\`bash
# Find trait definition
rg "pub trait AudioSource" src/core/traits.rs --context=5

# Find implementations
rg "impl AudioSource for" --type rust

# Check if mock exists
rg "MockAudioSource" src/core/mocks.rs
\`\`\`

**Never assume a trait exists.** If not found, either:
1. Create it (if you're designing new architecture)
2. Block the task (if expected to exist)
\`\`\`
```

---

**Remember**: This is a **trait-based**, **async**, **Rust** project with strong emphasis on **testability**, **platform independence**, and **user privacy**. When in doubt, check existing patterns in `src/core/` and `src/adapters/`.
