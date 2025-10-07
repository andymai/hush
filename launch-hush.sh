#!/bin/bash

# Hush Voice-to-Text Launcher Script
# Fixes audio device configuration and hotkey conflicts

echo "🤫 Starting Hush Voice-to-Text..."

# Change to the project directory
cd "$(dirname "$0")"

# Check if the binary exists
if [ ! -f "./target/debug/hush-mvp" ]; then
    echo "❌ Hush binary not found. Please build it first with: cargo build"
    exit 1
fi

# Check if config exists
if [ ! -f "$HOME/.config/hush/config.toml" ]; then
    echo "❌ Configuration file not found at ~/.config/hush/config.toml"
    echo "Please ensure the config file exists with proper audio and hotkey settings."
    exit 1
fi

echo "📋 Configuration:"
echo "   • Audio: PulseAudio (avoids ALSA errors)"  
echo "   • Hotkey: Ctrl+Shift+Space (avoids F10 conflicts)"
echo "   • Config: ~/.config/hush/config.toml"
echo ""

# Offer mode selection
echo "Select mode:"
echo "  [1] Manual mode (Press Enter to record, RECOMMENDED)"
echo "  [2] Daemon mode (Currently has hotkey event loop issues)"
echo ""
echo "⚠️  Note: Daemon mode has a known issue where the hotkey event loop"
echo "    immediately disconnects. Manual mode works perfectly for now."
echo ""
echo -n "Choice [1/2]: "
read choice

case $choice in
    1)
        echo ""
        echo "🔧 Starting in Manual Mode..."
        echo "   • Press Enter to start recording"
        echo "   • Press Enter again to stop and transcribe" 
        echo "   • Type 'q' and press Enter to quit"
        echo ""
        ./target/debug/hush-mvp --config ~/.config/hush/config.toml manual
        ;;
    2)
        echo ""
        echo "🚀 Starting in Daemon Mode..."
        echo "   • Press Ctrl+Shift+Space to record"
        echo "   • Release Ctrl+Shift+Space to transcribe"
        echo "   • Press Ctrl+C to quit"
        echo ""
        ./target/debug/hush-mvp --config ~/.config/hush/config.toml daemon
        ;;
    *)
        echo "Invalid choice. Defaulting to manual mode..."
        ./target/debug/hush-mvp --config ~/.config/hush/config.toml manual
        ;;
esac