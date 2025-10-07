# 🤫 Hush - Voice-to-Text for Linux Developers

> **Fast, accurate, and private voice-to-text transcription with GPU acceleration and universal app compatibility**

Hush is a production-ready voice-to-text application built specifically for Linux developers. It uses OpenAI's Whisper models locally with CUDA GPU acceleration for fast, accurate transcription without sending your voice data to any external services.

## ✨ Key Features

- 🚀 **GPU-Accelerated** - CUDA support for lightning-fast transcription on NVIDIA GPUs
- 🎤 **Local Voice Transcription** - Uses Whisper models locally, no cloud dependencies
- ⌨️ **Universal Text Insertion** - Works with **ALL applications** including VMs, password fields, games
- 🔒 **Privacy First** - All processing happens locally on your machine
- 🎯 **Hardware-Level Integration** - Linux UInput support for maximum compatibility
- 🖥️ **Modern CLI Interface** - Comprehensive command-line interface with TUI support
- 📊 **Advanced Logging** - Structured logging with performance metrics and request tracing
- 🔧 **Multiple Model Sizes** - From tiny (75MB) to large (2.9GB) Whisper models
- ⚡ **Multiple Modes** - Interactive TUI, single recording, hotkey support

## 🚀 Quick Start

### 1. Build Hush
```bash
# Install system dependencies
sudo apt install libasound2-dev pkg-config

# Build with GPU support
make build
```

### 2. Download Models
```bash
# Download recommended base model
./hush models download base

# Or start with tiny model for testing
./hush models download tiny

# List available models
./hush models list
```

### 3. Setup Text Insertion (Recommended)
For universal compatibility with all applications:

```bash
# Quick setup guide
./hush setup uinput --quick

# Diagnose any issues
./hush setup diagnose-uinput

# Automated fix (requires sudo)
./hush setup uinput --auto-fix
```

### 4. Test Everything
```bash
# Test your setup
./hush status --full

# Test audio capture
./hush test audio --duration 3

# Test transcription
./hush test transcription --file audio.wav

# Test text insertion
./hush test text-insertion

# Full pipeline test
./hush test pipeline
```

## 🎮 Usage Modes

### 🎤 Start Voice-to-Text
```bash
# Interactive TUI mode (recommended)
./hush start

# CLI/hotkey mode 
./hush start --cli

# Background daemon mode
./hush start --daemon
```

### 🎧 Quick Recording
```bash
# Single recording (auto-transcribe and insert)
./hush record --duration 10

# Record but don't insert text
./hush record --duration 5 --print-only

# Save audio for debugging
./hush record --duration 3 --save-audio debug.wav
```

### 🔧 Manual Mode
```bash
# Interactive manual recording
./hush manual

# Multiple recordings
./hush manual --count 3
```

### ⚙️ System Management
```bash
# Install desktop integration
./hush install --desktop --autostart

# Check system status  
./hush status --full

# Show configuration
./hush status --config

# List audio devices
./hush status --devices
```

## 🎯 Universal App Compatibility

With UInput integration, Hush works with:

✅ **All Desktop Apps** - Text editors, IDEs, browsers, chat apps  
✅ **Password Fields** - KeePass, password managers, login forms  
✅ **Virtual Machines** - VMware, VirtualBox, QEMU guests  
✅ **Games & Fullscreen** - Any application, including anti-cheat protected  
✅ **Terminal Applications** - SSH sessions, vim, nano, tmux  
✅ **Secure Contexts** - Lock screens, sudo prompts, elevated apps  
✅ **Cross-Platform** - Works on X11, Wayland, and console applications  

## 🚀 GPU Acceleration

**CUDA Support:** Hush automatically detects and uses NVIDIA GPUs for significantly faster transcription.

### Requirements
- NVIDIA GPU (GTX 10-series or newer recommended)
- CUDA toolkit 12.0+ installed
- NVIDIA drivers 520.x or newer

### Performance Benefits
- **Up to 10x faster** transcription on supported GPUs
- **Real-time processing** for most model sizes
- **Reduced CPU usage** for better system responsiveness

```bash
# Check GPU status
./hush status --full
# Look for: "CUDA0 total size = XX MB" in model loading
```

## 📊 Model Sizes & Performance

| Model  | Size    | CPU Speed | GPU Speed | Accuracy | Use Case |
|--------|---------|-----------|-----------|----------|-----------|
| Tiny   | 75 MB   | ~2-3s     | ~0.3s     | Basic    | Quick testing, commands |
| Base   | 145 MB  | ~4-6s     | ~0.5s     | Good     | **Recommended** general use |
| Small  | 466 MB  | ~8-12s    | ~0.8s     | Better   | Higher accuracy needs |
| Medium | 1.5 GB  | ~15-25s   | ~1.5s     | High     | Professional use |
| Large  | 2.9 GB  | ~30-45s   | ~2.5s     | Highest  | Maximum accuracy |

