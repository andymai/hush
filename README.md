# 🤫 Hush - Voice-to-Text for Linux Developers

> **Fast, accurate, and private voice-to-text transcription for Linux developers**

Hush is a production-ready voice-to-text application built specifically for Linux developers. It uses OpenAI's Whisper models locally for accurate transcription without sending your voice data to any external services.

## ✨ Features

- 🎤 **Local Voice Transcription** - Uses Whisper models locally, no cloud dependencies
- ⌨️ **Multiple Input Modes** - Global hotkeys, manual mode, or one-shot recordings
- 🖥️ **Desktop Integration** - Native GNOME integration with custom keyboard shortcuts
- 🚀 **High Performance** - Optimized for development workflows
- 🔒 **Privacy First** - All processing happens locally on your machine
- 📱 **Desktop Notifications** - Visual feedback for recording and transcription status
- 🎯 **Smart Text Insertion** - Automatically inserts transcribed text at cursor position
- 🔧 **Multiple Model Sizes** - From tiny (39MB) to large (1.5GB) Whisper models

## 🚀 Quick Start

### Installation

```bash
# Clone the repository
git clone <repository-url>
cd hush

# Run the installation script
./install.sh
```

The installer will:
- Build Hush in release mode
- Install the binary to `~/.local/bin/hush`
- Set up desktop integration
- Download a basic Whisper model
- Create default configuration

### Basic Usage

After installation, you have several options:

#### Option 1: GNOME Keyboard Shortcut (Recommended)
1. Open **Settings → Keyboard → Keyboard Shortcuts**
2. Click **Custom Shortcuts** and add a new shortcut
3. **Name:** `Hush Voice-to-Text`
4. **Command:** `hush one-shot --duration 10`
5. **Shortcut:** `Ctrl+Shift+Space`

Now press `Ctrl+Shift+Space` anywhere to record for 10 seconds!

#### Option 2: Manual Mode (No Hotkeys)
```bash
hush manual
# Press Enter to start recording
# Press Enter again to stop and transcribe
# Type 'q' to quit
```

#### Option 3: One-Shot Mode
```bash
hush one-shot --duration 5  # Record for 5 seconds
hush one-shot --print-only  # Print to stdout instead of inserting
```

## 📋 All Commands

```bash
hush --help                    # Show all options
hush daemon                    # Run with global hotkeys (if supported)
hush manual                    # Manual recording mode
hush one-shot [OPTIONS]       # Single recording mode
hush status                    # Show system status
hush install                   # Install desktop integration
hush uninstall                # Remove desktop integration

# Options
--no-notifications            # Disable desktop notifications
-c, --config <CONFIG>         # Use custom configuration file
-v, --verbose                 # Increase logging verbosity
-vv                           # Even more verbose
```

## 🔧 Configuration

Hush uses a TOML configuration file located at `~/.config/hush/config.toml`:

```toml
[audio]
sample_rate = 16000
channels = 1
buffer_size = 1024
device = null  # Use default audio device

[transcription]
model_path = "models/whisper-tiny.bin"
model_size = "tiny"
language = "en"
use_cuda = false
beam_size = 5
no_speech_threshold = 0.6

[hotkey]
enabled = true
combination = "Ctrl+Shift+Space"

[feedback]
audio_enabled = true
start_sound = "assets/sounds/start.wav"
stop_sound = "assets/sounds/stop.wav"
error_sound = "assets/sounds/error.wav"
```

## 📊 Model Management

Hush supports multiple Whisper model sizes:

| Model | Size | Speed | Accuracy | Best For |
|-------|------|-------|----------|----------|
| tiny  | 39MB | Fastest | Basic | Quick notes, commands |
| small | 244MB | Fast | Good | General transcription |
| medium | 769MB | Medium | Better | Professional use |
| large | 1550MB | Slower | Best | High-accuracy needs |

### Download Models

```bash
# Download specific model
./scripts/download-models.sh tiny
./scripts/download-models.sh small
./scripts/download-models.sh medium

# List available models
./target/debug/simple-model-manager list

# Show cache info
./target/debug/simple-model-manager info
```

## 🛠️ Troubleshooting

### Global Hotkeys Don't Work

This is common on GNOME due to security restrictions. Solutions:

1. **Use GNOME Custom Shortcuts** (Recommended)
   - Follow the GNOME integration steps above
   - This works reliably on all GNOME systems

2. **Use Manual Mode**
   ```bash
   hush manual
   ```

3. **Check Permissions**
   ```bash
   hush status -v  # Show detailed system info
   ```

### Audio Issues

```bash
hush status  # Check audio device status

# List available audio devices
arecord -l

# Test audio recording
arecord -d 3 -f cd test.wav && aplay test.wav
```

### Build Issues

```bash
# Install required dependencies
sudo apt install libasound2-dev pkg-config

# Check build environment
./scripts/build.sh --bin hush-mvp
```

### Common Error Messages

- **"BadAccess" X11 Error**: Use GNOME shortcuts or manual mode
- **"No audio device"**: Check `arecord -l` and audio system
- **"Model not found"**: Run `./scripts/download-models.sh tiny`
- **"CUDA not available"**: Normal for most systems, falls back to CPU

## 🏗️ Development

### Building from Source

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install system dependencies
sudo apt install libasound2-dev pkg-config

# Clone and build
git clone <repository-url>
cd hush
./scripts/build.sh --bin hush-mvp

# Run tests (hardware-free!)
cargo test

# Run benchmarks
cargo bench

