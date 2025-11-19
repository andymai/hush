<div align="center">

# 🤫 Hush

**Fast, accurate, and private voice-to-text for Linux and macOS developers**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![CUDA](https://img.shields.io/badge/CUDA-12.0%2B-76B900.svg)](https://developer.nvidia.com/cuda-toolkit)
[![GitHub stars](https://img.shields.io/github/stars/andymai/hush?style=social)](https://github.com/andymai/hush/stargazers)
[![GitHub issues](https://img.shields.io/github/issues/andymai/hush)](https://github.com/andymai/hush/issues)

[Quick Start](#-quick-start) • [Features](#-features) • [How It Compares](#-how-hush-compares) • [Documentation](#-documentation) • [FAQ](#-frequently-asked-questions)

</div>

---

## 📋 Table of Contents

- [About](#-about)
- [Why Hush?](#-why-hush)
- [How Hush Compares](#-how-hush-compares)
- [Features](#-features)
- [Quick Start](#-quick-start)
- [Usage](#-usage-modes)
- [Real-World Use Cases](#-real-world-use-cases)
- [Configuration](#️-configuration)
- [Documentation](#-documentation)
- [System Requirements](#-system-requirements)
- [Performance](#-performance)
- [Troubleshooting](#-troubleshooting)
- [FAQ](#-frequently-asked-questions)
- [Development](#️-development)
- [Contributing](#-contributing)
- [Community & Support](#-community--support)
- [License](#-license)
- [Acknowledgments](#-acknowledgments)

---

## 🎯 About

Hush is a production-ready voice-to-text application built for Linux and macOS developers. It uses OpenAI's Whisper models locally with GPU acceleration (CUDA on Linux, Metal on Apple Silicon) for fast, accurate transcription, and features intelligent text processing with automatic filler word removal and optional LLM polishing.

**Privacy-first design:** All processing happens locally on your machine (except optional LLM integration). Your voice data never leaves your computer.

### 🌐 Platform Support

| Platform | Status | GPU Acceleration | Notes |
|----------|--------|------------------|-------|
| **Linux** | ✅ Fully Supported | CUDA (NVIDIA GPUs) | Primary platform, UInput integration |
| **macOS** | ✅ Fully Supported | Metal (Apple Silicon) | Native NSStatusBar, Accessibility API |
| **Windows** | 🚧 Planned | - | Not yet implemented |

#### macOS
- ✅ macOS 11.0 (Big Sur) and later
- ✅ Apple Silicon (M1, M2, M3, M4) with Metal GPU acceleration (~120ms latency)
- ✅ Intel Macs with CPU-only mode (~800ms latency)
- ✅ Native system tray integration via NSStatusBar
- ✅ Text insertion via Accessibility API and CGEvent
- ✅ See [macOS Installation Guide](docs/macos/INSTALL.md)

### 🎬 Demo

<div align="center">

<!-- TODO: Add demo GIF showing push-to-talk workflow -->
*Hold `Ctrl+Alt+V` → Speak → Release → Text appears instantly*

</div>

---

## 💡 Why Hush?

| Challenge | Hush's Solution |
|-----------|-----------------|
| **Cloud transcription = privacy concerns** | 100% local processing with Whisper models |
| **Slow CPU transcription** | CUDA GPU acceleration (up to 10x faster) |
| **Clipboard workarounds** | Hardware-level text insertion via UInput |
| **"Um, uh, like" in transcripts** | Intelligent filler word removal |
| **Raw speech → professional text** | Optional LLM polishing with Claude API |
| **Doesn't work in VMs/games** | Universal compatibility (X11, Wayland, SSH, VMs) |

---

## 📊 How Hush Compares

| Feature | Hush | whisper-writer | nerd-dictation | OpenWhispr | Handy |
|---------|------|----------------|----------------|------------|-------|
| **Language** | Rust | Python | Python | TypeScript | Rust/Tauri |
| **GPU Acceleration** | ✅ CUDA | ❌ | ❌ | Limited | ✅ |
| **Linux-Optimized** | ✅ UInput | ❌ | ✅ | ❌ | ❌ |
| **Voice Commands** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Works in VMs** | ✅ | ❌ | Limited | ❌ | ❌ |
| **Filler Removal** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **LLM Polish** | ✅ Claude | ❌ | ❌ | ✅ Multiple | ❌ |
| **Installation** | Build from source | pip install | Manual setup | npm install | Download binary |
| **Dependencies** | Minimal | Many | Few | Many | Minimal |
| **Works Everywhere** | ✅ VMs/Games/SSH | ❌ | Limited | ❌ | ❌ |

**Key Advantage:** Hush is the only solution that combines GPU acceleration, hardware-level UInput integration, voice commands, and intelligent text processing in a single, production-ready Rust application.

---

## ✨ Features

### Core Capabilities
- 🚀 **GPU-Accelerated** - CUDA support for lightning-fast transcription (10x faster)
- 🎤 **Local Voice Transcription** - Whisper models run entirely on your machine
- 🔒 **Privacy First** - Zero cloud dependencies, your voice stays private
- ⌨️ **Universal Text Insertion** - Works in ALL applications (VMs, terminals, games, browsers)
- 🎯 **Hardware-Level Integration** - Linux UInput for maximum compatibility

### Intelligent Features
- 🎧 **Intelligent Listening Mode** - Wispr Flow-style overlay with push-to-talk hotkey
- 🎨 **Visual Feedback** - Floating overlay with real-time amplitude monitoring
- ✨ **Smart Text Processing** - Automatic filler word removal ("um", "uh", "like")
- 🤖 **LLM Polishing** - Optional Claude API integration for professional output
- 🗣️ **Voice Commands** - "undo", "new paragraph", "new line", "cap that"
- 📝 **Undo Support** - Say "undo" to remove the last insertion

### Developer Features
- 🔧 **Multiple Model Sizes** - From tiny (75MB) to large (2.9GB)
- 📊 **Advanced Logging** - Structured logging with performance metrics
- ⚡ **Multiple Modes** - Intelligent listening, single recording, manual mode
- 🧪 **Comprehensive Testing** - Built-in test suite for all components

---

## 🚀 Quick Start

> **📖 For detailed installation instructions, troubleshooting, and all Linux distributions, see [INSTALL.md](INSTALL.md)**

### Prerequisites
```bash
# Ubuntu/Debian
sudo apt install libasound2-dev pkg-config libx11-dev libdbus-1-dev

# Fedora/RHEL
sudo dnf install pkg-config alsa-lib-devel libX11-devel dbus-devel

# Arch Linux
sudo pacman -S pkg-config alsa-lib libx11 dbus

# For GPU acceleration (optional but recommended)
# Install NVIDIA drivers 520.x+ and CUDA toolkit 12.0+
```

### Installation

**Automated Setup (Recommended):**
```bash
git clone https://github.com/andymai/hush.git
cd hush
./scripts/setup-dev.sh  # Installs dependencies and builds automatically
./hush models download base
./hush setup uinput --quick
```

**Quick Install (One-Liner):**
```bash
git clone https://github.com/andymai/hush.git && cd hush && make build && ./hush models download base && ./hush setup uinput --quick
```

**Step-by-Step (Manual):**
```bash
# 1. Clone the repository
git clone https://github.com/andymai/hush.git
cd hush

# 2. Build Hush
make build

# 3. Download a Whisper model
./hush models download base

# 4. Setup text insertion (required for automatic typing)
./hush setup uinput --quick

# 5. Test your setup
./hush status --full
```

> **💡 Tip:** For detailed instructions, all Linux distributions, and troubleshooting, see [INSTALL.md](INSTALL.md)

### First Run

```bash
# Start intelligent listening mode
./hush listen

# Hold Ctrl+Alt+V, speak, then release - your text appears!
```

**Optional:** Enable LLM text polishing for professional output:
```bash
echo "ANTHROPIC_API_KEY=your_api_key_here" > .env
./hush listen  # Will automatically use Claude for polishing
```

### ✅ Verify Installation

```bash
# Check system status
./hush status --full

# Test components
./hush test audio --duration 3
./hush test transcription
./hush test text-insertion

# All tests passing? You're ready!
./hush listen
```

---

## 🎮 Usage Modes

### 🎧 Intelligent Listening Mode (Recommended)

The most powerful mode - Wispr Flow-style push-to-talk with smart text processing.

```bash
./hush listen
```

**Features:**
- 🎯 Hold `Ctrl+Alt+V` to record, release to transcribe
- 🎨 Floating overlay with real-time amplitude feedback
- ✨ Automatic filler word removal
- 🤖 Optional LLM polishing for professional text
- 🗣️ Voice commands: "undo", "new paragraph", "cap that"

**Options:**
```bash
# Aggressive text editing for formal writing
./hush listen --editing-mode aggressive

# Disable text processing (raw transcription only)
./hush listen --no-processing

# Hide overlay button when idle
./hush listen --no-button
```

### 🎙️ Quick Recording

Single recording with automatic transcription and insertion.

```bash
# Record for 10 seconds
./hush record --duration 10

# Record but only print (don't insert)
./hush record --duration 5 --print-only

# Save audio file for debugging
./hush record --duration 3 --save-audio debug.wav
```

### 🔧 Manual Mode

Interactive mode for multiple recordings.

```bash
# Interactive manual recording
./hush manual

# Multiple recordings
./hush manual --count 3
```

---

## 💼 Real-World Use Cases

- **📝 Code Documentation** - Dictate docstrings, comments, and inline documentation
- **📧 Email & Communication** - Compose messages in Slack, email, or any text field
- **🐛 Bug Reports** - Describe issues and steps to reproduce while debugging
- **📖 Note Taking** - Capture thoughts during meetings or research sessions
- **💬 Commit Messages** - Dictate well-formatted git commit messages
- **📚 Documentation** - Write docs, READMEs, and technical content hands-free

---

## ⚙️ Configuration

### 🗣️ Voice Commands

Say these commands during or after transcription:

| Command | Aliases | Effect |
|---------|---------|--------|
| `new paragraph` | `next paragraph` | Insert double newline |
| `new line` | `next line` | Insert single newline |
| `undo` | `undo that` | Remove last text insertion |
| `delete that` | `scratch that` | Remove last text insertion |
| `cap that` | `capitalize that` | Capitalize preceding text |
| `all caps` | `upper case` | Convert to UPPERCASE |

### 🤖 LLM Integration (Optional)

Enable intelligent text polishing with Claude API:

```bash
# Create .env file in project directory
echo "ANTHROPIC_API_KEY=your_api_key_here" > .env
```

**Editing Modes:**
- **Light** - Minimal editing, preserves natural speech
- **Medium** (default) - Removes filler words, light polishing
- **Aggressive** - Heavy editing for professional, formal text

```bash
./hush listen --editing-mode aggressive
```

### ⚙️ System Management

```bash
# Check system status
./hush status --full

# List audio devices
./hush status --devices

# Show configuration
./hush status --config

# Install desktop integration
./hush install --desktop --autostart
```

---

## 🎯 Universal App Compatibility

Hush uses Linux UInput for hardware-level keyboard emulation, providing universal compatibility:

✅ **Desktop Applications** - Text editors, IDEs, browsers, chat apps
✅ **Terminal Applications** - SSH sessions, vim, nano, tmux
✅ **Virtual Machines** - VMware, VirtualBox, QEMU guests
✅ **Games & Fullscreen Apps** - Any application, including those with anti-cheat
✅ **Secure Contexts** - Lock screens, sudo prompts, elevated applications
✅ **Cross-Platform** - Works on X11, Wayland, and console applications

---

## 🚀 Performance

### Model Sizes & Speed

| Model  | Size    | CPU Speed | GPU Speed | Accuracy | Use Case |
|--------|---------|-----------|-----------|----------|-----------|
| Tiny   | 75 MB   | ~2-3s     | ~0.3s     | Basic    | Quick testing, commands |
| **Base**   | **145 MB**  | **~4-6s**     | **~0.5s**     | **Good**     | **Recommended for general use** |
| Small  | 466 MB  | ~8-12s    | ~0.8s     | Better   | Higher accuracy needs |
| Medium | 1.5 GB  | ~15-25s   | ~1.5s     | High     | Professional use |
| Large  | 2.9 GB  | ~30-45s   | ~2.5s     | Highest  | Maximum accuracy |

*Performance measured on RTX 4080 SUPER with 3-second audio clips*

**💡 Recommendation:** For interactive use, GPU acceleration is highly recommended (10x faster). For occasional use, CPU with tiny/base models works well.

### GPU Acceleration

**CUDA Support:** Hush automatically detects and uses NVIDIA GPUs.

**Requirements:**
- NVIDIA GPU (GTX 10-series or newer)
- CUDA toolkit 12.0+
- NVIDIA drivers 520.x or newer

**Benefits:**
- Up to 10x faster transcription
- Real-time processing for most models
- Reduced CPU usage

```bash
# Check GPU status
./hush status --full | grep -i cuda
# Look for: "use gpu = 1" and "CUDA0 total size"
```

---

## 📚 Documentation

### User Documentation
- **[INSTALL.md](INSTALL.md)** - Complete installation guide for Linux distributions
- **[macOS Installation](docs/macos/INSTALL.md)** - macOS installation and setup guide
- **[macOS Permissions](docs/macos/PERMISSIONS.md)** - macOS permission setup guide
- **[macOS Troubleshooting](docs/macos/TROUBLESHOOTING.md)** - macOS troubleshooting guide
- **[SETUP.md](SETUP.md)** - General setup and configuration guide
- **[CHANGELOG.md](CHANGELOG.md)** - Version history and release notes

### Developer & AI Agent Documentation
All developer documentation and project context is in the `.ai/knowledge/` directory:
- **[Architecture](.ai/knowledge/architecture.md)** - Complete architecture reference
- **[Conventions](.ai/knowledge/conventions.md)** - Coding standards and patterns
- **[Error Handling](.ai/knowledge/error-handling.md)** - Error handling guide
- **[Voice Commands](.ai/knowledge/voice-commands.md)** - Voice commands reference
- **[UInput Guide](.ai/knowledge/uinput-guide.md)** - UInput setup and reference
- **[ADR Summary](.ai/knowledge/adr-summary.md)** - Architecture decisions

### AI Coding Agent Protocol
- **[CLAUDE.md](CLAUDE.md)** - AI agent protocol and task management
- **[.ai/](.ai/)** - AI agent directory structure

---

## 💻 System Requirements

### Linux

#### Minimum Requirements (CPU-only)
- **OS:** Linux (Ubuntu 20.04+, Fedora 35+, or equivalent)
- **CPU:** x86_64 with AVX2 support
- **RAM:** 4GB+ (8GB+ for larger models)
- **Storage:** 1GB free space for models
- **Audio:** Microphone or audio input device
- **Rust:** 1.70+

#### Recommended (GPU-accelerated)
- **GPU:** NVIDIA GPU with CUDA support (GTX 1060 or newer)
- **VRAM:** 4GB+ for base model, 8GB+ for large models
- **CUDA:** CUDA Toolkit 12.0+
- **Drivers:** NVIDIA 520.x or newer

### macOS

#### Minimum Requirements
- **OS:** macOS 11.0 (Big Sur) or later
- **CPU:** Apple Silicon or Intel (x86_64)
- **RAM:** 8GB+ (16GB+ recommended)
- **Storage:** 2GB free space for models
- **Permissions:** Accessibility and Microphone access

#### Recommended
- **CPU:** Apple Silicon (M1, M2, M3, M4) for Metal GPU acceleration
- **OS:** macOS 13.0 (Ventura) or later
- **RAM:** 16GB+

**See:** [macOS Installation Guide](docs/macos/INSTALL.md)

### Build Options

**GPU-accelerated (default):**
```bash
cargo build --release
# Requires: CUDA toolkit, NVIDIA GPU
# Performance: ~0.5s for 3s audio (base model)
```

**CPU-only:**
```bash
cargo build --release --no-default-features --features notifications,system-tray
# No CUDA required - works in Docker, Codespaces, CPU-only systems
# Performance: ~4-6s for 3s audio (base model)
```

---

## 🔧 Troubleshooting

### Text Insertion Not Working?

```bash
# Comprehensive diagnosis
./hush setup diagnose-uinput

# Quick fix (temporary)
sudo modprobe uinput && sudo chmod 666 /dev/uinput

# Permanent fix
./hush setup uinput --auto-fix

# Test text insertion
./hush test text-insertion
```

### Audio Issues?

```bash
# List available devices
./hush status --devices

# Test audio capture
./hush test audio --duration 3

# Test specific device
./hush test audio --device "pulse" --duration 2
```

### GPU Not Working?

```bash
# Check CUDA status
nvidia-smi

# Check if Hush detects GPU
./hush status --full
# Look for GPU information in the output

# If GPU is not detected, Hush will automatically fall back to CPU
# For CPU-only builds, rebuild without CUDA:
cargo build --release --no-default-features --features notifications,system-tray

# Test transcription performance
./hush test transcription --timing
```

### More Help

- Check the [UInput Setup Guide](docs/uinput-setup.md) for detailed troubleshooting
- Enable verbose logging: `./hush -vv record --duration 3`
- View logs: `tail -f ~/.local/share/hush/logs/*.log`

---

## ❓ Frequently Asked Questions

**Q: Does Hush work on Wayland?**
A: Yes! Hush uses UInput which works on both X11 and Wayland display servers.

**Q: Do I need a GPU?**
A: No, but GPU acceleration provides 10x faster transcription (0.5s vs 5s for typical dictation). Hush works fine on CPU with smaller models (tiny/base).

**Q: How is this different from cloud dictation services?**
A: Your voice never leaves your computer. Everything runs locally for complete privacy. No internet connection required (except for optional LLM polishing).

**Q: Can I use this for programming?**
A: Absolutely! Many developers use Hush for writing docstrings, commit messages, code comments, documentation, and bug reports.

**Q: Does it work in virtual machines?**
A: Yes! UInput hardware-level integration allows Hush to work in VMs, games, SSH sessions, and secure contexts where clipboard-based solutions fail.

**Q: What languages are supported?**
A: Whisper supports 99+ languages including English, Spanish, French, German, Chinese, Japanese, and more. See the [Whisper documentation](https://github.com/openai/whisper#available-models-and-languages) for the full list.

**Q: Why Rust instead of Python?**
A: Rust provides memory safety, native performance, instant startup, and a single binary with no dependency conflicts. No Python environment needed, no version issues.

**Q: Can I use multiple Whisper models?**
A: Yes! Download multiple models with `./hush models download <model>` and switch between them in your configuration.

**Q: Does it require an internet connection?**
A: No for transcription (100% local). Yes only if you enable optional LLM polishing with Claude API.

**Q: How accurate is it compared to commercial services?**
A: Whisper models match or exceed commercial accuracy. The base model handles most use cases well, while larger models rival professional transcription services.

---

## 🛠️ Development

### Building from Source

```bash
# Clone repository
git clone https://github.com/andymai/hush.git
cd hush

# Option 1: Automated setup (installs dependencies and builds)
./scripts/setup-dev.sh

# Option 2: Manual installation
# Install dependencies (see INSTALL.md for other distributions)
sudo apt install build-essential pkg-config libasound2-dev libx11-dev libdbus-1-dev

# Build debug version
make build

# Build optimized release version
make release

# Run tests
cargo test

# Check code quality
cargo clippy
cargo fmt
```

**Build without CUDA** (CPU-only, for systems without GPU):
```bash
cargo build --release --no-default-features --features notifications,system-tray
# Useful for: Docker containers, GitHub Codespaces, cloud VMs, CPU-only systems
```

**Build without notifications** (avoids D-Bus dependency):
```bash
cargo build --release --no-default-features
```

For detailed build instructions and troubleshooting, see **[INSTALL.md](INSTALL.md)**.

### Project Structure

```
hush/
├── src/
│   ├── adapters/         # External integrations (audio, text, hotkeys)
│   ├── application/      # Application orchestration
│   ├── audio/            # Audio capture and feedback
│   ├── cli/              # Command-line interface
│   ├── core/             # Core traits and types
│   ├── overlay/          # Visual overlay UI
│   ├── text_processing/  # Intelligent text processing
│   ├── transcription/    # Whisper integration
│   └── ...
├── docs/                 # Documentation
├── models/               # Whisper model storage
└── tests/                # Integration tests
```

### Testing

```bash
# Test individual components
./hush test audio --duration 3
./hush test transcription --file audio.wav
./hush test text-insertion

# Full pipeline test
./hush test pipeline

# Run benchmarks
cargo bench
```

---

## 🤝 Contributing

We welcome contributions! Whether it's:

- 🐛 Bug reports
- 💡 Feature requests
- 📝 Documentation improvements
- 🔧 Code contributions

### How to Contribute

1. **Fork the repository**
2. **Create a feature branch** (`git checkout -b feature/amazing-feature`)
3. **Make your changes**
4. **Run tests** (`cargo test`)
5. **Check formatting** (`cargo fmt && cargo clippy`)
6. **Commit your changes** (`git commit -m 'Add amazing feature'`)
7. **Push to branch** (`git push origin feature/amazing-feature`)
8. **Open a Pull Request**

### Development Guidelines

- Follow Rust best practices and idioms
- Add tests for new features
- Update documentation as needed
- Keep commits focused and descriptive
- Ensure all tests pass before submitting PR

### Code of Conduct

Be respectful, inclusive, and constructive. We're all here to build something useful together.

---

## 👥 Community & Support

- 💬 **[Discussions](https://github.com/andymai/hush/discussions)** - Ask questions, share tips, and connect with other users
- 🐛 **[Issue Tracker](https://github.com/andymai/hush/issues)** - Report bugs and request features
- 🌟 **[Show & Tell](https://github.com/andymai/hush/discussions/categories/show-and-tell)** - Share your workflows and use cases

### Getting Help

1. Check the [Troubleshooting](#-troubleshooting) section and [FAQ](#-frequently-asked-questions)
2. Search [existing issues](https://github.com/andymai/hush/issues) and [discussions](https://github.com/andymai/hush/discussions)
3. Enable verbose logging to gather diagnostic information:
   ```bash
   ./hush -vv listen
   ```
4. Open a [new issue](https://github.com/andymai/hush/issues/new) with:
   - System info from `./hush status --full`
   - Relevant log excerpts
   - Steps to reproduce

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## 🙏 Acknowledgments

- **[OpenAI Whisper](https://github.com/openai/whisper)** - State-of-the-art speech recognition models
- **[candle](https://github.com/huggingface/candle)** - Minimalist ML framework for Rust
- **[whisper-rs](https://github.com/tazz4843/whisper-rs)** - Rust bindings for Whisper
- **[Anthropic Claude](https://www.anthropic.com/claude)** - Optional LLM for text polishing
- **The Rust Community** - For excellent tools and libraries

---

<div align="center">

**Ready to experience intelligent voice-to-text?**

```bash
make build
./hush models download base
./hush setup uinput --quick
./hush listen
```

**Privacy-first • GPU-accelerated • Works everywhere**

[⬆ Back to Top](#-hush)

</div>
