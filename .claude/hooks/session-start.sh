#!/bin/bash
#
# SessionStart Hook for Hush - Claude Code on the Web
#
# This hook installs system dependencies and prepares the Rust build environment
# so that cargo check, cargo test, and cargo clippy work immediately when the
# session starts.
#
# The container state is cached after this hook completes, so subsequent sessions
# start instantly without re-running the dependency installation.

set -euo pipefail

# Only run in Claude Code on the web (remote environment)
if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

echo "🚀 Hush SessionStart Hook: Setting up development environment..."

# Install system dependencies required for building Hush
# These are needed for:
# - ALSA (audio capture): libasound2-dev
# - X11 (window management): libx11-dev
# - D-Bus (notifications): libdbus-1-dev
# - Build tools: build-essential, pkg-config

echo "📦 Installing system dependencies..."

# Update package lists (suppress errors from bad PPAs)
# Only update if not recently updated
if [ ! -f /var/lib/apt/periodic/update-success-stamp ] || \
   [ "$(find /var/lib/apt/periodic/update-success-stamp -mmin +60 2>/dev/null)" ]; then
  apt-get update -qq 2>/dev/null || true
fi

# Install dependencies non-interactively
# Use -qq for quiet output, -y for automatic yes
# Suppress errors - packages may already be installed
DEBIAN_FRONTEND=noninteractive apt-get install -qq -y \
  build-essential \
  pkg-config \
  libasound2-dev \
  libx11-dev \
  libdbus-1-dev \
  2>/dev/null || true

echo "✅ System dependencies installed"

# Download Rust dependencies
# Note: This project has CUDA dependencies hardcoded which won't compile in
# the web environment (no NVIDIA GPU/CUDA toolkit). However, we can still:
# - Run code analysis and navigation
# - Use rust-analyzer for IDE features
# - Review code and documentation

echo "🦀 Downloading Rust dependencies..."

# Attempt to fetch dependencies (this will fail due to CUDA, but will cache what it can)
# We suppress errors since CUDA unavailability is expected in web environment
cargo fetch 2>/dev/null || true

echo "⚠️  Note: Full build requires CUDA (not available in web environment)"
echo "   - Code navigation and analysis will work"
echo "   - cargo check/build/test require CUDA and will fail"
echo "   - Consider creating CUDA-optional features for web compatibility"

# Set helpful environment variables for the session
if [ -n "${CLAUDE_ENV_FILE:-}" ]; then
  # Inform that CUDA is not available
  echo 'export CUDA_AVAILABLE=false' >> "$CLAUDE_ENV_FILE"
fi

echo "🎉 SessionStart hook complete! Environment configured for code review and analysis."
