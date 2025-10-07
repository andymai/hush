#!/bin/bash

# Hush Manual Mode - Continuous Recording Script
# This provides a daemon-like experience using manual mode

echo "🤫 Hush Voice-to-Text (Manual Mode)"
echo "========================================="
echo ""
echo "🎯 Features:"
echo "   • Audio: PulseAudio (no ALSA errors)"
echo "   • Recording: Press Enter to start/stop"
echo "   • Transcription: Automatic after recording"
echo "   • Text insertion: Direct to cursor position"
echo ""
echo "⌨️  Usage:"
echo "   • Press Enter to start recording"
echo "   • Press Enter again to stop and transcribe"
echo "   • Text appears where your cursor is"
echo "   • Type 'q' + Enter to quit"
echo ""
echo "🚀 Starting Hush..."
echo ""

# Change to project directory
cd "$(dirname "$0")"

# Continuous loop for multiple recordings
while true; do
    echo "────────────────────────────────────────"
    echo "Ready for voice input..."
    echo ""
    
    # Start Hush in manual mode
    ./target/debug/hush-mvp --config ~/.config/hush/config.toml manual
    
    # Check exit status
    exit_code=$?
    
    # If user quit normally, exit
    if [ $exit_code -eq 0 ]; then
        echo ""
        echo "👋 Thanks for using Hush!"
        break
    fi
    
    # If there was an error, show it and continue
    if [ $exit_code -ne 0 ]; then
        echo ""
        echo "⚠️  Hush exited with error code $exit_code"
        echo "Restarting in 2 seconds..."
        sleep 2
    fi
done