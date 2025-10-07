#!/bin/bash

# Hush Build Script
# Handles PKG_CONFIG_PATH issues on systems with Homebrew

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Set PKG_CONFIG_PATH to include system paths
# This fixes issues where Homebrew's pkg-config doesn't find system libraries like ALSA
export PKG_CONFIG_PATH="/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig:/usr/lib/pkgconfig:${PKG_CONFIG_PATH:-}"

echo -e "${BLUE}🔨 Building Hush Voice-to-Text Application${NC}"
echo "PKG_CONFIG_PATH: $PKG_CONFIG_PATH"
echo

cd "$PROJECT_ROOT"

# Parse command line arguments
BUILD_MODE="debug"
TARGET=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --release)
            BUILD_MODE="release"
            shift
            ;;
        --bin)
            TARGET="--bin $2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--release] [--bin binary_name]"
            exit 1
            ;;
    esac
done

# Build command
if [[ "$BUILD_MODE" == "release" ]]; then
    echo -e "${BLUE}Building in release mode...${NC}"
    cargo build --release $TARGET
else
    echo -e "${BLUE}Building in debug mode...${NC}"
    cargo build $TARGET
fi

if [[ $? -eq 0 ]]; then
    echo
    echo -e "${GREEN}✅ Build completed successfully!${NC}"
    echo
    echo "Available binaries:"
    if [[ "$BUILD_MODE" == "release" ]]; then
        ls -la target/release/hush* 2>/dev/null || true
        ls -la target/release/simple-model-manager 2>/dev/null || true
        ls -la target/release/model-manager 2>/dev/null || true
    else
        ls -la target/debug/hush* 2>/dev/null || true
        ls -la target/debug/simple-model-manager 2>/dev/null || true
        ls -la target/debug/model-manager 2>/dev/null || true
    fi
    echo
    echo "To run the MVP:"
    if [[ "$BUILD_MODE" == "release" ]]; then
        echo "  ./target/release/hush-mvp"
    else
        echo "  ./target/debug/hush-mvp"
    fi
else
    echo -e "${RED}❌ Build failed${NC}"
    exit 1
fi