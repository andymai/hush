# macOS Permissions Guide

Hush requires specific permissions to function properly on macOS. This guide explains what permissions are needed and why.

## Required Permissions

### 1. Microphone Access

**Why:** To capture your voice for transcription.

**When:** Requested automatically on first run.

**How to grant:**
1. macOS will show a prompt: "hush would like to access the microphone"
2. Click "OK"

**If denied:**
1. System Settings → Privacy & Security → Microphone
2. Find "hush" or your terminal app in the list
3. Enable the checkbox

**Verify:**
```bash
hush check-permissions
# Should show: "✅ Microphone: Accessible"
```

### 2. Accessibility Access

**Why:** To insert transcribed text into any application.

**When:** Required before first use, not auto-prompted.

**How to grant:**
1. Run: `hush check-permissions`
2. Follow the printed instructions
3. System Settings → Privacy & Security → Accessibility
4. Click the lock icon to make changes
5. Click the "+" button
6. Navigate to `/usr/local/bin/hush` (or `Terminal.app` if running from terminal)
7. Select and click "Open"
8. Enable the checkbox for the app

**Important:**
- You must restart Hush after granting Accessibility permissions
- If running from a terminal, add the terminal app (Terminal, iTerm2, etc.) to Accessibility

**Verify:**
```bash
hush check-permissions
# Should show: "✅ Accessibility: Granted"
```

## Permission Troubleshooting

### "Text insertion not working"

**Symptoms:**
- Hotkey triggers
- Recording completes
- No text appears

**Solution:**
1. Check Accessibility permissions: `hush check-permissions`
2. Ensure checkbox is enabled in System Settings
3. **Restart Hush** after granting permissions
4. If using terminal: Add terminal app to Accessibility, not just `hush` binary

### "Microphone not detected"

**Symptoms:**
- Error: "No input device found"
- Silent recording

**Solution:**
1. Check Microphone permission in System Settings
2. Verify microphone hardware is working (System Settings → Sound → Input)
3. Check `hush status --devices` shows input device

### "Permission keeps getting revoked"

**Possible causes:**
1. App signature changed (after rebuilding from source)
2. macOS security update reset permissions
3. App moved to different location

**Solution:**
1. Remove app from Accessibility list
2. Re-add using steps above
3. If building from source, consider code signing

## Privacy & Security

### What Hush Accesses

- ✅ Microphone audio (only when hotkey pressed)
- ✅ Active window information (for text insertion context)
- ✅ Keyboard simulation (to insert text)

### What Hush Does NOT Access

- ❌ Other app data or files
- ❌ Network (all processing is local)
- ❌ Location
- ❌ Contacts, calendar, photos, etc.
- ❌ System monitoring or surveillance

### Local-First Privacy

All audio processing happens **locally on your Mac**:
- Audio never leaves your device
- No cloud API calls
- No telemetry or analytics
- Whisper models run entirely offline

## Permission Management

### Revoking Permissions

To revoke at any time:
1. System Settings → Privacy & Security
2. Select Microphone or Accessibility
3. Disable checkbox for Hush

**Note:** Hush will not function without these permissions.

### Verifying Current Status

```bash
hush check-permissions
```

Output example:
```
🔐 macOS Permission Status

✅ Accessibility: Granted
   Required for text insertion

✅ Microphone: Accessible
   Required for audio capture
```

## Advanced: Code Signing

If you build from source frequently, consider code signing to prevent permission resets:

```bash
# Sign the binary (requires Apple Developer account)
codesign --force --deep --sign "Your Developer ID" target/release/hush
```

## Further Reading

- [Apple: Privacy and Security](https://support.apple.com/guide/mac-help/mh11389/mac)
- [macOS Security Documentation](https://developer.apple.com/documentation/security)
- [Installation Guide](INSTALL.md)
- [Troubleshooting Guide](TROUBLESHOOTING.md)
