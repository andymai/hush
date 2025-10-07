# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Hush** is a fast, accurate voice-to-text service for Linux developers built in Rust. It provides local Whisper-based speech transcription with CUDA acceleration, designed specifically for developers who need reliable speech-to-text with excellent support for coding terminology.

**Key Technologies**: Rust 1.85.0, OpenAI Whisper (via Candle framework), CUDA acceleration, PipeWire/ALSA audio, X11 system integration

**Target Platform**: Ubuntu Linux with X11, PipeWire audio, and NVIDIA GPU (CUDA 13.0+)

## Build and Development Commands

### Building
```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release

# Build specific binaries
cargo build --bin hush          # Main application
cargo build --bin hush-mvp      # MVP version with simulation mode
cargo build --bin model-manager # Model management CLI
cargo build --bin simple-model-manager # Simplified model manager
```

### Running
```bash
# MVP application (with audio simulation)
PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig cargo run --bin hush-mvp

# Main application (requires real Whisper models)
PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig cargo run --bin hush

# Download Whisper models
./scripts/download-models.sh
```

### Testing
```bash
# Run all tests
cargo test

# Run integration tests specifically
cargo test --test integration_tests

# Run with logging enabled
RUST_LOG=debug cargo test
```

### System Dependencies
Required system packages (Ubuntu):
```bash
sudo apt-get install libasound2-dev libjack-jackd2-dev libxdo-dev xclip pkg-config
```

## Project Architecture

### Module Structure

The codebase is organized into clearly separated modules:

```
src/
├── main.rs              # Main application entry point
├── main_mvp.rs          # MVP with simulation mode (HushApp orchestration)
├── lib.rs               # Library exports and common types
├── audio/               # Audio capture and feedback
│   ├── capture.rs       # PipeWire/ALSA audio input (16kHz mono)
│   └── feedback.rs      # Audio feedback system (start/stop/error sounds)
├── transcription/       # Whisper integration
│   ├── whisper.rs       # Whisper transcription with simulation mode
│   ├── cuda.rs          # CUDA device management
│   └── models.rs        # Model management and downloads
├── hotkey/              # Global hotkey management
│   └── global.rs        # X11 hotkey registration (press/release detection)
├── text/                # Text insertion
│   └── insertion.rs     # X11-based typing with enigo + clipboard fallback
├── config/              # Configuration management
│   └── settings.rs      # TOML config loading and validation
├── error/               # Error handling
│   └── handler.rs       # Centralized error management
└── wakeword/            # Wake word detection (planned)
    └── detector.rs      # Voice activation (future feature)
```

### Core Pipeline Flow

**Hotkey → Record → Transcribe → Insert**

1. **User presses hotkey** (Ctrl+Shift+Space by default)
2. **Audio capture starts** via cpal with PipeWire backend (16kHz mono)
3. **User releases hotkey** to stop recording
4. **Whisper processes audio** with CUDA or CPU fallback
5. **Text is inserted** at cursor position using X11/enigo

### Key Components

**HushApp** (`src/main_mvp.rs`): Main orchestrator that manages the complete pipeline lifecycle. Handles:
- Component initialization
- Hotkey event processing
- Recording state management (atomic operations)
- Performance monitoring and metrics
- End-to-end error handling

**AudioCapture** (`src/audio/capture.rs`): Real-time audio capture with:
- PipeWire/ALSA/JACK backend support via cpal
- 16kHz mono sampling optimized for Whisper
- Thread-safe buffer management
- Simulation mode for headless/testing environments

**WhisperTranscriber** (`src/transcription/whisper.rs`): Transcription engine with:
- Candle-based Whisper integration
- CUDA acceleration with CPU fallback
- Simulation mode for development without models
- Model management and automatic downloads
- Confidence scoring and audio analysis

**HotkeyManager** (`src/hotkey/global.rs`): Global hotkey handling with:
- X11-based key combination parsing (supports Ctrl, Alt, Shift, Super)
- Press/release event detection via MPSC channels
- 50+ supported key codes (letters, numbers, function keys, punctuation)
- Thread-safe event distribution

**TextInserter** (`src/text/insertion.rs`): Smart text insertion with:
- X11 window focus detection
- Direct keyboard simulation via enigo
- Clipboard fallback using xclip
- Application-aware insertion strategies (terminals, editors, browsers)

**Config** (`src/config/settings.rs`): TOML-based configuration with:
- Audio, transcription, hotkey, wake word, and feedback settings
- Comprehensive validation with helpful error messages
- Default values for all settings
- Support for `config/default.toml` and user overrides

## Important Implementation Details

### Audio System
- **Sample Rate**: Always 16kHz for Whisper compatibility
- **Channels**: Mono (speech-focused)
- **Backends**: PipeWire (modern), ALSA (legacy), with automatic fallback
- **Simulation Mode**: Can run without real audio hardware for testing

### Whisper Integration
- **Framework**: Candle (pure Rust ML framework)
- **Models**: Supports tiny to large-v3 (39MB to 1.55GB)
- **Model Location**: `models/` directory with automatic HuggingFace Hub downloads
- **Devices**: CUDA-accelerated or CPU fallback
- **Simulation**: Realistic transcription simulation based on audio duration/quality

