# Hush Architecture Summary

**Last Updated:** 2025-11-19
**Status:** Complete reference for AI agents

This is the complete architecture reference for AI agents working on Hush. All architecture details are consolidated in this file.

---

## System Overview

**Hush** is a local-first voice-to-text application for Linux with:
- GPU-accelerated Whisper transcription
- Universal text insertion (UInput kernel-level)
- Privacy-first (all local, optional LLM polishing)
- Trait-based architecture for testability and platform independence

---

## Architecture Principles

1. **Trait-Based Architecture**: All components defined as traits in `src/core/`
2. **Adapter Pattern**: Platform-specific implementations in `src/adapters/`
3. **Centralized State**: Single state machine in `src/core/state.rs`
4. **Structured Errors**: Domain-specific errors, not generic `anyhow!`
5. **Mock-First Testing**: All traits have mock implementations

---

## Core Architecture

### Component Hierarchy

```
┌────────────────────────────────────────┐
│         Application Layer              │
│   (HushApp, Pipeline, Orchestration)   │
└─────────────────┬──────────────────────┘
                  │
         ┌────────┴────────┐
         │                 │
    ┌────▼────┐      ┌────▼────┐
    │  Core   │      │Adapters │
    │ Traits  │◄─────┤  Impls  │
    └─────────┘      └─────────┘
         │
    ┌────▼────────────────────┐
    │  Audio | Text | Hotkey  │
    │  Transcription | State  │
    └────────────────────────┘
```

### Data Flow

```
User Input (Hotkey)
    → Audio Recording (CPAL)
    → Transcription (Whisper)
    → Text Processing (LLM optional)
    → Text Insertion (UInput/X11)
    → User Feedback (Overlay/Sound)
```

---

## Core Traits

**Location:** `src/core/traits.rs`

### 1. AudioSource

```rust
pub trait AudioSource: Send + Sync {
    fn start_recording(&mut self) -> Result<()>;
    fn stop_recording(&mut self) -> Result<AudioBuffer>;
    fn is_recording(&self) -> bool;
    fn device_name(&self) -> &str;
    fn config(&self) -> AudioConfig;
}
```

**Implementations:**
- `CpalAudioSource` - Production (CPAL library)
- `MockAudioSource` - Testing

**Location:** `src/adapters/audio/cpal_adapter.rs`, `src/core/mocks.rs`

### 2. Transcriber

```rust
#[async_trait]
pub trait Transcriber: Send + Sync {
    async fn transcribe(&self, audio: &AudioBuffer) -> Result<TranscriptionResult>;
    fn info(&self) -> TranscriberInfo;
    async fn is_ready(&self) -> bool;
}
```

**Implementations:**
- `WhisperTranscriber` - Production (Whisper with CUDA)
- `MockTranscriber` - Testing

**Location:** `src/adapters/transcription/whisper_adapter.rs`, `src/core/mocks.rs`

### 3. TextOutput

```rust
#[async_trait]
pub trait TextOutput: Send + Sync {
    async fn insert_text(&mut self, text: &str) -> Result<()>;
    async fn focused_window(&self) -> Result<Option<WindowInfo>>;
    fn is_available(&self) -> bool;
    fn output_method(&self) -> &str;
}
```

**Implementations:**
- `UInputTextOutput` - Production (Linux UInput, universal)
- `X11TextOutput` - Production (X11 fallback)
- `MockTextOutput` - Testing

**Location:** `src/adapters/text/`, `src/core/mocks.rs`

### 4. InputTrigger

```rust
#[async_trait]
pub trait InputTrigger: Send + Sync {
    async fn start_listening(&mut self) -> Result<()>;
    async fn stop_listening(&mut self) -> Result<()>;
    fn event_stream(&self) -> mpsc::Receiver<TriggerEvent>;
    fn description(&self) -> String;
}
```

**Implementations:**
- `HotkeyTrigger` - Production (global hotkeys)
- `MockInputTrigger` - Testing

