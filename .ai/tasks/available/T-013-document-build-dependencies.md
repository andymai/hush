# Task: Document Build Dependencies and Fix Build Issues

## Description

Document all system dependencies required to build Hush and resolve build issues (particularly libdbus). This is a Priority 2 task from the architecture analysis.

## Requirements

- [ ] Document all system dependencies in README or INSTALL.md
- [ ] Provide installation commands for major Linux distros
- [ ] Fix or make optional the libdbus dependency issue
- [ ] Add dependency checks to build process
- [ ] Create setup script for development environment
- [ ] Test build on clean Ubuntu/Debian/Fedora/Arch systems
- [ ] Update CI to verify documentation is accurate

## Success Criteria

- `cargo build` succeeds on clean system following docs
- All system dependencies documented with installation commands
- Build errors provide helpful guidance on missing dependencies
- Setup script automates common dependency installation
- CI verifies build works from scratch
- Optional: Dockerfile for reproducible builds

## Context

Architecture analysis found: "⚠️ **Build Dependencies** - Some system dependencies may cause build issues (libdbus)"

**Current build error:**
```
error: failed to run custom build command for `libdbus-sys v0.2.6`
```

This blocks new contributors from building the project.

## System Dependencies to Document

Based on `Cargo.toml` analysis, likely dependencies:

### Audio (CPAL)
- `libasound2-dev` (Debian/Ubuntu)
- `alsa-lib-devel` (Fedora)
- `alsa-lib` (Arch)

### X11
- `libx11-dev` (Debian/Ubuntu)
- `libX11-devel` (Fedora)
- `libx11` (Arch)

### DBus (for notifications)
- `libdbus-1-dev` (Debian/Ubuntu)
- `dbus-devel` (Fedora)
- `dbus` (Arch)

### UInput
- Linux kernel with UInput support (usually built-in)
- `/dev/uinput` access (permissions)

### CUDA (optional)
- NVIDIA CUDA Toolkit 11.0+ (for GPU acceleration)
- Document as optional with fallback to CPU

### Build tools
- `pkg-config`
- `build-essential` / `base-devel`

## Tasks

### 1. Create Installation Documentation

Create `INSTALL.md`:

```markdown
# Installation Guide

## System Requirements

- Linux (Ubuntu 20.04+, Fedora 35+, Arch Linux)
- Rust 1.70+
- Audio device (microphone)

## System Dependencies

### Ubuntu/Debian

\`\`\`bash
sudo apt-get update
sudo apt-get install -y \
    build-essential \
    pkg-config \
    libasound2-dev \
    libx11-dev \
    libdbus-1-dev
\`\`\`

### Fedora

\`\`\`bash
sudo dnf install -y \
    gcc \
    pkg-config \
    alsa-lib-devel \
    libX11-devel \
    dbus-devel
\`\`\`

### Arch Linux

\`\`\`bash
sudo pacman -S --needed \
    base-devel \
    alsa-lib \
    libx11 \
    dbus
\`\`\`

## Optional: CUDA Support

For GPU-accelerated transcription:
[CUDA installation instructions]

## Build

\`\`\`bash
git clone https://github.com/andymai/hush
cd hush
cargo build --release
\`\`\`
```

### 2. Create Setup Script

Create `scripts/setup-dev.sh`:

```bash
#!/bin/bash
set -e

echo "Setting up Hush development environment..."

# Detect OS
if [ -f /etc/os-release ]; then
    . /etc/os-release
    OS=$ID
else
    echo "Cannot detect OS"
    exit 1
fi

# Install dependencies based on OS
case $OS in
    ubuntu|debian)
        sudo apt-get update
        sudo apt-get install -y build-essential pkg-config libasound2-dev libx11-dev libdbus-1-dev
        ;;
    fedora)
        sudo dnf install -y gcc pkg-config alsa-lib-devel libX11-devel dbus-devel
        ;;
    arch|manjaro)
        sudo pacman -S --needed base-devel alsa-lib libx11 dbus
        ;;
    *)
        echo "Unsupported OS: $OS"
        echo "Please install dependencies manually. See INSTALL.md"
        exit 1
        ;;
esac

echo "Dependencies installed successfully!"
echo "Run: cargo build"
```

### 3. Fix libdbus Dependency

Options:
1. Make notifications optional (already has feature flag)
2. Use alternative notification library
3. Document dependency clearly

**Recommended:** Ensure `notify-rust` dependency handles libdbus gracefully:

```toml
[dependencies]
# Make notifications truly optional
notify-rust = { version = "4.10", optional = true }

[features]
default = []  # Don't include notifications by default
notifications = ["dep:notify-rust"]
```

### 4. Add Helpful Build Error Messages

Create `build.rs` with dependency checks:

```rust
fn main() {
    // Check for pkg-config
    if !std::process::Command::new("pkg-config").arg("--version")
        .output().is_ok() {
        eprintln!("ERROR: pkg-config not found");
        eprintln!("Install: sudo apt-get install pkg-config");
        std::process::exit(1);
    }

    // Check for ALSA
    if !std::process::Command::new("pkg-config").args(&["--exists", "alsa"])
        .status().is_ok() {
        eprintln!("WARNING: ALSA not found");
        eprintln!("Install: sudo apt-get install libasound2-dev");
    }
}
```

## Files to Create/Modify

- `INSTALL.md` - Comprehensive installation guide
- `scripts/setup-dev.sh` - Automated setup script
- `build.rs` - Dependency checking (optional)
- `README.md` - Link to INSTALL.md
- `Cargo.toml` - Make notifications truly optional
- `.github/workflows/ci.yml` - Test on multiple distros

## Testing

Test on:
- [ ] Clean Ubuntu 22.04 container
- [ ] Clean Debian 12 container
- [ ] Clean Fedora 39 container
- [ ] Clean Arch Linux container
- [ ] WSL2 (if claiming Linux support)

## Estimated Complexity

**Medium** - Requires:
- Testing on multiple Linux distributions
- Understanding all transitive dependencies
- Writing clear documentation
- Creating robust setup scripts
- Potentially refactoring optional features

High value for contributor experience.

## Related Tasks

- Part of Priority 2 recommendations
- Blocks new contributor onboarding
- Complements architecture documentation
