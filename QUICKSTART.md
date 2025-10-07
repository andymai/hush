# 🚀 Hush Quick Start Guide

## Prerequisites

You'll need:
- Rust 1.85.0+ installed
- Linux with X11 (Ubuntu/Debian recommended)
- A microphone
- System dependencies: `libasound2-dev`, `pkg-config`, `libxdo-dev`, `xclip`

```bash
# Install system dependencies (Ubuntu/Debian)
sudo apt-get install libasound2-dev pkg-config libxdo-dev xclip
```

## Quick Start (No Build Required - With Mocks)

The fastest way to test the app is using **mock mode** (no real audio/transcription):

```bash
# Test with mocks - no hardware needed!
cargo run --bin hush-new -- --mock manual

# In the app:
# - Press Enter to "start recording" (simulated)
# - Press Enter again to "stop" and see mock transcription
# - Type 'q' to quit
```

This lets you test the architecture without needing:
- ✅ Real microphone
- ✅ Whisper models downloaded
- ✅ X11 text insertion

## Running with Real Hardware

### Step 1: Download Whisper Models

```bash
./scripts/download-models.sh tiny
# or
./scripts/download-models.sh small
```

### Step 2: Build the App

```bash
# Development build
cargo build --bin hush-new

# Release build (recommended for actual use)
cargo build --release --bin hush-new
```

### Step 3: Run It!

#### Option A: Manual Mode (Easiest)
```bash
cargo run --bin hush-new -- manual

# Or with release binary:
./target/release/hush-new manual
```

**How it works:**
1. App starts and waits
2. Press **Enter** to start recording
3. Speak into your microphone
4. Press **Enter** again to stop
5. Wait for transcription
6. Text will be inserted at your cursor!
7. Press **q** to quit

#### Option B: One-Shot Mode (Quick Test)
```bash
# Record for 5 seconds, then transcribe
cargo run --bin hush-new -- one-shot --duration 5

# Record and print to stdout (no text insertion)
cargo run --bin hush-new -- one-shot --duration 5 --print-only
```

#### Option C: Daemon Mode (Hotkeys)
```bash
cargo run --bin hush-new -- daemon
```

**Note**: Daemon mode requires hotkey registration. If it doesn't work on your system (common on GNOME), use Manual mode instead.

## Available Binaries

### Main Applications

**hush-new** (v0.2.0 - New Architecture) ✅ **← Use This One**
```bash
cargo run --bin hush-new -- --help

# Commands:
cargo run --bin hush-new -- manual          # Manual recording
cargo run --bin hush-new -- one-shot        # Single recording
cargo run --bin hush-new -- daemon          # Daemon with hotkeys
cargo run --bin hush-new -- info            # System info

# Flags:
--mock                    # Use mocks (no hardware)
--no-notifications        # Disable notifications
-v, -vv                   # Verbose logging
-c <config>               # Custom config file
```

**hush-mvp** (v0.1.0 - Legacy)
```bash
cargo run --bin hush-mvp
```

### Utility Tools

**test-audio** - Test your microphone
```bash
cargo run --bin test-audio
# Records 3 seconds and plays it back
```

**test-transcription** - Test Whisper transcription
```bash
cargo run --bin test-transcription
# Transcribes a test audio file
```

**test-text-insertion** - Test X11 text insertion
```bash
cargo run --bin test-text-insertion
# Types "Hello from Hush!" at cursor
```

**simple-model-manager** - Manage Whisper models
```bash
cargo run --bin simple-model-manager list
cargo run --bin simple-model-manager info
```

## Configuration

Hush looks for config at `~/.config/hush/config.toml`:

```toml
[audio]
sample_rate = 16000      # Hz (optimal for Whisper)
channels = 1             # Mono for speech
device = null            # Auto-detect or specify device

[transcription]
model_path = "models/whisper-tiny"
model_size = "tiny"      # tiny, small, medium, large
language = "en"
use_cuda = false         # GPU acceleration (needs CUDA)

[hotkey]
enabled = true
combination = "Ctrl+Shift+Space"

[feedback]
audio_enabled = true
```

## Troubleshooting

### "No audio device found"
```bash
# List available devices
arecord -l

# Test recording
arecord -d 3 test.wav && aplay test.wav

# Use test-audio binary
cargo run --bin test-audio
```

### "Model not found"
```bash
# Download models
./scripts/download-models.sh tiny

# Check if models exist
ls -la models/
```

### "Failed to connect to X11"
- Make sure you're running on X11 (not Wayland)
- Check: `echo $DISPLAY` should show something like `:0`
- Try: `export DISPLAY=:0`

### "Hotkeys don't work"
This is **normal** on GNOME due to security restrictions. Use **Manual Mode** instead:
```bash
cargo run --bin hush-new -- manual
```

### Build Errors
```bash
# Common issue: missing ALSA
sudo apt-get install libasound2-dev pkg-config

# Set PKG_CONFIG_PATH if needed
export PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig
cargo build --bin hush-new
```

## Examples

### Example 1: Quick Test with Mocks
```bash
# Test the new architecture without hardware
cargo run --bin hush-new -- --mock manual

# Output:
# 🤫 Hush Voice-to-Text (New Architecture) v0.2.0
# 🎤 Audio: Mock Audio Device
# 🔄 Transcriber: MockTranscriber
# ⌨️  Text Output: Mock
# 🚀 Starting Hush in manual mode
# Press Enter to start recording...
```

### Example 2: Real Recording with Verbose Output
```bash
# See what's happening under the hood
cargo run --bin hush-new -- -vv manual

# Output will show:
# - Device initialization
# - Recording start/stop
# - Audio buffer details
# - Transcription progress
# - Text insertion
```

### Example 3: Quick 3-Second Recording
```bash
# Record for 3 seconds and print result
cargo run --bin hush-new -- one-shot --duration 3 --print-only

# Output:
# 🎤 Recording for 3 seconds...
# 🔄 Transcribing...
# 📝 Result: "your transcribed text here"
```

### Example 4: Production Usage
```bash
# Build release binary
cargo build --release --bin hush-new

# Copy to PATH
sudo cp target/release/hush-new /usr/local/bin/hush

# Run from anywhere
hush manual
```

## Next Steps

1. **Test with mocks** to verify the app works
2. **Download models** for real transcription
3. **Try manual mode** for easy testing
4. **Configure** to your preferences
5. **Set up GNOME shortcut** for convenience (optional)

## Need Help?

- **Check logs**: Run with `-vv` flag for detailed output
- **System info**: `cargo run --bin hush-new -- info`
- **Test components**: Use `test-audio`, `test-transcription`, `test-text-insertion`
- **Read docs**: See `docs/` directory for detailed documentation

## Performance Tips

- **tiny model**: Fastest, good for quick notes (~39MB)
- **small model**: Balanced speed/accuracy (~244MB)
- **medium model**: Better accuracy, slower (~769MB)
- **large model**: Best accuracy, slowest (~1.5GB)

For development: Use `tiny` model
For production: Use `small` or `medium` model

---

**Happy transcribing! 🎙️✨**