**Location:** `src/adapters/hotkey/`, `src/core/mocks.rs`

---

## Directory Structure

```
src/
├── core/                     # ⭐ START HERE for understanding architecture
│   ├── traits.rs            # All trait definitions
│   ├── error.rs             # HushError and domain errors
│   ├── state.rs             # State machine
│   ├── types.rs             # Common types (AudioBuffer, etc.)
│   └── mocks.rs             # Mock implementations for testing
│
├── adapters/                 # Platform-specific implementations
│   ├── audio/
│   │   └── cpal_adapter.rs  # CPAL → AudioSource
│   ├── hotkey/
│   │   └── hotkey_adapter.rs # global-hotkey → InputTrigger
│   ├── text/
│   │   ├── x11_adapter.rs   # X11 → TextOutput
│   │   └── uinput_adapter.rs # UInput → TextOutput
│   └── transcription/
│       └── whisper_adapter.rs # Whisper → Transcriber
│
├── application/              # Application orchestration
│   ├── builder.rs           # Builder pattern for HushApp
│   └── hush_app.rs          # Main application logic
│
├── audio/                    # Audio utilities
│   ├── capture.rs           # Audio capture logic
│   └── feedback.rs          # Audio feedback (beeps)
│
├── cli/                      # Command-line interface
│   ├── commands/            # Subcommands (listen, record, test, etc.)
│   └── dispatcher.rs        # CLI routing
│
├── config/                   # Configuration management
│   └── settings.rs          # Config loading/parsing
│
├── hotkey/                   # Hotkey handling
│   └── global.rs            # Global hotkey registration
│
├── overlay/                  # Visual overlay UI
│   ├── ui.rs                # Overlay rendering (egui)
│   ├── state.rs             # Overlay state
│   └── window.rs            # Overlay window management
│
├── text/                     # Text insertion
│   ├── insertion.rs         # Multi-method text insertion
│   └── uinput_keyboard.rs   # UInput keyboard emulation
│
├── text_processing/          # Intelligent text processing
│   ├── commands.rs          # Voice commands ("new paragraph", "undo")
│   ├── filler_words.rs      # Filler word removal
│   ├── llm.rs               # LLM integration (Claude API)
│   └── executor.rs          # Text processing pipeline
│
├── transcription/            # Whisper integration
│   ├── whisper.rs           # Whisper interface
│   ├── models.rs            # Model management
│   └── cuda.rs              # CUDA detection
│
└── main.rs                   # Entry point
```

---

## State Management

**Location:** `src/core/state.rs`

### State Machine

```rust
pub enum AppState {
    Idle,
    Recording { started_at: Instant },
    Transcribing { audio_duration: Duration },
    Inserting { text_length: usize },
    Error { recoverable: bool },
}
```

### Valid Transitions

```
Idle → Recording
Recording → Transcribing | Idle
Transcribing → Inserting | Idle
Inserting → Idle
Any → Error
Error → Idle
```

### State Observer Pattern

```rust
pub trait StateObserver: Send + Sync {
    fn on_state_change(&self, old_state: AppState, new_state: AppState);
}
```

**Usage:** Overlay, audio feedback, and logging observe state changes.

---

## Error Handling

**Location:** `src/core/error.rs`

### Error Hierarchy

```rust
pub enum HushError {
    Audio(AudioError),
    Transcription(TranscriptionError),
    TextOutput(TextOutputError),
    InputTrigger(InputTriggerError),
    State(StateError),
    Config(ConfigError),
}

pub enum ErrorSeverity {
    Transient,    // Retry automatically
    Recoverable,  // User can fix
    Fatal,        // Application must exit
}
```

### Error Recovery Strategy

```rust
match error.severity() {
    ErrorSeverity::Transient => retry_with_backoff(),
    ErrorSeverity::Recoverable => show_user_guidance(),
    ErrorSeverity::Fatal => shutdown_gracefully(),
}
```

---

## Key Design Patterns