*Performance measured on RTX 4080 SUPER with 3-second audio clips*

## 🔧 Troubleshooting

### ⌨️ Text Insertion Not Working?
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

### 🎤 Audio Issues?
```bash
# List available devices
./hush status --devices

# Test audio capture
./hush test audio --duration 3 --list-devices

# Test specific device
./hush test audio --device "pulse" --duration 2
```

### 🧠 Model Issues?
```bash
# List downloaded models
./hush models list

# Download missing models
./hush models download base

# Verify model integrity
./hush models verify

# Test transcription
./hush test transcription --all-models
```

### 🚀 GPU Not Working?
```bash
# Check CUDA status
nvidia-smi

# Check if Hush detects GPU
./hush status --full | grep -i cuda

# Expected output: "use gpu = 1" and "CUDA0 total size"

# Test with/without GPU
./hush test transcription --timing
```

### 📊 Performance Issues?
```bash
# Check system resources
./hush status --full

# Enable verbose logging
./hush -vv record --duration 3 --print-only

# Check logs
tail -f ~/.local/share/hush/logs/*.log
```

## 🆆 Advanced Features

### 📊 Comprehensive Logging
```bash
# Enable different logging levels
./hush -v status          # Verbose
./hush -vv record         # Debug  
./hush -vvv test pipeline # Trace

# View structured logs
tail -f ~/.local/share/hush/logs/*.log

# Log analysis
grep "error" ~/.local/share/hush/logs/*.log
grep "performance" ~/.local/share/hush/logs/*.log
```

### ⚙️ Configuration
```bash
# Custom config file
./hush -c ~/.config/hush/custom.toml status

# Show current configuration
./hush status --config

# Disable notifications
./hush --no-notifications start
```

### 📊 Performance Monitoring
Hush includes built-in performance tracking:
- **Session correlation** - Track operations across the entire pipeline
- **Request tracing** - Follow audio from capture to text insertion
- **System metrics** - CPU usage, memory usage, processing times
- **GPU utilization** - Monitor CUDA performance

### 🗺 Component Testing
```bash
# Test individual components
./hush test audio --save audio_test.wav
./hush test transcription --file audio_test.wav --timing
./hush test text-insertion --text "Hello World"
./hush test hotkeys --combination "F11" --duration 10

# Comprehensive system test
./hush test all --benchmarks --output results.json
```

## 📚 Documentation

- **[Testing Guide](TESTING_VOICE_TO_TEXT.md)** - Complete testing instructions
- **[UInput Integration](UINPUT_INTEGRATION.md)** - Technical implementation details
- **[Simple Whisper Integration](SIMPLE_WHISPER_INTEGRATION.md)** - Model integration guide
- **[Architecture Documentation](docs/architecture/)** - Design patterns and ADRs

## 🏗️ Development

### Quick Start
```bash
# Clone and build
git clone <repo-url>
cd hush
make build

# Download a model and test
./hush models download tiny
./hush status --full
```

### Development Setup
```bash
# Install dependencies
sudo apt install libasound2-dev pkg-config

# Build with full features
make build

# Run tests
cargo test

# Check code quality
cargo clippy
cargo fmt
```

### Building from Source
```bash
# Debug build
make build

# Release build (optimized)
make release

# Clean build artifacts
make clean
```

## 💻 System Requirements

### Minimum Requirements
- **OS:** Ubuntu 20.04+ (or compatible Linux distribution)
- **RAM:** 4GB (8GB recommended)
- **Storage:** 1GB free space for models
- **Audio:** Microphone or audio input device

### Recommended for GPU Acceleration
- **GPU:** NVIDIA GTX 1060 or newer
- **VRAM:** 4GB+ (for larger models)
- **CUDA:** 12.0+ toolkit
- **Drivers:** NVIDIA 520.x or newer

### Dependencies
```bash
# Ubuntu/Debian
sudo apt install libasound2-dev pkg-config

# For GPU support, install CUDA toolkit
# Follow NVIDIA's official CUDA installation guide
```

## 🎆 What's New

**Latest features:**
- 🚀 **CUDA GPU Acceleration** - Up to 10x faster transcription
- 📊 **Structured Logging** - Advanced debugging and performance monitoring
- 🖥️ **Modern CLI** - Comprehensive command interface with TUI support
- ⚡ **Real-time Processing** - GPU-accelerated real-time transcription
- 🔧 **Automated Setup** - One-command UInput configuration

---

**Ready to experience lightning-fast voice-to-text?** 

```bash
make build
./hush models download base
./hush record --duration 10
```

✨ **Works everywhere. Processes locally. Accelerated by GPU.**
