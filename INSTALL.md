# Installation

## Release files

Each release has four packages for x86-64 Linux and a `SHA256SUMS` file. The
[latest release](https://github.com/andymai/hush/releases/latest) lists them.
`<version>` is the release number, such as `0.30.2`.

| File | Install |
|---|---|
| `hush_<version>-1_amd64.deb` | `sudo apt install ./hush_<version>-1_amd64.deb` |
| `hush-<version>-1.x86_64.rpm` | `sudo dnf install ./hush-<version>-1.x86_64.rpm` |
| `hush-<version>-x86_64-linux.tar.gz` | Extract it, and keep `bin/hush` and `lib/hush` together |
| `hush-<version>-x86_64.AppImage` | `chmod +x` the file, then run it |

Three other ways install Hush:

- `install.sh` picks the `.deb` on Debian and Ubuntu, the `.rpm` on Fedora,
  RHEL, and openSUSE, and the tarball on every other distribution. The tarball
  goes in `~/.local`. Set `HUSH_VERSION` to a tag, such as `v0.30.2`, to
  install that release.
- COPR builds an `.rpm` from the release tarball. Run
  `sudo dnf copr enable veroz/hush && sudo dnf install hush`.
- The [Nix flake](#nix-and-nixos) builds Hush from source.

To build from source, see [Build](#build).

## Libraries and CPU

Hush runs on any x86-64 CPU. The speech engine, `libtranscribe`, loads the ggml
backends as modules from its own folder. The folder has one CPU module for each
x86 level, from plain x86-64 to AVX-512, and one GPU module. The GPU module is
Vulkan, or CUDA in a `make release-cuda` build. At start, ggml loads the CPU
module that fits the CPU best. The `Backends` line of `hush doctor` and of
`hush status --full` names the CPU module, the GPU backend, and the folder.

The binary looks for `libtranscribe` and the modules in `../lib/hush`, next to
its `bin` folder. A binary that `cargo build` or `make release` makes also looks
in the `lib/hush` folder next to it:

| Install | Binary | Libraries |
|---|---|---|
| `.deb`, `.rpm`, COPR | `/usr/bin/hush` | `/usr/lib/hush` |
| Tarball | `bin/hush` | `lib/hush` |
| `install.sh` with the tarball | `~/.local/bin/hush` | `~/.local/lib/hush` |
| AppImage | `usr/bin/hush` in the image | `usr/lib/hush` in the image |
| Nix flake | `bin/hush` in the store path | `lib/hush` in the store path |
| `make release` | `target/release/hush` | `target/release/lib/hush` |
| `make install` | `~/.cargo/bin/hush` | `~/.cargo/lib/hush` |

To move the tarball's Hush, move `bin/hush` and `lib/hush` together. A copy of
the binary alone does not start.

Hush loads `libxkbcommon` at run time, to type on every keyboard layout. The
`.deb` and the `.rpm` recommend it with the XKB layout data (`xkb-data` or
`xkeyboard-config`). With the tarball and the AppImage, install them yourself.
Without `libxkbcommon`, Hush types on the plain US layout and pastes on every
other layout.

## Verify a download

The release workflow attests the build provenance of each release file: the
tarball, the `.deb`, the `.rpm`, the AppImage, and `SHA256SUMS`. An attestation
is a Sigstore-signed record. It ties the SHA-256 digest of a file to the
workflow run that built the file. Releases from 0.17.0 have attestations.

To verify a release file, use GitHub CLI 2.49 or later. The command needs a
GitHub login, also for a public repository.

```bash
gh auth login
gh attestation verify <file> --repo andymai/hush
```

The command fails when no attestation from `andymai/hush` matches the file.

`install.sh` verifies the attestation after the checksum:

- If you have `gh` 2.49 or later and you logged in, the script runs
  `gh attestation verify`. If verification fails, the script stops and installs
  nothing.
- If `gh` cannot verify, the script prints the command to run by hand
  and continues.
- A release before 0.17.0 has no attestation, so the script does not verify it.

## Dependencies

### Ubuntu / Debian
```bash
sudo apt install build-essential cmake pkg-config libasound2-dev \
  libx11-dev libxi-dev libxtst-dev libxcursor-dev libxrandr-dev libxinerama-dev libgl1-mesa-dev \
  glslc libvulkan-dev spirv-headers
```

### Fedora
```bash
sudo dnf install gcc gcc-c++ cmake make pkgconf-pkg-config alsa-lib-devel \
  libX11-devel libXi-devel libXtst-devel libXcursor-devel libXrandr-devel libXinerama-devel mesa-libGL-devel \
  glslc vulkan-headers vulkan-loader-devel spirv-headers-devel
```

### Arch Linux
```bash
sudo pacman -S base-devel cmake pkgconf alsa-lib \
  libx11 libxi libxtst libxcursor libxrandr libxinerama mesa \
  shaderc vulkan-headers vulkan-icd-loader spirv-headers
```

## Build

```bash
git clone https://github.com/andymai/hush.git
cd hush

make release        # Vulkan GPU build: NVIDIA, AMD, and Intel with the stock driver
make release-cuda   # CUDA build, needs the CUDA toolkit (nvcc)
make release-cpu    # CPU only
make install        # The Vulkan build in ~/.cargo/bin and ~/.cargo/lib/hush; PREFIX=... for another place
```

Each target puts the binary in `target/release` and `libtranscribe` with the
ggml modules in `target/release/lib/hush`. `./hush` runs from there. A source
build has the CPU modules for every x86 level too, so the build machine does
not limit the CPU that runs it.

Use `make install`, not `cargo install`: `cargo install` copies only the
binary, which then cannot start. `make install` puts `lib/hush` next to the
`bin` folder, where the binary looks for its libraries.

## Setup

```bash
./hush setup init             # Write ~/.config/hush/config.toml
./hush models download base   # Download a Whisper model
./hush setup permissions      # One polkit prompt: keyboard and uinput access
./hush status --full          # Verify
```

## Nix and NixOS

The flake builds Hush with Vulkan:

```bash
nix run github:andymai/hush -- --version
nix profile install github:andymai/hush
nix develop                   # Build dependencies, clippy, and cargo-nextest
```

On NixOS, add the flake as an input named `hush` and add this module to the system configuration. Its udev rule replaces `hush setup permissions`, which cannot write to `/etc` on NixOS.

```nix
{ inputs, ... }:
let
  hush = inputs.hush.packages.x86_64-linux.default;
in
{
  environment.systemPackages = [ hush ];
  services.udev.packages = [ hush ];
  hardware.uinput.enable = true;
  systemd.packages = [ hush ];
  systemd.user.services.hush = {
    wantedBy = [ "graphical-session.target" ];
    # Hush runs hyprctl, swaymsg, and setxkbmap to find the focused window and the layout.
    path = [ "/run/current-system/sw" "/etc/profiles/per-user/%u" ];
  };
}
```

The user service starts the daemon at login. Use it instead of `hush install --autostart`, because that entry points at a store path, and garbage collection can remove the path after an upgrade.

## GPU Acceleration

The Vulkan build needs `glslc`, the Vulkan headers, and the SPIR-V headers at build
time and only the graphics driver at runtime. The Makefile sets
`TRANSCRIBE_CMAKE_ARGS=-DTRANSCRIBE_USE_SYSTEM_BLAS=OFF`. Without it, a plain
`cargo build` links OpenBLAS when the system has the library, and the binary then needs
OpenBLAS at runtime. `./hush status --full` shows the modules that loaded and the
device ggml found.

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

**"libtranscribe.so.0.4: cannot open shared object file" or "No ggml backend
module loaded":** the binary did not find its libraries in `../lib/hush`. Keep
`bin/hush` and `lib/hush` together, or install the package again. A binary from
`cargo install` or a copied binary has no `lib/hush`: use `make install`. See
[Libraries and CPU](#libraries-and-cpu).

**A crash at start with `LD_LIBRARY_PATH` set:** the dynamic loader reads
`LD_LIBRARY_PATH` before the folder of Hush. When the variable names a folder
with another `libggml-base.so.0`, `libggml.so.0`, or `libtranscribe.so.0.4`,
Hush loads that copy. A llama.cpp or whisper.cpp build can leave such a copy,
and its ggml version does not match. Start Hush without that folder in
`LD_LIBRARY_PATH`.

**Text insertion not working:**
```bash
./hush setup permissions --check
./hush setup permissions
```

**The first word gets cut off:** the microphone sleeps between recordings and
needs time to wake. Make one recording, then read the `Microphone wake` line of
`./hush doctor`. It warns when a recording waited more than 200 ms for the
microphone. Then keep that microphone awake with one WirePlumber rule:
```bash
./hush setup mic          # writes the rule; WirePlumber restarts, and sound stops for about a second
./hush setup mic --undo   # removes the rule again
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
