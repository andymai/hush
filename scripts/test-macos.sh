#!/bin/bash
# Manual macOS testing script
# Run this on a real macOS machine with display and permissions

set -e

echo "🍎 macOS Integration Testing"
echo ""

# Check platform
if [[ "$OSTYPE" != "darwin"* ]]; then
    echo "❌ This script must run on macOS"
    exit 1
fi

# Check architecture
ARCH=$(uname -m)
echo "📋 System Information:"
echo "   Architecture: $ARCH"
echo "   macOS Version: $(sw_vers -productVersion)"
echo ""

# Detect build features
FEATURES=""
if [[ "$ARCH" == "arm64" ]] || [[ "$ARCH" == "aarch64" ]]; then
    echo "🔧 Apple Silicon detected - will build with Metal support"
    FEATURES="--features metal"
else
    echo "🔧 Intel Mac detected - CPU-only build"
    FEATURES="--no-default-features"
fi

# Build the project
echo ""
echo "1️⃣  Building project..."
cargo build $FEATURES
echo "✅ Build complete"

# Check for Accessibility permissions (if binary exists)
echo ""
echo "2️⃣  Checking permissions..."
if [ -f ./target/debug/hush ]; then
    # Note: check-permissions command may not exist yet
    # This is a placeholder for when it's implemented
    if ./target/debug/hush check-permissions 2>/dev/null; then
        echo "✅ Permission check complete"
    else
        echo "⚠️  Permission check command not available"
        echo "   Accessibility: Settings → Privacy & Security → Accessibility"
        echo "   Microphone: Will be prompted on first use"
    fi
else
    echo "⚠️  Binary not found, skipping permission check"
fi

# Run unit tests
echo ""
echo "3️⃣  Running unit tests..."
cargo test --lib $FEATURES
echo "✅ Unit tests passed"

# Run macOS-specific integration tests
echo ""
echo "4️⃣  Running macOS integration tests..."
echo "   (Some tests may be skipped without permissions/display)"
cargo test --test macos_integration $FEATURES -- --test-threads=1 --nocapture || true
echo "✅ Integration tests complete"

# Run ignored tests (require permissions)
echo ""
echo "5️⃣  Running permission-required tests..."
echo "   (These may fail without Accessibility permissions)"
cargo test --test macos_integration $FEATURES -- --ignored --test-threads=1 --nocapture || true

# Interactive tests
echo ""
echo "6️⃣  Interactive Tests"
echo "   ==================="
echo ""

# Test device detection
echo "📊 GPU Device Detection:"
if ./target/debug/hush device-info 2>/dev/null; then
    echo "✅ Device detection working"
else
    echo "⚠️  Device info command not available"
fi

# Test system tray (interactive)
echo ""
echo "🎨 System Tray Test:"
echo "   Starting application in background..."
echo "   Check for tray icon in menu bar (top-right)"
echo ""
echo "   Press Enter to start, Ctrl+C to stop"
read -p "   Ready? "

./target/debug/hush listen &
HUSH_PID=$!

echo "   Hush is running (PID: $HUSH_PID)"
echo "   Check menu bar for microphone icon"
echo "   Click it to see the menu"
echo ""
echo "   Press Enter when done testing..."
read

kill $HUSH_PID 2>/dev/null || true
echo "✅ System tray test complete"

# Test hotkey (interactive)
echo ""
echo "🔥 Hotkey Test:"
echo "   Press Cmd+Alt+V to test hotkey"
echo "   (Default hotkey, may be different in your config)"
echo ""
echo "   Starting Hush in listen mode..."
echo "   Press the hotkey, then press Ctrl+C to stop"
echo ""
read -p "   Ready? "

./target/debug/hush listen || true
echo "✅ Hotkey test complete"

# Test text insertion (interactive)
echo ""
echo "✍️  Text Insertion Test:"
echo "   1. Open TextEdit or any text editor"
echo "   2. Focus the editor window"
echo "   3. We'll start Hush in listen mode"
echo "   4. Press the hotkey (Cmd+Alt+V)"
echo "   5. Speak something like 'Hello world'"
echo "   6. Verify text appears in the editor"
echo ""
read -p "   Ready? Press Enter to start... "

./target/debug/hush listen || true
echo "✅ Text insertion test complete"

# Summary
echo ""
echo "=" | tr "=" "=" | head -c 60; echo ""
echo "✅ All tests complete!"
echo ""
echo "Summary:"
echo "  ✅ Build successful"
echo "  ✅ Unit tests passed"
echo "  ✅ Integration tests ran"
echo "  ✅ Manual tests completed"
echo ""
echo "If any interactive tests failed:"
echo "  - Check Accessibility permissions"
echo "  - Check Microphone permissions"
echo "  - See docs/macos/TROUBLESHOOTING.md"
echo ""
