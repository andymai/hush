#!/bin/bash
#
# Hush Development Environment Setup Script
#
# Automatically installs system dependencies and builds Hush
# on supported Linux distributions.
#
# Supported: Ubuntu, Debian, Fedora, RHEL, CentOS, Arch, Manjaro, openSUSE
#
# Usage: ./scripts/setup-dev.sh [--skip-build]

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Options
SKIP_BUILD=false
if [[ "$1" == "--skip-build" ]]; then
    SKIP_BUILD=true
fi

# Print colored output
print_header() {
    echo -e "${BLUE}==>${NC} ${1}"
}

print_success() {
    echo -e "${GREEN}✓${NC} ${1}"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} ${1}"
}

print_error() {
    echo -e "${RED}✗${NC} ${1}"
}

print_header "Hush Development Environment Setup"
echo

# Check if running as root
if [[ $EUID -eq 0 ]]; then
   print_error "This script should not be run as root"
   print_warning "Run without sudo. The script will ask for sudo password when needed."
   exit 1
fi

# Detect OS
print_header "Detecting operating system..."

if [ ! -f /etc/os-release ]; then
    print_error "Cannot detect OS (missing /etc/os-release)"
    print_warning "Please install dependencies manually. See INSTALL.md"
    exit 1
fi

# Source OS information
. /etc/os-release
OS=$ID
OS_VERSION=$VERSION_ID
OS_NAME=$NAME

print_success "Detected: $OS_NAME"
echo

# Install system dependencies based on OS
print_header "Installing system dependencies..."

case $OS in
    ubuntu|debian|pop|linuxmint|elementary)
        print_warning "This will install: build-essential, pkg-config, libasound2-dev, libx11-dev, libdbus-1-dev"
        echo

        sudo apt-get update

        sudo apt-get install -y \
            build-essential \
            pkg-config \
            libasound2-dev \
            libx11-dev \
            libdbus-1-dev

        print_success "Dependencies installed successfully"
        ;;

    fedora|rhel|centos|rocky|almalinux)
        print_warning "This will install: gcc, gcc-c++, pkg-config, make, alsa-lib-devel, libX11-devel, dbus-devel"
        echo

        sudo dnf install -y \
            gcc \
            gcc-c++ \
            pkg-config \
            make \
            alsa-lib-devel \
            libX11-devel \
            dbus-devel

        print_success "Dependencies installed successfully"
        ;;

    arch|manjaro|endeavouros|garuda)
        print_warning "This will install: base-devel, pkg-config, alsa-lib, libx11, dbus"
        echo

        sudo pacman -S --needed --noconfirm \
            base-devel \
            pkg-config \
            alsa-lib \
            libx11 \
            dbus

        print_success "Dependencies installed successfully"
        ;;

    opensuse*|sles)
        print_warning "This will install: gcc, gcc-c++, pkg-config, make, alsa-devel, libX11-devel, dbus-1-devel"
        echo

        sudo zypper install -y \
            gcc \
            gcc-c++ \
            pkg-config \
            make \
            alsa-devel \
            libX11-devel \
            dbus-1-devel

        print_success "Dependencies installed successfully"
        ;;

    void)
        print_warning "This will install: base-devel, pkg-config, alsa-lib-devel, libX11-devel, dbus-devel"
        echo

        sudo xbps-install -Sy \
            base-devel \
            pkg-config \
            alsa-lib-devel \
            libX11-devel \
            dbus-devel

        print_success "Dependencies installed successfully"
        ;;

    *)
        print_error "Unsupported OS: $OS"
        echo
        print_warning "Please install dependencies manually:"
        echo "  - C compiler (gcc/clang)"
        echo "  - pkg-config"
        echo "  - ALSA development libraries"
        echo "  - X11 development libraries"
        echo "  - D-Bus development libraries"
        echo
        print_warning "See INSTALL.md for detailed instructions"
        exit 1
        ;;
esac

echo

# Check for Rust
print_header "Checking Rust installation..."

