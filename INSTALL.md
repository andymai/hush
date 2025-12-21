# Installation

## Dependencies

### Ubuntu / Debian
```bash
sudo apt install build-essential pkg-config libasound2-dev libx11-dev
```

### Fedora
```bash
sudo dnf install gcc gcc-c++ pkg-config make alsa-lib-devel libX11-devel
```

### Arch Linux
```bash
sudo pacman -S base-devel pkg-config alsa-lib libx11
```

## Build

```bash
git clone https://github.com/andymai/hush.git
cd hush

# GPU-accelerated build (requires CUDA 12.0+)
make release

# Or CPU-only build
make release-cpu
```

## Setup

```bash
# Download Whisper model
./hush models download base

# Enable text insertion
./hush setup uinput --quick

# Verify
./hush status --full
```

## GPU Acceleration (Optional)

GPU provides ~10x faster transcription. Requires NVIDIA GPU with CUDA 12.0+.

### Install CUDA

**Ubuntu:**
```bash
wget https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2204/x86_64/cuda-keyring_1.1-1_all.deb
sudo dpkg -i cuda-keyring_1.1-1_all.deb
sudo apt update && sudo apt install cuda-toolkit-12-3
export PATH=/usr/local/cuda/bin:$PATH
```

**Fedora:**
```bash
sudo dnf config-manager --add-repo https://developer.download.nvidia.com/compute/cuda/repos/fedora37/x86_64/cuda-fedora37.repo
sudo dnf install cuda
```

**Arch:**
```bash
sudo pacman -S cuda
```

Verify with `nvidia-smi` and `nvcc --version`, then rebuild.

## Troubleshooting

**Build fails with missing library:**
```bash
# Check which package is missing from the error message
# Ubuntu: sudo apt install <package>-dev
# Fedora: sudo dnf install <package>-devel
```

**Text insertion not working:**
```bash
./hush setup diagnose-uinput
./hush setup uinput --auto-fix
```

**Audio device not found:**
```bash
./hush status --devices
./hush test audio --duration 3
```

**GPU not detected:**
```bash
nvidia-smi                    # Check driver
nvcc --version                # Check CUDA
cargo clean && make release   # Rebuild
```