### 1. Adapter Pattern

**Purpose:** Wrap existing implementations to match traits

**Example:**
```rust
// Existing code
struct AudioCapture { /* cpal-specific */ }

// Adapter
struct CpalAudioSource {
    inner: AudioCapture,
}

impl AudioSource for CpalAudioSource {
    fn start_recording(&mut self) -> Result<()> {
        self.inner.start()
    }
}
```

### 2. Builder Pattern

**Purpose:** Configure complex HushApp

**Example:**
```rust
let app = HushAppBuilder::new()
    .with_audio_source(Box::new(CpalAudioSource::new()?))
    .with_transcriber(Box::new(WhisperTranscriber::new()?))
    .with_text_output(Box::new(UInputTextOutput::new()?))
    .build()?;
```

**Location:** `src/application/builder.rs`

### 3. Strategy Pattern

**Purpose:** Choose text insertion method dynamically

**Example:**
```rust
pub enum InsertionMethod {
    UInput,       // Kernel-level (best)
    X11,          // X11 simulation (fallback)
    Clipboard,    // Paste (last resort)
}

fn choose_method(&self, text: &str) -> InsertionMethod {
    if uinput_available() { InsertionMethod::UInput }
    else if x11_available() { InsertionMethod::X11 }
    else { InsertionMethod::Clipboard }
}
```

**Location:** `src/text/insertion.rs`

### 4. Observer Pattern

**Purpose:** Notify components of state changes

**Example:**
```rust
// Overlay observes state to show visual feedback
impl StateObserver for OverlayState {
    fn on_state_change(&self, old: AppState, new: AppState) {
        match new {
            AppState::Recording { .. } => self.show_recording_indicator(),
            AppState::Idle => self.hide_indicator(),
            _ => {}
        }
    }
}
```

---

## Testing Strategy

### Mock Implementations

**All traits have mocks in `src/core/mocks.rs`**

```rust
pub struct MockAudioSource {
    pub samples: Vec<f32>,
    pub is_recording: bool,
}

pub struct MockTranscriber {
    pub responses: Vec<String>,
    pub call_count: usize,
}

pub struct MockTextOutput {
    pub inserted_texts: Vec<String>,
}
```

### Hardware-Free Testing

```rust
#[tokio::test]
async fn test_full_pipeline_no_hardware() {
    // No mic, GPU, or X11 required!
    let audio = Box::new(MockAudioSource::new());
    let transcriber = Box::new(MockTranscriber::new());
    let output = Box::new(MockTextOutput::new());

    let mut pipeline = SimplePipeline::new(audio, transcriber, output);
    let result = pipeline.process_once().await?;

    assert_eq!(result.text, "Expected transcription");
}
```

---

## Platform Abstraction

### Text Insertion Methods

1. **UInput** (Preferred) - Linux kernel-level keyboard emulation
   - Works everywhere (VMs, games, terminals, X11, Wayland)
   - Requires `/dev/uinput` access
   - Setup: `./hush setup uinput --quick`

2. **X11** (Fallback) - X11 protocol keyboard simulation
   - Works on X11 only
   - No special permissions needed

3. **Clipboard** (Last Resort) - Copy + paste
   - Universal but slower
   - Requires user paste action

### GPU Acceleration

```rust
// Auto-detect CUDA
let use_gpu = candle_core::cuda_is_available();

// Load model accordingly
let transcriber = if use_gpu {
    WhisperTranscriber::new_with_gpu(model_path)?
} else {
    WhisperTranscriber::new_cpu(model_path)?
};
```

**Performance:**
- GPU: ~0.5s for 3s audio (base model)
- CPU: ~4-6s for 3s audio (base model)

---

## Configuration

### Config File Location

- `~/.config/hush/config.toml` (Linux)
- Project root `.env` (development)

### Config Structure

