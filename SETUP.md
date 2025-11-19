# Hush Setup Guide

Complete installation and setup instructions for Hush voice-to-text.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [System Configuration](#system-configuration)
- [Verification](#verification)
- [Troubleshooting](#troubleshooting)

---

## Prerequisites

### System Requirements

**Operating System:**
- Linux (Ubuntu 20.04+, Debian 11+, Fedora 35+, Arch Linux, or similar)
- Kernel 2.6+ (for UInput support)

**Hardware:**
- Microphone (any USB or built-in microphone)
- NVIDIA GPU with CUDA support (recommended, but not required)
- 4GB RAM minimum, 8GB+ recommended
- 2GB disk space for Whisper models

**Software:**
- Rust 1.70+
- CUDA Toolkit 12.0+ (optional, for GPU acceleration)
- X11 or Wayland display server

### Check Your System

```bash
# Check Rust version
rustc --version  # Should be 1.70+

# Check if CUDA is available (optional)
nvidia-smi

# Check display server
echo $XDG_SESSION_TYPE  # Should show x11 or wayland

# Check if uinput module is available
lsmod | grep uinput
```

---

## Installation

### 1. Clone the Repository

```bash
git clone https://github.com/andymai/hush.git
cd hush
```

### 2. Build from Source

**For CPU-only (no NVIDIA GPU):**
```bash
cargo build --release
```

**For CUDA/GPU acceleration:**
```bash
# Ensure CUDA toolkit is installed first
# Ubuntu/Debian:
sudo apt install nvidia-cuda-toolkit

# Fedora:
sudo dnf install cuda

# Then build with CUDA support
cargo build --release --features cuda
```

### 3. Install Binary

```bash
# Option 1: Copy to user binaries
mkdir -p ~/.local/bin
cp target/release/hush ~/.local/bin/
# Ensure ~/.local/bin is in your PATH

# Option 2: System-wide installation (requires sudo)
sudo cp target/release/hush /usr/local/bin/

# Verify installation
hush --version
```

---

## System Configuration

### 1. UInput Setup (Required for Best Experience)

UInput provides kernel-level keyboard emulation for universal text insertion.

**Quick Setup (Recommended):**
```bash
# Add yourself to input group
sudo usermod -a -G input $USER

# Load uinput module
sudo modprobe uinput

# Make uinput load at boot
echo 'uinput' | sudo tee /etc/modules-load.d/uinput.conf

# Log out and log back in for group changes to take effect
```

**Verify UInput:**
```bash
# Check if you're in input group
groups | grep input

# Check if uinput device exists
ls -la /dev/uinput

# Should show: crw-rw---- 1 root input ...
```

**Alternative: Temporary setup (testing only):**
```bash
# This only works until reboot
sudo chmod 666 /dev/uinput
```

### 2. Download Whisper Models

```bash
# Create models directory
mkdir -p ~/.local/share/hush/models

# Download base model (recommended, ~140MB)
# Option 1: Use Hush's built-in downloader (if available)
hush models download base

# Option 2: Manual download
cd ~/.local/share/hush/models
wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin

# Other model options:
# - tiny: Fastest, least accurate (~75MB)
# - small: Balanced (~465MB)
# - medium: More accurate (~1.5GB)
# - large: Most accurate, slowest (~3GB)
```

### 3. Audio Device Configuration

```bash
# List available audio devices
hush status --audio

# Test audio capture
hush test audio

# If you have multiple microphones, specify in config:
mkdir -p ~/.config/hush
cat > ~/.config/hush/config.toml << EOF
[audio]
device = "your_device_name"  # Or "default"
sample_rate = 16000
EOF
```

### 4. Hotkey Configuration

```bash
# Default hotkey: Ctrl+Alt+V
# To customize, edit config:
cat >> ~/.config/hush/config.toml << EOF
[hotkey]
trigger = "Ctrl+Shift+Space"  # Your preferred combination
EOF
```

### 5. Optional: LLM Integration

For intelligent text polishing (optional):

```bash
# Set up Anthropic API key
echo 'ANTHROPIC_API_KEY=your_api_key_here' >> ~/.env

# Or set in config
cat >> ~/.config/hush/config.toml << EOF
[text_processing]
use_llm_polishing = true
editing_mode = "medium"  # light, medium, or aggressive
EOF
```

---

## Verification

### Test Each Component

```bash
# 1. Test audio capture
hush test audio
# Should record 5 seconds and show audio levels

# 2. Test transcription
hush record --duration 5
# Speak during recording, should display transcribed text

# 3. Test text insertion
hush test text-insertion
# Should type test text into focused window

# 4. Test full pipeline
hush manual
# Interactive mode - record and insert multiple times

# 5. Check system status
hush status --full
# Shows all component status
```

### Verify Installation Checklist

- [ ] `hush --version` shows version number
- [ ] `hush status --full` shows all components as "ready"
- [ ] UInput device accessible (`ls -la /dev/uinput`)
- [ ] Whisper model downloaded and detected
- [ ] Audio device detected and working
- [ ] Test recording produces transcription
- [ ] Text insertion works in a text editor

---

## Troubleshooting

### Common Issues

#### "No microphone detected"

**Solution:**
```bash
# List audio devices
arecord -l

# Test microphone directly
arecord -d 5 test.wav
aplay test.wav

# Check Hush audio status
hush status --audio
```

#### "Permission denied: /dev/uinput"

**Solution:**
```bash
# Check current permissions
ls -la /dev/uinput

# Add user to input group
sudo usermod -a -G input $USER

# Load uinput module if missing
sudo modprobe uinput

# Log out and log back in
```

#### "Model not found"

**Solution:**
```bash
# Check model directory
ls ~/.local/share/hush/models/

# Download model
hush models download base

# Or specify custom model path in config
cat >> ~/.config/hush/config.toml << EOF
[transcription]
model_path = "/path/to/your/model.bin"
EOF
```

#### "CUDA not available" (when you have NVIDIA GPU)

**Solution:**
```bash
# Check CUDA installation
nvidia-smi

# Verify CUDA toolkit
nvcc --version

# Rebuild with CUDA support
cargo clean
cargo build --release --features cuda

# Hush will auto-detect and use CUDA
```

#### "Text insertion not working"

**Solution:**
```bash
# Check UInput status
hush diagnose-uinput

# Verify you're in input group
groups | grep input

# Test with different applications
# Try: text editor, terminal, browser

# Check logs for insertion method used
RUST_LOG=debug hush listen
```

#### "Hotkey not registering"

**Solution:**
```bash
# Check if hotkey is already in use
# Try a different combination

# Edit config
nano ~/.config/hush/config.toml
# Change [hotkey] trigger

# Test in manual mode first
hush manual
```

### Getting Help

If you encounter issues not covered here:

1. **Check logs:**
   ```bash
   RUST_LOG=debug hush listen
   ```

2. **Run diagnostics:**
   ```bash
   hush status --full
   hush diagnose-uinput
   ```

3. **Check GitHub issues:**
   https://github.com/andymai/hush/issues

4. **Create a new issue with:**
   - Output of `hush status --full`
   - Your Linux distribution and version
   - Error messages or logs
   - Steps to reproduce

---

## Quick Start

Once installation is complete:

```bash
# Start listening mode with push-to-talk
hush listen

# Hold Ctrl+Alt+V, speak, release
# Your speech will be transcribed and inserted as text

# Or use manual mode for interactive testing
hush manual
```

For more usage information, see the main [README.md](README.md).

---

## Advanced Configuration

### Full Configuration File

Create `~/.config/hush/config.toml`:

```toml
[audio]
device = "default"
sample_rate = 16000

[transcription]
model = "base"           # tiny, base, small, medium, large
language = "en"
use_gpu = true           # Auto-detected if CUDA available

[hotkey]
trigger = "Ctrl+Alt+V"

[text_output]
method = "uinput"        # uinput, x11, clipboard
typing_delay_ms = 10     # Delay between keystrokes

[text_processing]
remove_filler_words = true
use_llm_polishing = false
editing_mode = "medium"  # light, medium, aggressive

[overlay]
enabled = true
show_recording_indicator = true
```

### Desktop Integration

```bash
# Install desktop entry
hush install

# Enable autostart
hush install --autostart

# Create system tray icon
# (Requires X11/Wayland session)
```

### Environment Variables

```bash
# Override config location
export HUSH_CONFIG_PATH=~/.config/hush/config.toml

# Set Anthropic API key for LLM features
export ANTHROPIC_API_KEY=your_key_here

# Enable debug logging
export RUST_LOG=debug

# Specify model directory
export HUSH_MODEL_DIR=~/.local/share/hush/models
```

---

## Next Steps

- Read the [README.md](README.md) for usage examples
- Check [CHANGELOG.md](CHANGELOG.md) for latest features
- Review `.ai/knowledge/` for developer documentation
- Join the community (see README for links)

---

**Last Updated:** 2025-11-19