### Text Insertion Strategy
The system chooses insertion method based on:
- **Text length**: Short text → direct typing, long text → clipboard
- **Target app**: Terminals prefer clipboard, editors prefer direct typing
- **Special characters**: Complex Unicode → clipboard for reliability
- **Fallback chain**: Direct → Clipboard → Error with helpful message

### Error Handling Philosophy
- **Graceful Degradation**: Fall back to simulation modes when hardware unavailable
- **Recovery Mechanisms**: Automatic retries with fallback strategies
- **User-Friendly Messages**: Clear error explanations with actionable suggestions
- **Resource Cleanup**: Proper cleanup on all error paths

### Performance Targets
- **End-to-End**: < 1 second for typical 3-second recordings
- **Audio Feedback**: < 10ms (immediate response)
- **Transcription**: 100-2000ms depending on length and hardware
- **Text Insertion**: < 50ms
- **Accuracy**: > 95% English, > 90% technical terms

## Configuration

Default configuration is in `config/default.toml`. Key settings:

```toml
[audio]
sample_rate = 16000      # Required for Whisper
channels = 1             # Mono for speech
buffer_size = 1024       # Low-latency buffer
device = null            # Auto-detect or specify device name

[transcription]
model_path = "models/whisper-base"  # Model directory
model_size = "base"      # tiny, base, small, medium, large
language = "en"          # Primary language
use_cuda = true          # GPU acceleration
beam_size = 5            # Search parameter
no_speech_threshold = 0.6

[hotkey]
enabled = true
combination = "Ctrl+Shift+Space"  # Customizable

[wakeword]
enabled = false          # Future feature
phrase = "Hey Hush"
sensitivity = 0.5

[feedback]
audio_enabled = true     # Audio cues for start/stop/error
start_sound = "assets/sounds/start.wav"
stop_sound = "assets/sounds/stop.wav"
error_sound = "assets/sounds/error.wav"
```

## Development Workflow

### Current Status (as of October 2025)
- ✅ Tasks 1-5.5 Complete: Core transcription pipeline fully functional
- 🚧 Task 6: Global hotkeys (next priority)
- ⏳ Tasks 7-9: Text insertion, audio feedback, main event loop

### Making Changes

**When modifying audio capture:**
- Maintain 16kHz mono output for Whisper compatibility
- Update simulation mode if changing real audio behavior
- Test with both PipeWire and ALSA backends

**When modifying transcription:**
- Keep simulation mode in sync with real transcription behavior
- Update confidence scoring logic if changing preprocessing
- Test both CUDA and CPU paths

**When modifying text insertion:**
- Test insertion in terminals, text editors, and browsers
- Verify clipboard fallback works
- Check special character and Unicode handling

**When adding new features:**
- Add corresponding configuration options in `config/settings.rs`
- Update `config/default.toml` with sensible defaults
- Add validation in `Config::validate()`
- Consider simulation mode for testing without hardware

### Common Patterns

**Async initialization:**
```rust
pub async fn new() -> Result<Self> {
    // Components are initialized asynchronously
    // Use .await? for async operations
}
```

**Thread-safe state:**
```rust
// Recording state uses Arc<AtomicBool> for lock-free concurrency
let is_recording = Arc::new(AtomicBool::new(false));
is_recording.store(true, Ordering::SeqCst);
```

**Error handling:**
```rust
// Use anyhow::Result for error propagation
// Provide context with .context() or map_err()
audio_capture.start_recording()
    .context("Failed to start audio recording")?;
```

**Performance monitoring:**
```rust
let start = Instant::now();
// ... operation ...
let elapsed = start.elapsed();
info!("Operation took {:?}", elapsed);
```

## Documentation

Comprehensive project documentation is in `docs/`:
- `IMPLEMENTATION_PLAN.md` - Technical architecture and MVP components
- `TASK_TRACKER.md` - Development progress with 9 concrete tasks
- `DECISION_LOG.md` - All major technical decisions with rationale
- `CURRENT_SYSTEM_DESIGN.md` - Complete system design with diagrams
- `notes.md` - Original planning session notes

Always refer to these documents when:
- Understanding why a technical decision was made
- Checking the status of features
- Planning new work
- Understanding the overall architecture

## Special Notes

### Model Management
- Models are downloaded from HuggingFace Hub using the included scripts
- Use `./scripts/download-models.sh` for easy model setup
- Models are cached in `models/` directory
- The system validates model files before use

### X11 Integration
- All window management and text insertion requires X11
- Wayland support is not currently implemented
- The system detects focused windows for smart text insertion

### Audio Backends
- PipeWire is preferred for modern Linux systems
- ALSA provides fallback for older systems
- JACK is supported for professional audio setups
- The system automatically selects the best available backend

### Testing Without Hardware
- Set `simulated_mode: true` in audio capture for testing without microphone
- Whisper transcriber has built-in simulation mode for testing without models
- The MVP binary (`hush-mvp`) uses simulation mode by default

### Code Style
- Follow Rust conventions (rustfmt)
- Use structured logging with tracing crate
- Document public APIs with rustdoc comments
- Include error context for debugging
- Prefer explicit error handling over panics