# Run integration tests (requires hardware)
cargo test --test integration_hardware_tests -- --ignored
```

### Architecture (v0.2.0)

Hush v0.2.0 features a **trait-based architecture** for:
- ✅ **Hardware-free testing** - Run full test suite without microphone/GPU
- ✅ **Platform independence** - Easy path to Wayland/Windows/macOS support
- ✅ **Runtime component swapping** - Choose backends dynamically
- ✅ **Dependency injection** - Testable, maintainable, extensible

**Key Components**:
- `AudioSource` - Audio capture abstraction (CPAL adapter + mocks)
- `Transcriber` - Speech-to-text abstraction (Whisper adapter + mocks)
- `TextOutput` - Text insertion abstraction (X11 adapter + mocks)
- `InputTrigger` - Input event abstraction (Hotkey adapter + mocks)

**Documentation**:
- 📖 **[Architecture Guide](docs/ARCHITECTURE.md)** - Complete architecture overview
- 📖 **[Migration Guide](docs/MIGRATION_GUIDE.md)** - Upgrade from v0.1.0 to v0.2.0
- 📖 **[ADRs](docs/architecture/adrs/)** - Architecture Decision Records
- 📖 **[Phase Reports](docs/architecture/)** - Detailed refactoring documentation

### Project Structure

```
hush/
├── src/
│   ├── main_mvp.rs          # Production CLI (v0.1.0 - legacy)
│   ├── bin/
│   │   └── hush-new.rs      # New trait-based binary (v0.2.0)
│   ├── core/                # NEW: Trait abstractions
│   │   ├── traits.rs        # AudioSource, Transcriber, TextOutput, InputTrigger
│   │   ├── error.rs         # Structured error handling
│   │   ├── state.rs         # Centralized state machine
│   │   └── mocks.rs         # Mock implementations for testing
│   ├── adapters/            # NEW: Adapters for existing components
│   │   ├── audio/           # CpalAudioAdapter
│   │   ├── transcription/   # WhisperAdapter
│   │   ├── text/            # X11TextAdapter
│   │   └── hotkey/          # HotkeyTriggerAdapter
│   ├── application/         # NEW: Application layer
│   │   ├── hush_app.rs      # HushApp with dependency injection
│   │   └── builder.rs       # HushAppBuilder (fluent API)
│   ├── audio/               # Legacy audio capture
│   ├── transcription/       # Legacy Whisper integration
│   ├── text/                # Legacy text insertion
│   ├── hotkey/              # Legacy hotkey management
│   └── config/              # Configuration management
├── tests/
│   ├── application_tests.rs          # NEW: 15 unit tests (hardware-free)
│   ├── architecture_poc_test.rs      # NEW: 12 POC tests
│   └── integration_hardware_tests.rs # NEW: 5 integration tests (hardware)
├── benches/
│   └── architecture_benchmarks.rs    # NEW: Performance benchmarks
├── docs/
│   ├── ARCHITECTURE.md               # NEW: Complete architecture guide
│   ├── MIGRATION_GUIDE.md            # NEW: v0.1.0 → v0.2.0 migration
│   └── architecture/
│       ├── adrs/                     # Architecture Decision Records
│       ├── DEPENDENCY_ANALYSIS.md    # Coupling analysis
│       ├── TRAIT_DESIGN.md           # Trait specifications
│       └── PHASE*.md                 # Refactoring phase reports
├── scripts/
│   ├── build.sh             # Build script with PKG_CONFIG_PATH fixes
│   └── download-models.sh   # Model management script
├── config/
│   └── default.toml         # Default configuration
└── models/                  # Downloaded Whisper models
```

### Testing Philosophy

**Unit Tests** (hardware-free, 27 tests):
```bash
cargo test  # Runs without microphone, GPU, X11, or models!
```

**Integration Tests** (real hardware, 5 suites):
```bash
cargo test --test integration_hardware_tests -- --ignored
```

**Benchmarks** (performance validation):
```bash
cargo bench
```

### Extending Hush

**Example: Add OpenAI Transcription Backend**

```rust
use hush::core::traits::{Transcriber, AudioBuffer, TranscriptionResult};
use async_trait::async_trait;

pub struct OpenAITranscriber {
    api_key: String,
}

#[async_trait]
impl Transcriber for OpenAITranscriber {
    async fn transcribe(&self, audio: &AudioBuffer) -> Result<TranscriptionResult> {
        // Call OpenAI Whisper API
    }

    fn info(&self) -> TranscriberInfo {
        TranscriberInfo {
            name: "OpenAI Whisper API".to_string(),
            // ...
        }
    }
}

// Use it immediately!
let app = HushAppBuilder::new()
    .with_transcriber(Box::new(OpenAITranscriber::new(api_key)))
    .build()?;
```

See **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#extensibility)** for more examples.

## 📈 Performance Tips

- **Use tiny model** for fast responses (development commands)
- **Use small/medium** for general transcription
- **Use large model** only when accuracy is critical
- **Enable CUDA** if you have compatible GPU (requires additional setup)
- **Adjust beam_size** in config (1=fastest, 20=most accurate)

## 🔒 Privacy & Security

- **No cloud dependencies** - All processing is local
- **No data collection** - Your voice never leaves your machine
- **Open source** - Audit the code yourself
- **Local models** - Whisper models stored and run locally

## 📝 License

[To be determined - specify your license here]

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## 📞 Support

- **Check system status**: `hush status -v`
- **View logs**: Run with `-v` or `-vv` flags
- **Report issues**: [Create an issue on GitHub]
- **Documentation**: This README and `hush --help`

---

**Happy transcribing! 🎙️✨**