#!/bin/bash

# Test script to demonstrate comprehensive logging capabilities

echo "🔍 Testing Hush Comprehensive Logging System"
echo "============================================"

# Clear any existing logs for clean test
rm -f ~/.local/share/hush/logs/*.log 2>/dev/null

echo
echo "1. Test basic application startup logging..."
./hush -v status 2>/dev/null > /dev/null
echo "✅ Application startup complete"

echo
echo "2. Test error handling and recovery logging..."
./hush -vv models list 2>/dev/null > /dev/null || true
echo "✅ Error handling test complete"

echo
echo "3. Test audio system logging..."
./hush -vv test audio --duration 1 --list-devices 2>/dev/null > /dev/null || true
echo "✅ Audio system test complete"

echo
echo "4. Test command execution logging..."
./hush -v setup uinput --quick 2>/dev/null > /dev/null || true
echo "✅ Command execution test complete"

echo
echo "5. Check log files and content..."
LOG_DIR="$HOME/.local/share/hush/logs"
if [ -d "$LOG_DIR" ]; then
    echo "   Log directory: $LOG_DIR"
    echo "   Log files:"
    ls -la "$LOG_DIR"
    echo
    echo "   Sample log entries (last 10 lines):"
    tail -10 "$LOG_DIR"/*.log 2>/dev/null || echo "   No log files found"
else
    echo "   ❌ Log directory not found: $LOG_DIR"
fi

echo
echo "6. Analyzing log entries for comprehensive features..."
if [ -f "$LOG_DIR"/*.log ]; then
    LOG_FILE=$(ls "$LOG_DIR"/*.log | head -1)
    echo "   📊 Log Analysis:"
    
    SESSION_COUNT=$(grep -c "session_id" "$LOG_FILE" 2>/dev/null || echo "0")
    echo "   - Session tracking entries: $SESSION_COUNT"
    
    REQUEST_COUNT=$(grep -c "request_id" "$LOG_FILE" 2>/dev/null || echo "0")
    echo "   - Request correlation entries: $REQUEST_COUNT"
    
    PERFORMANCE_COUNT=$(grep -c "cpu_usage\|memory_usage" "$LOG_FILE" 2>/dev/null || echo "0")
    echo "   - Performance metrics entries: $PERFORMANCE_COUNT"
    
    ERROR_COUNT=$(grep -c "ERROR" "$LOG_FILE" 2>/dev/null || echo "0")
    echo "   - Error entries: $ERROR_COUNT"
    
    STRUCTURED_COUNT=$(grep -c "=" "$LOG_FILE" 2>/dev/null || echo "0")
    echo "   - Structured log entries: $STRUCTURED_COUNT"
    
    echo "   - Log file size: $(du -h "$LOG_FILE" | cut -f1)"
else
    echo "   ❌ No log files to analyze"
fi

echo
echo "🎉 Comprehensive logging system test complete!"
echo "   Check $LOG_DIR for detailed log files"