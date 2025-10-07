#!/bin/bash

# Hush Voice-to-Text Installation Script
# This script builds and installs Hush as a production-ready application

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_header() {
    echo -e "${BLUE}$1${NC}"
}

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || [ ! -d "src" ]; then
    print_error "This script must be run from the Hush project root directory"
    exit 1
fi

print_header "🤫 Hush Voice-to-Text Installation Script"
print_header "========================================"

# Check dependencies
print_status "Checking system dependencies..."

# Check for Rust/Cargo
if ! command -v cargo &> /dev/null; then
    print_error "Cargo is not installed. Please install Rust from https://rustup.rs/"
    exit 1
fi

# Check for required system packages
missing_packages=()

if ! pkg-config --exists alsa; then
    missing_packages+=("libasound2-dev")
fi

if [ ${#missing_packages[@]} -ne 0 ]; then
    print_error "Missing required system packages: ${missing_packages[*]}"
    print_status "Install them with: sudo apt install ${missing_packages[*]}"
    exit 1
fi

print_status "All system dependencies are available"

# Build the application
print_status "Building Hush in release mode..."
./scripts/build.sh --release --bin hush-mvp

if [ $? -ne 0 ]; then
    print_error "Build failed"
    exit 1
fi

# Create installation directories
print_status "Setting up installation directories..."
BIN_DIR="$HOME/.local/bin"
mkdir -p "$BIN_DIR"

# Install the binary
print_status "Installing hush binary to $BIN_DIR..."
cp target/release/hush-mvp "$BIN_DIR/hush"
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
    print_status "✅ Binary installation successful"
else
    print_error "Binary installation failed - hush command not working"
    exit 1
fi

# Install desktop integration
print_status "Installing desktop integration..."
"$BIN_DIR/hush" install

# Create configuration directory
print_status "Setting up configuration..."
CONFIG_DIR="$HOME/.config/hush"
mkdir -p "$CONFIG_DIR"

# Copy default configuration if it doesn't exist
if [ ! -f "$CONFIG_DIR/config.toml" ]; then
    if [ -f "config/default.toml" ]; then
        cp "config/default.toml" "$CONFIG_DIR/config.toml"
        print_status "Created default configuration at $CONFIG_DIR/config.toml"
    else
        print_warning "Default configuration file not found, skipping config setup"
    fi
fi

# Download a basic Whisper model if models directory doesn't exist
MODELS_DIR="$PWD/models"
if [ ! -d "$MODELS_DIR" ] || [ -z "$(ls -A "$MODELS_DIR")" ]; then
    print_status "Setting up Whisper models..."
    ./scripts/download-models.sh tiny
fi

print_header ""
print_header "🎉 Installation Complete!"
print_header "========================"
print_status "Hush has been installed successfully!"
echo ""
print_status "Available commands:"
echo "  hush daemon           - Run with global hotkeys (requires setup)"
echo "  hush manual           - Manual recording mode (no hotkeys needed)"
echo "  hush one-shot         - Single recording mode (for GNOME shortcuts)"
echo "  hush status           - Show system status"
echo "  hush install          - Setup desktop integration"
echo ""
print_status "Next steps:"
echo "1. 🎹 Set up keyboard shortcut in GNOME Settings (recommended)"
echo "   - Open Settings → Keyboard → Keyboard Shortcuts"
echo "   - Add custom shortcut: 'hush one-shot --duration 10'"
echo "   - Assign to Ctrl+Shift+Space"
echo ""
echo "2. 🔧 Or try manual mode: hush manual"
echo ""
echo "3. 📊 Check status anytime: hush status"
echo ""
print_status "For troubleshooting, run: hush status -v"

# Check GNOME environment
if [ "$XDG_CURRENT_DESKTOP" = "GNOME" ]; then
    print_status ""
    print_status "🔧 GNOME detected! You can now:"
    echo "   - Open Settings → Keyboard → Keyboard Shortcuts"
    echo "   - Find 'Custom Shortcuts' and add a new one"
    echo "   - Command: hush one-shot --duration 10"
    echo "   - Shortcut: Ctrl+Shift+Space"
fi

print_status ""
print_status "Installation location: $BIN_DIR/hush"
print_status "Configuration: $CONFIG_DIR/config.toml"
print_status "Models: $MODELS_DIR/"