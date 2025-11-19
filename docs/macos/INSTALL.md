# Installing Hush on macOS

Hush is a fast, accurate voice-to-text application that works great on macOS, with native Metal GPU acceleration for Apple Silicon.

## System Requirements

### Minimum
- macOS 11.0 (Big Sur) or later
- 8 GB RAM
- 2 GB free disk space (for Whisper models)
- Microphone access

### Recommended
- macOS 13.0 (Ventura) or later
- Apple Silicon (M1, M2, M3, M4+) for Metal GPU acceleration
- 16 GB RAM
- 5 GB free disk space

### Supported Architectures
- ✅ Apple Silicon (M1, M2, M3, M4) - Full Metal GPU support
- ✅ Intel Macs - CPU-only mode

## Installation Methods

### Method 1: Homebrew (Coming Soon)

```bash
# Not yet available
brew install hush
```

### Method 2: Pre-built Binary

```bash
# Download latest release
curl -L https://github.com/andymai/hush/releases/latest/download/hush-macos-$(uname -m).tar.gz -o hush.tar.gz

# Extract
tar xzf hush.tar.gz

# Move to /usr/local/bin
sudo mv hush /usr/local/bin/

# Verify installation
hush --version
```

### Method 3: Build from Source

#### Prerequisites

Install Xcode Command Line Tools:
```bash
xcode-select --install
```

Install Rust (if not already installed):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

#### Build

```bash
# Clone repository
git clone https://github.com/andymai/hush.git
cd hush

# Build with Metal GPU support (Apple Silicon)
cargo build --release --features metal

# OR: Build CPU-only (Intel Mac or testing)
cargo build --release --no-default-features

# Install to /usr/local/bin
sudo cp target/release/hush /usr/local/bin/

# Verify
hush --version
```

#### Build Times
- Apple Silicon (M2): ~5-10 minutes
- Intel Mac: ~10-15 minutes

## Initial Setup

### 1. Download Whisper Model

```bash
# Download recommended model (base.en - 150MB)
hush models download base.en

# Options:
# - tiny.en (75MB) - Fastest, lower accuracy
# - base.en (150MB) - Good balance (recommended)
# - small.en (500MB) - Better accuracy
# - medium.en (1.5GB) - Best accuracy, slower
```

### 2. Grant Permissions

Hush requires two permissions:

#### Microphone Access
On first run, macOS will prompt for microphone access. Click "OK".

Manual grant:
1. System Settings → Privacy & Security → Microphone
2. Enable checkbox for "hush" or your terminal app

#### Accessibility Access (Required for Text Insertion)
Hush needs Accessibility permissions to insert transcribed text.

**Setup:**
1. Run `hush check-permissions`
2. If not granted, macOS will show instructions
3. System Settings → Privacy & Security → Accessibility
4. Click the "+" button
5. Navigate to `/usr/local/bin/hush` (or your terminal app)
6. Enable the checkbox

**Important:** Restart the app after granting permissions.

### 3. Configure Hotkey

Default hotkey: `Cmd+Alt+V`

To customize:
```bash
# Edit config file
open ~/.config/hush/config.toml

# Change hotkey line:
hotkey = "Cmd+Shift+V"  # Or your preferred combo
```

### 4. Test Installation

```bash
# Check device and permissions
hush status --full
hush check-permissions

# Start listening
hush listen

# Press Cmd+Alt+V, speak, verify text appears
```

## GPU Acceleration

### Apple Silicon (M1/M2/M3/M4)

Metal GPU acceleration is **enabled by default** on Apple Silicon.

**Performance:**
- First token latency: ~120ms (6-8x faster than CPU)
- Continuous streaming: ~50ms per word
- Model loading: ~1-2 seconds

**Verify Metal is active:**
```bash
hush status --devices
# Expected output: "Metal GPU: Apple Silicon (Apple M2)" or similar
```

### Intel Macs

Metal GPU is not effective on Intel Macs. CPU-only mode is recommended.

**Performance:**
- First token latency: ~800ms
- Continuous streaming: ~200ms per word
- Model loading: ~2-3 seconds

**Build for Intel:**
```bash
cargo build --release --no-default-features
```

## Uninstallation

```bash
# Remove binary
sudo rm /usr/local/bin/hush

# Remove config and models
rm -rf ~/.config/hush
rm -rf ~/.local/share/hush

# Remove logs
rm -rf ~/Library/Logs/hush
```

## Next Steps

- [Permission Setup Guide](PERMISSIONS.md)
- [Troubleshooting](TROUBLESHOOTING.md)
- [Architecture Documentation](../ARCHITECTURE.md)

## Getting Help

- GitHub Issues: https://github.com/andymai/hush/issues
- Documentation: https://github.com/andymai/hush/docs
