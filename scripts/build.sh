#!/bin/bash

# Hush Build Script

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Set PKG_CONFIG_PATH to include system paths
export PKG_CONFIG_PATH="/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig:/usr/lib/pkgconfig:${PKG_CONFIG_PATH:-}"

echo -e "${BLUE}Building Hush Voice-to-Text${NC}"
echo "PKG_CONFIG_PATH: $PKG_CONFIG_PATH"
echo

cd "$PROJECT_ROOT"

# Parse command line arguments
BUILD_MODE="debug"

while [[ $# -gt 0 ]]; do
    case $1 in
        --release)
            BUILD_MODE="release"
            shift
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--release]"
            exit 1
            ;;
    esac
done

# Build command
if [[ "$BUILD_MODE" == "release" ]]; then
    echo -e "${BLUE}Building in release mode...${NC}"
    cargo build --release
else
    echo -e "${BLUE}Building in debug mode...${NC}"
    cargo build
fi

if [[ $? -eq 0 ]]; then
    echo
    echo -e "${GREEN}Build completed successfully!${NC}"
    echo
    echo "Binary location:"
    if [[ "$BUILD_MODE" == "release" ]]; then
        ls -la target/release/hush 2>/dev/null || true
        echo
        echo "To run:"
        echo "  ./target/release/hush --help"
    else
        ls -la target/debug/hush 2>/dev/null || true
        echo
        echo "To run:"
        echo "  ./target/debug/hush --help"
    fi
else
    echo -e "${RED}Build failed${NC}"
    exit 1
fi
