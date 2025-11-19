# Installation Guide

Complete installation guide for Hush voice-to-text application on Linux.

## Table of Contents
- [System Requirements](#system-requirements)
- [System Dependencies](#system-dependencies)
- [Quick Installation](#quick-installation)
- [Step-by-Step Installation](#step-by-step-installation)
- [Optional: GPU Acceleration](#optional-gpu-acceleration)
- [Verification](#verification)
- [Troubleshooting](#troubleshooting)

---

## System Requirements

### Minimum Requirements
- **OS:** Ubuntu 20.04+ or compatible Linux distribution (Debian, Fedora, Arch Linux)
- **Rust:** 1.70 or newer
- **RAM:** 4GB (8GB recommended for larger models)
- **Storage:** 1GB free space for Whisper models
- **Audio:** Microphone or audio input device

### Recommended for GPU Acceleration
- **GPU:** NVIDIA GTX 1060 or newer (GTX 10-series+)
- **VRAM:** 4GB+ (for medium/large models)
- **CUDA:** 12.0+ toolkit
- **Drivers:** NVIDIA 520.x or newer

---

## System Dependencies

Hush requires several system libraries to build and run. Choose the commands for your Linux distribution:

### Ubuntu / Debian

```bash
sudo apt-get update
sudo apt-get install -y \
    build-essential \
    pkg-config \
    libasound2-dev \
    libx11-dev \
    libdbus-1-dev
```

**Package Details:**
- `build-essential` - GCC compiler and build tools
- `pkg-config` - Helper tool for compiling applications
- `libasound2-dev` - ALSA audio library (for microphone input)
- `libx11-dev` - X11 window system library
- `libdbus-1-dev` - D-Bus system message bus (for desktop notifications)

### Fedora / RHEL / CentOS

```bash
sudo dnf install -y \
    gcc \
    gcc-c++ \
    pkg-config \
    make \
    alsa-lib-devel \
    libX11-devel \
    dbus-devel
```

### Arch Linux / Manjaro

```bash
sudo pacman -S --needed \
    base-devel \
    pkg-config \
    alsa-lib \
    libx11 \
    dbus
```

### openSUSE

```bash
sudo zypper install -y \
    gcc \
    gcc-c++ \
    pkg-config \
    make \
    alsa-devel \
    libX11-devel \
    dbus-1-devel
```

### Void Linux

```bash
sudo xbps-install -S \
    base-devel \
    pkg-config \
    alsa-lib-devel \
    libX11-devel \
    dbus-devel
```

---

## Quick Installation

**Automated setup script** (Ubuntu/Debian/Fedora/Arch):

```bash
# Clone repository
git clone https://github.com/andymai/hush.git
cd hush

# Run automated setup (installs dependencies and builds)
./scripts/setup-dev.sh

# Download Whisper model
./hush models download base

# Setup UInput for text insertion
./hush setup uinput --quick

# Verify installation
./hush status --full
```

---

## Step-by-Step Installation

### 1. Install Rust

If you don't have Rust installed:

```bash
# Install Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Follow the prompts, then:
source "$HOME/.cargo/env"

# Verify installation
rustc --version
cargo --version
```

### 2. Install System Dependencies

Choose the command for your distribution from the [System Dependencies](#system-dependencies) section above.

### 3. Clone Hush Repository

```bash
git clone https://github.com/andymai/hush.git
cd hush
```

### 4. Build Hush

```bash
# Debug build (faster compilation, slower runtime)
cargo build

# OR: Release build (recommended - optimized for performance)
cargo build --release

# OR: Use the Makefile
make build        # Debug build
make release      # Release build
```

**Build artifacts:**
- Debug: `./target/debug/hush`
- Release: `./target/release/hush`

You can create a symlink for convenience:
```bash
# If you built with --release
ln -sf ./target/release/hush ./hush

# If you built debug
ln -sf ./target/debug/hush ./hush
```

### 5. Download Whisper Model

```bash
# Download the base model (recommended for general use)
./hush models download base

# Or choose a different model size:
./hush models download tiny    # 75 MB - fastest, basic accuracy
./hush models download small   # 466 MB - better accuracy
./hush models download medium  # 1.5 GB - high accuracy
./hush models download large   # 2.9 GB - highest accuracy
```

### 6. Setup Text Insertion (UInput)

For Hush to automatically insert transcribed text, you need UInput access:

```bash
# Quick setup (temporary - until reboot)
./hush setup uinput --quick

# OR: Permanent setup (recommended)
./hush setup uinput --auto-fix
```

This creates a udev rule that gives your user access to `/dev/uinput`.

### 7. Verify Installation

```bash
# Check system status
./hush status --full

# Test individual components
./hush test audio --duration 3
./hush test transcription
./hush test text-insertion

# All tests passing? You're ready!
```

---

## Optional: GPU Acceleration

GPU acceleration provides 10x faster transcription (e.g., 0.5s instead of 5s for typical dictation).

### Prerequisites

1. **NVIDIA GPU** - GTX 10-series or newer
2. **NVIDIA Drivers** - Version 520.x or newer
3. **CUDA Toolkit** - Version 12.0 or newer

### Install NVIDIA Drivers

#### Ubuntu / Debian

```bash
# Check recommended driver
ubuntu-drivers devices

# Install recommended driver (e.g., 535)
sudo apt install nvidia-driver-535

# Reboot
sudo reboot

# Verify installation
nvidia-smi
```

#### Fedora

```bash
# Enable RPM Fusion repository
sudo dnf install -y https://download1.rpmfusion.org/free/fedora/rpmfusion-free-release-$(rpm -E %fedora).noarch.rpm
sudo dnf install -y https://download1.rpmfusion.org/nonfree/fedora/rpmfusion-nonfree-release-$(rpm -E %fedora).noarch.rpm

# Install NVIDIA drivers
sudo dnf install akmod-nvidia

# Reboot
sudo reboot

# Verify installation
nvidia-smi
```

#### Arch Linux

```bash
# Install NVIDIA drivers
sudo pacman -S nvidia nvidia-utils

# Reboot
sudo reboot

# Verify installation
nvidia-smi
```

### Install CUDA Toolkit

#### Ubuntu / Debian

```bash
# Add NVIDIA package repository
wget https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2204/x86_64/cuda-keyring_1.1-1_all.deb
sudo dpkg -i cuda-keyring_1.1-1_all.deb
sudo apt-get update

# Install CUDA toolkit
sudo apt-get install -y cuda-toolkit-12-3

# Add to PATH (add to ~/.bashrc for persistence)
export PATH=/usr/local/cuda/bin:$PATH
export LD_LIBRARY_PATH=/usr/local/cuda/lib64:$LD_LIBRARY_PATH
```

#### Fedora

```bash
# Add CUDA repository
sudo dnf config-manager --add-repo https://developer.download.nvidia.com/compute/cuda/repos/fedora37/x86_64/cuda-fedora37.repo

# Install CUDA
sudo dnf install cuda

# Add to PATH
export PATH=/usr/local/cuda/bin:$PATH
export LD_LIBRARY_PATH=/usr/local/cuda/lib64:$LD_LIBRARY_PATH
```

#### Arch Linux

```bash
# Install CUDA from official repos
sudo pacman -S cuda

# CUDA is automatically added to PATH
```

### Verify CUDA Installation

```bash
# Check NVIDIA driver
nvidia-smi

# Check CUDA compiler
nvcc --version

# Rebuild Hush (it will detect CUDA automatically)
cargo clean
cargo build --release

# Verify GPU is detected
./hush status --full | grep -i cuda
# Expected output: "use gpu = 1" and "CUDA0 total size"
```

### Performance Comparison

| Model  | CPU Time | GPU Time | Speedup |
|--------|----------|----------|---------|
| Tiny   | ~2-3s    | ~0.3s    | 8x      |
| Base   | ~4-6s    | ~0.5s    | 10x     |
| Small  | ~8-12s   | ~0.8s    | 12x     |
| Medium | ~15-25s  | ~1.5s    | 14x     |
| Large  | ~30-45s  | ~2.5s    | 16x     |

*Measured on RTX 4080 SUPER with 3-second audio clips*

---

## Verification

### System Status Check

```bash
./hush status --full
```

**Expected output should include:**
- ✅ Rust version (1.70+)
- ✅ Audio device detected
- ✅ UInput accessible (`/dev/uinput` found)
- ✅ Whisper model present
- ✅ GPU detected (if CUDA installed)

### Component Testing

```bash
# Test audio capture (should save test.wav)
./hush test audio --duration 3

# Test transcription (should show transcribed text)
./hush test transcription

# Test text insertion (should type "test" in active window)
./hush test text-insertion

# Test full pipeline
./hush test pipeline
```

### First Transcription

```bash
# Start intelligent listening mode
./hush listen

# Hold Ctrl+Alt+V, speak "hello world", release
# Text should appear in your active application
```

---

## Troubleshooting

### Build Errors

#### `error: failed to run custom build command for 'libdbus-sys'`

**Problem:** Missing D-Bus development libraries.

**Solution:**
```bash
# Ubuntu/Debian
sudo apt-get install libdbus-1-dev

# Fedora
sudo dnf install dbus-devel

# Arch
sudo pacman -S dbus
```

Alternatively, build without notifications:
```bash
cargo build --release --no-default-features
```

#### `error: linker 'cc' not found`

**Problem:** Missing C compiler.

**Solution:**
```bash
# Ubuntu/Debian
sudo apt-get install build-essential

# Fedora
sudo dnf install gcc gcc-c++

# Arch
sudo pacman -S base-devel
```

#### `pkg-config: command not found`

**Problem:** Missing pkg-config tool.

**Solution:**
```bash
# Ubuntu/Debian
sudo apt-get install pkg-config

# Fedora
sudo dnf install pkg-config

# Arch
sudo pacman -S pkg-config
```

#### `Could not find ALSA development files`

**Problem:** Missing ALSA libraries for audio capture.

**Solution:**
```bash
# Ubuntu/Debian
sudo apt-get install libasound2-dev

# Fedora
sudo dnf install alsa-lib-devel

# Arch
sudo pacman -S alsa-lib
```

### Runtime Errors

#### Text Insertion Not Working

```bash
# Diagnose UInput issues
./hush setup diagnose-uinput

# Quick fix (temporary)
sudo modprobe uinput
sudo chmod 666 /dev/uinput

# Permanent fix
./hush setup uinput --auto-fix
```

#### Audio Device Not Found

```bash
# List available audio devices
./hush status --devices

# Test specific device
./hush test audio --device "pulse" --duration 3
```

#### GPU Not Detected

```bash
# Check NVIDIA driver
nvidia-smi

# Check if CUDA libraries are in path
echo $LD_LIBRARY_PATH | grep cuda

# Rebuild with CUDA detection
cargo clean
cargo build --release

# Verify
./hush status --full | grep -i cuda
```

### Still Having Issues?

1. **Enable verbose logging:**
   ```bash
   ./hush -vv listen
   ```

2. **Check logs:**
   ```bash
   tail -f ~/.local/share/hush/logs/*.log
   ```

3. **Get system information:**
   ```bash
   ./hush status --full > status.txt
   ```

4. **Open an issue:** [GitHub Issues](https://github.com/andymai/hush/issues)
   - Include output from `./hush status --full`
   - Include relevant log excerpts
   - Describe steps to reproduce

---

## Build Options

### Without Desktop Notifications

If you don't need desktop notifications or have D-Bus issues:

```bash
cargo build --release --no-default-features
```

### Development Build

For development with debug symbols and faster compilation:

```bash
cargo build
# Binary at: ./target/debug/hush
```

### Optimized Release Build

For production use with maximum performance:

```bash
cargo build --release
# Binary at: ./target/release/hush
```

Or use the Makefile:
```bash
make release
```

---

## Next Steps

After successful installation:

1. **Read the Quick Start guide:** See [README.md](README.md#-quick-start)
2. **Configure hotkeys:** Default is `Ctrl+Alt+V` (customizable)
3. **Enable LLM polishing (optional):** Create `.env` with `ANTHROPIC_API_KEY`
4. **Join the community:** [GitHub Discussions](https://github.com/andymai/hush/discussions)

---

## System-Specific Notes

### WSL2 (Windows Subsystem for Linux)

Hush works on WSL2 with some limitations:
- Audio input requires PulseAudio forwarding
- GPU acceleration requires CUDA on WSL
- Text insertion works within WSL environment only

### Docker / Containers

For containerized builds, see the example `Dockerfile`:
```bash
# Build in container
docker build -t hush .

# Run with audio and display access
docker run --rm \
  --device /dev/snd \
  -e DISPLAY=$DISPLAY \
  -v /tmp/.X11-unix:/tmp/.X11-unix \
  hush
```

### Headless Servers

For SSH/headless use:
- Audio capture works via ALSA
- Text insertion works but requires X11 forwarding or local display
- Overlay UI requires display (can be disabled with `--no-button`)

---

**Ready to start?** Return to [README.md](README.md) for usage instructions and examples.
