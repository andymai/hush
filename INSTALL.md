# Installation

## Dependencies

### Ubuntu / Debian
```bash
sudo apt install build-essential cmake pkg-config libasound2-dev \
  libx11-dev libxi-dev libxtst-dev libxcursor-dev libxrandr-dev libxinerama-dev libgl1-mesa-dev \
  glslc libvulkan-dev
```

### Fedora
```bash
sudo dnf install gcc gcc-c++ cmake make pkgconf-pkg-config alsa-lib-devel \
  libX11-devel libXi-devel libXtst-devel libXcursor-devel libXrandr-devel libXinerama-devel mesa-libGL-devel \
  glslc vulkan-headers vulkan-loader-devel
```

### Arch Linux
```bash
sudo pacman -S base-devel cmake pkgconf alsa-lib \
  libx11 libxi libxtst libxcursor libxrandr libxinerama mesa \
  shaderc vulkan-headers vulkan-icd-loader
```

## Build

```bash
git clone https://github.com/andymai/hush.git
cd hush

make release        # Vulkan GPU build: NVIDIA, AMD, and Intel with the stock driver
make release-cuda   # CUDA build, needs the CUDA toolkit (nvcc)
make release-cpu    # CPU only
```

## Setup

```bash
./hush setup init             # Write ~/.config/hush/config.toml
./hush models download base   # Download a Whisper model
./hush setup uinput --quick   # Enable text insertion
./hush status --full          # Verify
```

## GPU Acceleration

The Vulkan build needs `glslc` and the Vulkan headers at build time and only the
graphics driver at runtime. `./hush status` shows the device ggml found.

### CUDA

`make release-cuda` needs CUDA 12.0+ with `nvcc` on `PATH`.

**Ubuntu:**
```bash
wget https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2404/x86_64/cuda-keyring_1.1-1_all.deb
sudo dpkg -i cuda-keyring_1.1-1_all.deb
sudo apt update && sudo apt install cuda-toolkit-12-6
export PATH=/usr/local/cuda/bin:$PATH
```

**Fedora:**
```bash
sudo dnf config-manager addrepo --from-repofile=https://developer.download.nvidia.com/compute/cuda/repos/fedora41/x86_64/cuda-fedora41.repo
sudo dnf install cuda-toolkit
```

**Arch:**
```bash
sudo pacman -S cuda
```

## Troubleshooting

**Build fails with missing library:** the error names the header; install the
matching `-dev` (Ubuntu) or `-devel` (Fedora) package from the lists above.

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
vulkaninfo --summary          # Vulkan driver present?
./hush status                 # Device ggml found
nvidia-smi && nvcc --version  # CUDA build only
```
