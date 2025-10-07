# 🤫 Hush - Voice-to-Text for Linux Developers

> **Fast, accurate, and private voice-to-text transcription with universal app compatibility**

Hush is a production-ready voice-to-text application built specifically for Linux developers. It uses OpenAI's Whisper models locally for accurate transcription without sending your voice data to any external services.

## ✨ Key Features

- 🎤 **Local Voice Transcription** - Uses Whisper models locally, no cloud dependencies
- ⌨️ **Universal Text Insertion** - Works with **ALL applications** including VMs, password fields, games
- 🔒 **Privacy First** - All processing happens locally on your machine
- 🎯 **Hardware-Level Integration** - Linux UInput support for maximum compatibility
- 📱 **Smart Setup Tools** - Built-in diagnostics and setup assistance
- 🔧 **Multiple Model Sizes** - From tiny (39MB) to large (2.9GB) Whisper models

## 🚀 Quick Start

### 1. Setup UInput (Recommended)
For universal compatibility with all applications:

```bash
# Check if setup is needed
cargo run --bin hush -- diagnose-uinput

# Get setup instructions  
cargo run --bin hush -- setup-uinput
```

Quick setup (most systems):
```bash
sudo usermod -a -G input $USER
sudo modprobe uinput
echo 'uinput' | sudo tee /etc/modules-load.d/uinput.conf
# Log out and log back in
```

### 2. Download a Model
```bash
cargo run --bin simple-model-manager download base
```

### 3. Test Voice-to-Text
```bash
# Test text insertion first
cargo run --bin test-text-insertion-simple

# Test complete voice-to-text
cargo run --bin test-voice-to-text
```

## 📋 Available Applications

### Text Insertion Testing
- `test-text-insertion-simple` - Test text insertion without voice
- `test-working-uinput` - Comprehensive UInput keyboard testing
- `test-text-insertion` - Full text insertion test suite

### Voice-to-Text Testing  
- `test-voice-to-text` - Complete voice-to-text pipeline test
- `test-simple-whisper` - Whisper transcription testing

### Model Management
- `simple-model-manager` - Download and manage Whisper models

### Utilities
- `test-audio` - Audio capture testing
- `test-cpal-devices` - List available audio devices

## 🎯 Universal App Compatibility

With UInput integration, Hush works with:

✅ **All Desktop Apps** - Text editors, IDEs, browsers, chat apps  
✅ **Password Fields** - KeePass, password managers, login forms  
✅ **Virtual Machines** - VMware, VirtualBox, QEMU guests  
✅ **Games & Fullscreen** - Any application, including anti-cheat protected  
✅ **Terminal Applications** - SSH sessions, vim, nano, tmux  
✅ **Secure Contexts** - Lock screens, sudo prompts, elevated apps  
✅ **Cross-Platform** - Works on X11, Wayland, and console applications  

## 📊 Model Sizes

| Model  | Size     | Speed | Accuracy | Use Case |
|--------|----------|-------|----------|----------|
| Tiny   | ~39 MB   | Fastest | Basic | Quick testing, commands |
| Base   | ~142 MB  | Fast | Good | General use, recommended |
| Small  | ~466 MB  | Medium | Better | Higher accuracy needs |
| Medium | ~1.5 GB  | Slower | High | Professional use |
| Large  | ~2.9 GB  | Slowest | Highest | Maximum accuracy |

## 🛠️ Troubleshooting

### Text Insertion Not Working?
```bash
# Quick diagnosis
cargo run --bin hush -- diagnose-uinput

# Quick fix (temporary)
sudo modprobe uinput && sudo chmod 666 /dev/uinput
```

### Audio Issues?
```bash
# Test audio devices
cargo run --bin test-cpal-devices

# Test audio capture
cargo run --bin test-audio
```

### Need Models?
```bash
# List downloaded models
cargo run --bin simple-model-manager list

# Download base model (recommended)
cargo run --bin simple-model-manager download base
```

## 📚 Documentation

- **[UInput Setup Guide](docs/uinput-setup.md)** - Complete setup instructions
- **[Quick Reference](docs/uinput-quick-reference.md)** - Quick commands
- **[Testing Guide](TESTING_VOICE_TO_TEXT.md)** - How to test voice-to-text
- **[UInput Integration](UINPUT_INTEGRATION.md)** - Technical details

## 🏗️ Development

### Quick Test
```bash
# Clone and test immediately
git clone <repo-url>
cd hush
cargo run --bin test-text-insertion-simple
```

### Full Setup
```bash
# Install dependencies
sudo apt install libasound2-dev pkg-config xclip

# Set up UInput
cargo run --bin hush -- setup-uinput

# Download a model  
cargo run --bin simple-model-manager download base

# Test everything
cargo run --bin test-voice-to-text
```

---

**Ready to try voice-to-text that works everywhere?** Start with `cargo run --bin test-text-insertion-simple`!