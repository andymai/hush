# Build System Guide for Hush

**Last Updated:** 2025-11-19
**Status:** Reference for AI agents and contributors

This document outlines build dependencies, configuration, and troubleshooting for building Hush from source.

---

## Quick Start

### Ubuntu/Debian

```bash
# Install dependencies
sudo apt-get update && sudo apt-get install -y \
    build-essential \
    pkg-config \
    libasound2-dev \
    libx11-dev \
    libdbus-1-dev

# Build
cargo build --release

# Run
./target/release/hush --version
```

### Fedora

```bash
# Install dependencies
sudo dnf install -y \
    gcc \
    pkg-config \
    alsa-lib-devel \
    libX11-devel \
    dbus-devel

# Build
cargo build --release
```

### Arch Linux

```bash
# Install dependencies
sudo pacman -S --needed \
    base-devel \
    alsa-lib \
    libx11 \
    dbus

# Build
cargo build --release
```

---

## System Dependencies

### Core Dependencies (Required)

| Dependency | Ubuntu/Debian | Fedora | Arch | Purpose |
|------------|---------------|--------|------|---------|
| Build tools | `build-essential` | `gcc` | `base-devel` | C compiler, linker |
| pkg-config | `pkg-config` | `pkg-config` | (in base-devel) | Library discovery |
| ALSA | `libasound2-dev` | `alsa-lib-devel` | `alsa-lib` | Audio I/O |
| X11 | `libx11-dev` | `libX11-devel` | `libx11` | Display server |
| DBus | `libdbus-1-dev` | `dbus-devel` | `dbus` | System notifications |

### Optional Dependencies

| Dependency | Ubuntu/Debian | Fedora | Arch | Purpose |
|------------|---------------|--------|------|---------|
| CUDA Toolkit | `nvidia-cuda-toolkit` | `cuda` | `cuda` | GPU acceleration |
| UInput module | (kernel) | (kernel) | (kernel) | Text insertion |

---

## Rust Requirements

### Minimum Supported Rust Version (MSRV)

**Version:** Rust 1.70.0 or later

**Verify:**
```bash
rustc --version
# Should show: rustc 1.70.0 or higher
```

**Install/Update Rust:**
```bash
# Install rustup (if not installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Update to latest stable
rustup update stable

# Or install specific version for MSRV testing
rustup install 1.70.0
```

### Required Rust Components

**Standard installation includes:**
- `rustc` - Compiler
- `cargo` - Build system and package manager
- `rustfmt` - Code formatter (for development)
- `clippy` - Linter (for development)

**Install components (if missing):**
```bash
rustup component add rustfmt clippy
```

---

## Build Configuration

### Cargo.toml Structure

```toml
[package]
name = "hush"
version = "0.1.0"
edition = "2021"
rust-version = "1.70"  # MSRV declaration

[dependencies]
# Core dependencies
tokio = { version = "1.38", features = ["full"] }
anyhow = "1.0"
thiserror = "1.0"

# Audio
cpal = "0.15"  # Pinned - 0.16 has breaking changes

# Optional features
notify-rust = { version = "4.10", optional = true }
tray-icon = { version = "0.14", optional = true }

[features]
default = ["notifications", "system-tray"]
notifications = ["dep:notify-rust"]
system-tray = ["dep:tray-icon"]

[dev-dependencies]
proptest = "1.0"
assert_cmd = "2.0"
```

### Feature Flags

**Default build (all features):**
```bash
cargo build
# Includes: notifications, system-tray
```

**Minimal build (no optional features):**
```bash
cargo build --no-default-features
# Smaller binary, fewer dependencies
```

**Custom features:**
```bash
cargo build --no-default-features --features notifications
cargo build --no-default-features --features system-tray
cargo build --features "notifications,system-tray"  # Same as default
```

**All features (including future ones):**
```bash
cargo build --all-features
```

---

## Build Commands

### Development Builds

```bash
# Fast build for development (debug mode)
cargo build

# Check compilation without building
cargo check  # Fastest way to verify code compiles

# Check with all features
cargo check --all-features

# Check with no features
cargo check --no-default-features
```

### Release Builds

```bash
# Optimized build
cargo build --release

# The binary will be at:
./target/release/hush

# Install to system
cargo install --path .
# Installs to ~/.cargo/bin/hush
```

### Build with Specific Features

```bash
# Minimal build (smallest binary)
cargo build --release --no-default-features

# Only notifications
cargo build --release --no-default-features --features notifications

# All features explicitly
cargo build --release --all-features
```

---

## Dependency Management

### Updating Dependencies

```bash
# Check for outdated dependencies
cargo outdated

# Update dependencies (respecting Cargo.toml constraints)
cargo update

# Update specific dependency
cargo update -p cpal

# Update and show what changed
cargo update --verbose
```

### Adding Dependencies