if command -v rustc &> /dev/null; then
    RUST_VERSION=$(rustc --version)
    print_success "Rust is installed: $RUST_VERSION"

    # Check version (need 1.70+)
    RUST_VERSION_NUM=$(rustc --version | cut -d' ' -f2 | cut -d'.' -f1-2)
    if [[ $(echo "$RUST_VERSION_NUM >= 1.70" | bc -l) -eq 1 ]] 2>/dev/null || [[ "$RUST_VERSION_NUM" > "1.70" ]] || [[ "$RUST_VERSION_NUM" == "1.70" ]]; then
        print_success "Rust version is compatible (1.70+)"
    else
        print_warning "Rust version may be too old (need 1.70+)"
        print_warning "Consider updating: rustup update"
    fi
else
    print_warning "Rust is not installed"
    print_header "Installing Rust via rustup..."

    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

    # Source cargo environment
    . "$HOME/.cargo/env"

    print_success "Rust installed successfully"
fi

echo

# Build Hush
if [ "$SKIP_BUILD" = false ]; then
    print_header "Building Hush..."

    # Check if we're in the hush directory
    if [ ! -f "Cargo.toml" ]; then
        print_error "Cargo.toml not found. Please run this script from the hush directory."
        exit 1
    fi

    # Build in release mode
    print_warning "Building in release mode (this may take a few minutes)..."
    cargo build --release

    print_success "Build completed successfully"

    # Create symlink for convenience
    if [ -f "./target/release/hush" ]; then
        ln -sf ./target/release/hush ./hush
        print_success "Created symlink: ./hush -> ./target/release/hush"
    fi

    echo
else
    print_warning "Skipping build (--skip-build flag provided)"
    echo
fi

# Check for NVIDIA GPU and CUDA
print_header "Checking for GPU acceleration support..."

if command -v nvidia-smi &> /dev/null; then
    GPU_INFO=$(nvidia-smi --query-gpu=name --format=csv,noheader | head -n1)
    print_success "NVIDIA GPU detected: $GPU_INFO"

    if command -v nvcc &> /dev/null; then
        CUDA_VERSION=$(nvcc --version | grep "release" | awk '{print $5}' | sed 's/,//')
        print_success "CUDA toolkit installed: $CUDA_VERSION"
        print_success "GPU acceleration is available!"

        # Suggest rebuilding if CUDA was just installed
        if [ "$SKIP_BUILD" = false ]; then
            print_warning "If CUDA was recently installed, rebuild to enable GPU acceleration:"
            echo "  cargo clean && cargo build --release"
        fi
    else
        print_warning "NVIDIA GPU found but CUDA toolkit not installed"
        print_warning "Install CUDA 12.0+ for GPU acceleration (10x faster transcription)"
        echo "  See INSTALL.md for CUDA installation instructions"
    fi
else
    print_warning "No NVIDIA GPU detected"
    print_warning "Hush will use CPU for transcription (slower but still functional)"
    echo "  For 10x faster transcription, use a system with NVIDIA GPU + CUDA"
fi

echo

# Summary
print_header "Setup Summary"
echo
print_success "System dependencies installed"
if [ "$SKIP_BUILD" = false ]; then
    print_success "Hush built successfully"
fi

if command -v nvidia-smi &> /dev/null && command -v nvcc &> /dev/null; then
    print_success "GPU acceleration available"
else
    echo "  • GPU acceleration: Not available (CPU mode)"
fi

echo
print_header "Next Steps"
echo

if [ "$SKIP_BUILD" = false ]; then
    echo "1. Download a Whisper model:"
    echo "   ${BLUE}./hush models download base${NC}"
    echo
    echo "2. Setup text insertion (UInput):"
    echo "   ${BLUE}./hush setup uinput --quick${NC}"
    echo
    echo "3. Verify installation:"
    echo "   ${BLUE}./hush status --full${NC}"
    echo
    echo "4. Test components:"
    echo "   ${BLUE}./hush test audio --duration 3${NC}"
    echo "   ${BLUE}./hush test transcription${NC}"
    echo "   ${BLUE}./hush test text-insertion${NC}"
    echo
    echo "5. Start using Hush:"
    echo "   ${BLUE}./hush listen${NC}"
    echo
else
    echo "1. Build Hush:"
    echo "   ${BLUE}cargo build --release${NC}"
    echo
    echo "2. Follow the steps above to complete setup"
    echo
fi

print_success "Setup complete! 🎉"
echo
echo "For more information, see:"
echo "  • ${BLUE}INSTALL.md${NC} - Detailed installation guide"
echo "  • ${BLUE}README.md${NC} - Usage instructions and features"
echo "  • ${BLUE}./hush --help${NC} - Command-line help"
echo
