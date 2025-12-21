#!/bin/bash

# Hush Voice-to-Text Installation Script
# Builds and installs Hush to ~/.local/bin

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_status() { echo -e "${GREEN}[INFO]${NC} $1"; }
print_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
print_error() { echo -e "${RED}[ERROR]${NC} $1"; }
print_header() { echo -e "${BLUE}$1${NC}"; }

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || [ ! -d "src" ]; then
    print_error "This script must be run from the Hush project root directory"
    exit 1
fi

print_header "Hush Voice-to-Text Installation Script"
print_header "======================================"

# Check for Rust/Cargo
print_status "Checking dependencies..."
if ! command -v cargo &> /dev/null; then
    print_error "Cargo is not installed. Please install Rust from https://rustup.rs/"
    exit 1
fi

# Check for required system packages
if ! pkg-config --exists alsa; then
    print_error "Missing required package: libasound2-dev"
    print_status "Install with: sudo apt install libasound2-dev"
    exit 1
fi

print_status "All dependencies available"

# Build the application
print_status "Building Hush in release mode..."
make release

if [ $? -ne 0 ]; then
    print_error "Build failed"
    exit 1
fi

# Install the binary
BIN_DIR="$HOME/.local/bin"
mkdir -p "$BIN_DIR"

print_status "Installing hush binary to $BIN_DIR..."
cp target/release/hush "$BIN_DIR/hush"
chmod +x "$BIN_DIR/hush"

# Check if ~/.local/bin is in PATH
if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    print_warning "~/.local/bin is not in your PATH"
    print_status "Add this line to your ~/.bashrc or ~/.zshrc:"
    echo "    export PATH=\"\$HOME/.local/bin:\$PATH\""
fi

# Test the installation
print_status "Testing installation..."
if "$BIN_DIR/hush" --version > /dev/null 2>&1; then
    print_status "Binary installation successful"
else
    print_error "Binary installation failed"
    exit 1
fi

# Create configuration directory
CONFIG_DIR="$HOME/.config/hush"
mkdir -p "$CONFIG_DIR"

if [ ! -f "$CONFIG_DIR/config.toml" ] && [ -f "config/default.toml" ]; then
    cp "config/default.toml" "$CONFIG_DIR/config.toml"
    print_status "Created default configuration at $CONFIG_DIR/config.toml"
fi

print_header ""
print_header "Installation Complete!"
print_header "====================="
echo ""
print_status "Next steps:"
echo "1. Download a Whisper model:"
echo "   hush models download base"
echo ""
echo "2. Setup text insertion:"
echo "   hush setup uinput --quick"
echo ""
echo "3. Start using Hush:"
echo "   hush listen"
echo ""
echo "Hold Ctrl+Alt+V to dictate."
echo ""
print_status "Installation location: $BIN_DIR/hush"
print_status "Configuration: $CONFIG_DIR/config.toml"