```bash
# Add a dependency
cargo add tokio --features full

# Add optional dependency
cargo add notify-rust --optional

# Add development dependency
cargo add proptest --dev
```

### Checking Dependency Tree

```bash
# Show dependency tree
cargo tree

# Show dependencies for specific feature
cargo tree --features notifications

# Show minimal dependencies
cargo tree --no-default-features
```

---

## Common Build Issues and Solutions

### Issue: libdbus-sys build failed

**Error:**
```
error: failed to run custom build command for `libdbus-sys v0.2.6`
```

**Solution:**
```bash
# Ubuntu/Debian
sudo apt-get install libdbus-1-dev

# Fedora
sudo dnf install dbus-devel

# Arch
sudo pacman -S dbus
```

### Issue: ALSA library not found

**Error:**
```
error: linking with `cc` failed: exit code: 1
/usr/bin/ld: cannot find -lasound
```

**Solution:**
```bash
# Ubuntu/Debian
sudo apt-get install libasound2-dev

# Fedora
sudo dnf install alsa-lib-devel

# Arch
sudo pacman -S alsa-lib
```

### Issue: X11 library not found

**Error:**
```
error: linking with `cc` failed: exit code: 1
/usr/bin/ld: cannot find -lX11
```

**Solution:**
```bash
# Ubuntu/Debian
sudo apt-get install libx11-dev

# Fedora
sudo dnf install libX11-devel

# Arch
sudo pacman -S libx11
```

### Issue: pkg-config not found

**Error:**
```
error: failed to run custom build command
Could not run `pkg-config`
```

**Solution:**
```bash
# Ubuntu/Debian
sudo apt-get install pkg-config

# Fedora
sudo dnf install pkg-config

# Arch
# Usually included in base-devel
```

### Issue: Linker 'cc' not found

**Error:**
```
error: linker `cc` not found
```

**Solution:**
```bash
# Ubuntu/Debian
sudo apt-get install build-essential

# Fedora
sudo dnf install gcc

# Arch
sudo pacman -S base-devel
```

### Issue: CUDA not available (non-blocking)

**Warning:**
```
Warning: CUDA not available, using CPU for transcription
```

**This is expected if:**
- No NVIDIA GPU present
- CUDA toolkit not installed
- Running in container/VM without GPU passthrough

**Solution (optional for GPU acceleration):**
```bash
# Ubuntu
sudo apt-get install nvidia-cuda-toolkit

# Or download from NVIDIA: https://developer.nvidia.com/cuda-downloads

# Verify
nvcc --version
```

**Note:** Hush works fine without CUDA, just slower transcription.

---

## Build Optimization

### Faster Builds

**Use `cargo check` for rapid iteration:**
```bash
cargo check  # Just type-checking, no codegen (~5x faster than build)
```

**Enable parallel compilation:**
```bash
# .cargo/config.toml
[build]
jobs = 8  # Or number of CPU cores
```

**Use sccache for distributed caching:**
```bash
cargo install sccache
export RUSTC_WRAPPER=sccache
cargo build
```

**Link-time optimization (slower build, faster binary):**
```bash
# Cargo.toml
[profile.release]
lto = true
codegen-units = 1
```

### Smaller Binaries

**Strip symbols:**
```bash
cargo build --release
strip target/release/hush

# Or in Cargo.toml:
[profile.release]
strip = true
```

**Optimize for size:**
```bash
# Cargo.toml
[profile.release]
opt-level = "z"  # Optimize for size instead of speed
lto = true
codegen-units = 1
panic = "abort"
```

**Build minimal features:**
```bash
cargo build --release --no-default-features
```

---

## Cross-Compilation

### Building for Different Targets

**List available targets:**
```bash
rustup target list
```

**Add a target:**
```bash
rustup target add x86_64-unknown-linux-musl
```

**Build for target:**
```bash
cargo build --release --target x86_64-unknown-linux-musl
```

**Note:** Cross-compilation for Hush is complex due to system dependencies (ALSA, X11, DBus). Recommended to build on target platform or use containers.

---

## Container Builds

### Dockerfile Example

```dockerfile
FROM rust:1.70 AS builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libasound2-dev \
    libx11-dev \
    libdbus-1-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source
COPY src ./src

# Build release
RUN cargo build --release

# Runtime image
FROM debian:bookworm-slim

# Install runtime libraries
RUN apt-get update && apt-get install -y \
    libasound2 \
    libx11-6 \
    libdbus-1-3 \
    && rm -rf /var/lib/apt/lists/*

# Copy binary
COPY --from=builder /app/target/release/hush /usr/local/bin/hush

ENTRYPOINT ["hush"]
```

**Build:**
```bash
docker build -t hush:latest .
```

**Note:** Running Hush in containers is limited (no audio device access, X11 forwarding needed).

---

## Verification and Testing

### Verify Build

