# 🤫 Hush - Voice-to-Text for Linux Developers

> **Fast, accurate, and private voice-to-text with GPU acceleration, intelligent text processing, and universal app compatibility**

Hush is a production-ready voice-to-text application built specifically for Linux developers. It uses OpenAI's Whisper models locally with CUDA GPU acceleration for fast, accurate transcription, and features intelligent text processing with automatic filler word removal and optional LLM polishing. All processing happens locally (except optional LLM), ensuring your voice data never leaves your machine.

## ✨ Key Features

- 🚀 **GPU-Accelerated** - CUDA support for lightning-fast transcription on NVIDIA GPUs
- 🎤 **Local Voice Transcription** - Uses Whisper models locally, no cloud dependencies
- ⌨️ **Universal Text Insertion** - Works with **ALL applications** including VMs, password fields, games
- 🔒 **Privacy First** - All processing happens locally on your machine
- 🎯 **Hardware-Level Integration** - Linux UInput support for maximum compatibility
- 🖥️ **Intelligent Listening Mode** - Wispr Flow-style overlay with push-to-talk hotkey
- 🎨 **Visual Feedback** - Floating overlay with real-time amplitude monitoring
- ✨ **Intelligent Text Processing** - Automatic filler word removal and LLM-based polishing
- 🗣️ **Voice Commands** - "undo", "new paragraph", "new line", and more
- 📊 **Advanced Logging** - Structured logging with performance metrics and request tracing
- 🔧 **Multiple Model Sizes** - From tiny (75MB) to large (2.9GB) Whisper models
- ⚡ **Multiple Modes** - Intelligent listening, single recording, manual mode

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

### 5. Start Using Hush 🎉
```bash
# Start intelligent listening mode (recommended)
./hush listen

# Or do a quick single recording
./hush record --duration 10

# Or use manual mode for multiple recordings
./hush manual
```

**Pro tip:** Hold `Ctrl+Alt+V` in listen mode to record, release to transcribe and insert!

## 🎮 Usage Modes

### 🎧 Intelligent Listening Mode (Recommended)
Wispr Flow-style push-to-talk with intelligent text processing:
```bash
# Start intelligent listening mode with overlay
./hush listen

# With custom editing mode (light, medium, aggressive)
./hush listen --editing-mode aggressive

# Disable text processing (raw transcription only)
./hush listen --no-processing

# Hide overlay button when idle
./hush listen --no-button
```

**Features:**
- 🎯 **Push-to-talk**: Hold `Ctrl+Alt+V` to record, release to transcribe
- 🎨 **Visual overlay**: Floating window with real-time amplitude feedback
- ✨ **Smart processing**: Automatic filler word removal (um, uh, like, etc.)
- 🤖 **LLM polishing**: Optional Claude API integration for professional text
- 🗣️ **Voice commands**: Say "undo", "new paragraph", "new line", "cap that", etc.
- 📝 **Undo support**: Say "undo" to remove the last insertion

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

## ⚙️ Configuration

### 🤖 Optional LLM Integration
Enable intelligent text polishing with Claude API (optional):

```bash
# Create .env file in project directory
echo "ANTHROPIC_API_KEY=your_api_key_here" > .env

# Then use listen mode - it will automatically detect and use the API
./hush listen
```

**Text Processing Modes:**
- **Light**: Minimal editing, preserves your natural speech
- **Medium** (default): Balanced - removes filler words, light polishing
- **Aggressive**: Heavy editing for professional, formal text

**Note:** LLM integration is completely optional. Without an API key, Hush uses fast rule-based processing.

### 🗣️ Voice Commands Reference

Say these commands during or after transcription:

| Command | Aliases | Effect |
|---------|---------|--------|
| `new paragraph` | `next paragraph` | Insert double newline |
| `new line` | `next line` | Insert single newline |
| `undo` | `undo that` | Remove last text insertion |
| `delete that` | `scratch that` | Remove last text insertion |
| `cap that` | `capitalize that` | Capitalize preceding text |
| `all caps` | `upper case` | Convert preceding text to UPPERCASE |

**Example:** Say "Hello world new paragraph this is a test" to insert:
```
Hello world

this is a test
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

### 🎨 Overlay UI

The intelligent listening mode includes a floating overlay window:

**Visual States:**
- **Idle**: Tiny button (10×20px) at bottom-center (optional, use `--no-button` to hide)
- **Recording**: Expanded bar (80×20px) with real-time amplitude visualization
- **Processing**: Processing indicator while transcribing
- **Success/Error**: Brief feedback message before returning to idle

**Customization:**
- Position: Bottom-center by default
- Auto-hide: Success/error messages automatically fade
- Minimal: Designed to stay out of your way

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
- 🎧 **Intelligent Listening Mode** - Wispr Flow-style overlay with push-to-talk (Ctrl+Alt+V)
- 🎨 **Visual Overlay** - Floating window with real-time amplitude feedback
- ✨ **Smart Text Processing** - Automatic filler word removal and LLM polishing
- 🗣️ **Voice Commands** - Undo, formatting, and text manipulation via voice
- 📝 **Undo Support** - Remove last insertion with "undo" command
- 🤖 **LLM Integration** - Optional Claude API for professional text polishing
- 🚀 **CUDA GPU Acceleration** - Up to 10x faster transcription
- 📊 **Structured Logging** - Advanced debugging and performance monitoring
- 🔧 **Automated Setup** - One-command UInput configuration

---

**Ready to experience intelligent voice-to-text?**

```bash
# Build and setup
make build
./hush models download base
./hush setup uinput --quick

# Optional: Add Claude API for text polishing
echo "ANTHROPIC_API_KEY=your_key" > .env

# Start intelligent listening mode
./hush listen
```

✨ **Works everywhere. Processes intelligently. Accelerated by GPU.**