```toml
[audio]
device = "default"
sample_rate = 16000

[transcription]
model = "base"           # tiny, base, small, medium, large
language = "en"
use_gpu = true

[hotkey]
trigger = "Ctrl+Alt+V"

[text_output]
method = "uinput"        # uinput, x11, clipboard

[text_processing]
remove_filler_words = true
use_llm_polishing = false
editing_mode = "medium"  # light, medium, aggressive
```

---

## Build & Test Commands

```bash
# Build
make build              # Debug build
make release            # Release build

# Test
cargo test              # All tests
cargo test {name}       # Specific test
cargo test -- --nocapture  # With output

# Quality
cargo check             # Type check (fast)
cargo clippy -- -D warnings  # Lint
cargo fmt               # Format

# Run
./hush listen           # Start listening mode
./hush record --duration 5  # Record once
./hush status --full    # System status
./hush test audio       # Test audio
./hush test text-insertion  # Test text insertion
```

---

## Architecture Decision Records (ADRs)

**Location:** `docs/architecture/adrs/`

1. **ADR-001**: Trait-Based Architecture
   - **Decision:** Use traits for all components
   - **Rationale:** Testability, platform independence, extensibility

2. **ADR-002**: Error Handling Strategy
   - **Decision:** Structured errors with `thiserror`
   - **Rationale:** Pattern matching, recovery, user-friendly messages

3. **ADR-003**: Centralized State Management
   - **Decision:** Single state machine
   - **Rationale:** Eliminate state duplication, validate transitions

---

## Critical Files Checklist

**Before making changes, check these files:**

1. ✅ `src/core/traits.rs` - Does the trait already exist?
2. ✅ `src/core/error.rs` - What error type should I use?
3. ✅ `src/core/state.rs` - What are the valid states?
4. ✅ `src/core/mocks.rs` - Do I need to add a mock?
5. ✅ `docs/ARCHITECTURE.md` - Full architecture details
6. ✅ `.ai/knowledge/conventions.md` - Project conventions

---

## Common Search Patterns

```bash
# Find trait definition
rg "pub trait AudioSource" src/core/traits.rs

# Find implementations
rg "impl AudioSource for" --type rust

# Find error types
rg "pub enum.*Error" src/core/error.rs

# Find state transitions
rg "transition\(" src/core/state.rs

# Find existing tests
rg "#\[test\]|#\[tokio::test\]" tests/ src/

# Find adapter implementations
fd "adapter" src/adapters/ --type f
```

---

## Migration Status (as of 2025-11-19)

### ✅ Completed
- Trait definitions in `src/core/`
- Error handling with `HushError`
- State machine implementation
- Mock implementations for testing
- UInput text insertion (universal)
- Audio capture with CPAL
- Whisper transcription with CUDA
- Overlay UI with real-time feedback

### ⚠️ In Progress
- Full migration to trait objects in `HushApp`
- Comprehensive test coverage with mocks
- Wayland display server adapter

### 📋 Future
- Plugin system for extensibility
- Alternative transcription backends
- Cross-platform support (Windows, macOS)

---

## Quick Reference Card

| Question | Answer |
|----------|--------|
| Where are traits? | `src/core/traits.rs` |
| Where are errors? | `src/core/error.rs` |
| Where is state? | `src/core/state.rs` |
| Where are mocks? | `src/core/mocks.rs` |
| Where are adapters? | `src/adapters/*` |
| How to test? | `cargo test` |
| How to check types? | `cargo check` |
| How to lint? | `cargo clippy -- -D warnings` |
| How to format? | `cargo fmt` |
| Main entry point? | `src/main.rs` |

---

**For additional details, refer to:**
- `.ai/knowledge/conventions.md` - Coding conventions and patterns
- `.ai/knowledge/error-handling.md` - Error handling guide
- `.ai/knowledge/adr-summary.md` - Architecture decisions
- `.ai/knowledge/uinput-guide.md` - UInput text insertion guide
- `.ai/knowledge/voice-commands.md` - Voice commands reference

**Remember:** Traits define the contract, adapters provide the implementation, mocks enable testing. Always search before assuming.