```bash
# Build succeeds
cargo build --release

# Binary exists
ls -lh target/release/hush

# Binary runs
./target/release/hush --version

# Help text works
./target/release/hush --help
```

### Run Tests

```bash
# All tests
cargo test

# Specific test
cargo test test_name

# Integration tests
cargo test --test integration_*

# Doc tests
cargo test --doc
```

### Quality Checks

```bash
# Type checking
cargo check

# Linting
cargo clippy -- -D warnings

# Formatting
cargo fmt --check

# All checks (typical CI)
cargo check && \
  cargo test && \
  cargo clippy -- -D warnings && \
  cargo fmt --check
```

---

## Development Setup Script

**Create `scripts/setup-dev.sh`:**

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
        echo "Installing dependencies for Ubuntu/Debian..."
        sudo apt-get update
        sudo apt-get install -y \
            build-essential \
            pkg-config \
            libasound2-dev \
            libx11-dev \
            libdbus-1-dev
        ;;
    fedora)
        echo "Installing dependencies for Fedora..."
        sudo dnf install -y \
            gcc \
            pkg-config \
            alsa-lib-devel \
            libX11-devel \
            dbus-devel
        ;;
    arch|manjaro)
        echo "Installing dependencies for Arch Linux..."
        sudo pacman -S --needed \
            base-devel \
            alsa-lib \
            libx11 \
            dbus
        ;;
    *)
        echo "Unsupported OS: $OS"
        echo "Please install dependencies manually"
        echo "See: .ai/knowledge/build-system.md"
        exit 1
        ;;
esac

# Check Rust installation
if ! command -v cargo &> /dev/null; then
    echo "Rust not found. Installing rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    source $HOME/.cargo/env
fi

# Install Rust components
rustup component add rustfmt clippy

echo "✅ Dependencies installed successfully!"
echo ""
echo "Next steps:"
echo "  cargo build          # Build debug version"
echo "  cargo test           # Run tests"
echo "  cargo run -- --help  # Run Hush"
```

**Usage:**
```bash
chmod +x scripts/setup-dev.sh
./scripts/setup-dev.sh
```

---

## Environment Variables

### Build-time Variables

```bash
# Use specific Rust version
RUSTUP_TOOLCHAIN=1.70.0 cargo build

# Enable sccache
RUSTC_WRAPPER=sccache cargo build

# Verbose output
CARGO_LOG=debug cargo build

# Target directory
CARGO_TARGET_DIR=/tmp/hush-build cargo build
```

### Runtime Variables (for development)

```bash
# Logging level
RUST_LOG=debug cargo run

# Specific module logging
RUST_LOG=hush::audio=trace cargo run

# Backtrace on panic
RUST_BACKTRACE=1 cargo run
RUST_BACKTRACE=full cargo run  # More detailed
```

---

## Troubleshooting Checklist

When build fails, check:

- [ ] Rust version >= 1.70.0 (`rustc --version`)
- [ ] System dependencies installed (see tables above)
- [ ] pkg-config available (`pkg-config --version`)
- [ ] Build tools installed (`gcc --version`)
- [ ] Cargo.lock not corrupted (`cargo update`)
- [ ] Clean build (`cargo clean && cargo build`)
- [ ] Sufficient disk space (`df -h`)
- [ ] Internet connection (for downloading crates)

**Nuclear option (start fresh):**
```bash
cargo clean
rm -rf ~/.cargo/registry
rm Cargo.lock
cargo build
```

---

## Quick Reference

### Essential Commands

```bash
# Development
cargo check           # Fast compilation check
cargo build          # Debug build
cargo test           # Run tests
cargo run            # Build and run

# Release
cargo build --release    # Optimized build
cargo install --path .   # Install to ~/.cargo/bin

# Quality
cargo clippy         # Linting
cargo fmt            # Formatting
cargo audit          # Security check

# Features
cargo build --no-default-features         # Minimal
cargo build --features notifications      # Specific
cargo build --all-features                # All

# Verification
cargo check --all-features               # Check all builds
cargo test --all-features                # Test all builds
cargo tree                               # Dependency tree
```

### Finding Build Information

```bash
# Check dependencies
rg "\\[dependencies\\]" Cargo.toml -A 20

# Check features
rg "\\[features\\]" Cargo.toml -A 10

# Check build scripts
fd "build.rs" .

# Check required libs
rg "pkg-config" Cargo.toml
```

---

## Related Documentation

- `Cargo.toml` - Dependencies and features
- `README.md` - Quick start and installation
- `.ai/knowledge/ci-cd-patterns.md` - CI build configuration
- `.ai/knowledge/conventions.md` - Coding standards

---

**Remember:** When adding new dependencies, consider:
1. Does it require system libraries? (document in this file)
2. Is it optional? (make it a feature flag)
3. Does it increase MSRV? (check and update)
4. Is it well-maintained? (check crates.io)
5. Does it have security advisories? (run `cargo audit`)
