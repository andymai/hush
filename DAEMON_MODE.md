# 🔥 Hush Daemon Mode Guide

## What is Daemon Mode?

Daemon mode runs Hush in the **background** with a **global hotkey** that lets you record voice from anywhere on your system - no need to switch windows or click buttons!

**How it works:**
1. Press your hotkey (default: `F10`) → Recording starts
2. Release the hotkey → Recording stops, transcription begins
3. Text appears at your cursor automatically
4. Repeat whenever you need voice-to-text!

---

## Quick Start

### Option 1: Real Hardware (Recommended)

```bash
# 1. Download Whisper models first
./scripts/download-models.sh tiny

# 2. Start daemon mode
cargo run --bin hush-new -- daemon

# Output:
# 🚀 Starting Hush in daemon mode
# 📝 Instructions:
#    • Press F10 to start recording
#    • Release to stop and transcribe
#    • Text will be inserted at cursor
#    • Press Ctrl+C to quit
# 🎯 Input trigger active - waiting for events...
```

### Option 2: Test with Mocks (No Hardware)

```bash
# Test daemon mode without microphone/models
cargo run --bin hush-new -- --mock daemon

# You'll see the same interface, but with simulated components
```

---

## Configuration

Edit `~/.config/hush/config.toml` (or `config/default.toml`):

```toml
[hotkey]
enabled = true
combination = "F10"    # Your hotkey here
```

### Supported Hotkey Combinations

**Single Keys:**
```toml
combination = "F10"           # Function key
combination = "F11"
combination = "F12"
```

**With Modifiers:**
```toml
combination = "Ctrl+F10"
combination = "Shift+F10"
combination = "Alt+F10"
combination = "Super+F10"     # Windows/Super key
combination = "Ctrl+Shift+Space"
```

**Common Combinations:**
- `F10` - Simple, won't conflict with most apps
- `F11` - Usually safe (fullscreen toggle in browsers)
- `F12` - DevTools in browsers, be careful
- `Ctrl+Shift+Space` - Push-to-talk feel
- `Alt+Space` - Easy to reach
- `Super+V` - "V" for voice

---

## Usage Examples

### Basic Workflow

```bash
# Start daemon
cargo run --bin hush-new -- daemon

# Then in any application:
# 1. Click where you want text to appear
# 2. Press F10 (hold)
# 3. Speak: "Hello, this is a test"
# 4. Release F10
# 5. Wait 1-2 seconds
# 6. Text appears: "Hello, this is a test"
```

### With Verbose Logging

```bash
# See what's happening under the hood
cargo run --bin hush-new -- -vv daemon

# Output shows:
# - Hotkey events (press/release)
# - Audio buffer details
# - Transcription progress
# - Text insertion status
```

### With Custom Config

```bash
# Use your own config file
cargo run --bin hush-new -- -c ~/my-hush-config.toml daemon
```

### Without Notifications

```bash
# Silent mode - no desktop notifications
cargo run --bin hush-new -- --no-notifications daemon
```

---

## How It Works Internally

**Architecture:**

```
┌─────────────────────────────────────────┐
│         Daemon Mode Event Loop          │
├─────────────────────────────────────────┤
│                                         │
│  1. HotkeyTriggerAdapter listens for    │
│     global hotkey events                │
│                                         │
│  2. On PRESS → StartRecording event     │
│     ├─ Start audio capture              │
│     └─ State: Idle → Recording          │
│                                         │
│  3. On RELEASE → StopRecording event    │
│     ├─ Stop audio capture               │
│     ├─ Send to Whisper                  │
│     ├─ State: Recording → Transcribing  │
│     ├─ Get transcribed text             │
│     ├─ State: Transcribing → Inserting  │
│     ├─ Insert text at cursor            │
│     └─ State: Inserting → Idle          │
│                                         │
│  4. Repeat from step 1                  │
│                                         │
└─────────────────────────────────────────┘
```

**State Transitions:**

```
Idle → (hotkey press) → Recording
Recording → (hotkey release) → Transcribing
Transcribing → (done) → Inserting
Inserting → (done) → Idle
```

---

## Troubleshooting

### "Failed to register global hotkey"

**Common on GNOME** due to security restrictions.

**Solutions:**

1. **Use a different hotkey** (try F10, F11, or F12):
   ```toml
   combination = "F10"  # More likely to work
   ```

2. **Use Manual Mode instead**:
   ```bash
   cargo run --bin hush-new -- manual
   ```

3. **Set up GNOME Custom Shortcut** (most reliable):
   ```bash
   # Build release binary first
   cargo build --release --bin hush-new

   # Then in GNOME:
   # Settings → Keyboard → Custom Shortcuts
   # Command: /path/to/hush-new one-shot --duration 10
   # Shortcut: Ctrl+Shift+Space
   ```

### "Hotkey doesn't respond"

**Check if hotkey is already taken:**
```bash
# Try different keys
cargo run --bin hush-new -- daemon
# Edit config to try F10, F11, F12

# Check system shortcuts
# Settings → Keyboard → View and Customize Shortcuts
```

**Try with verbose logging:**
```bash
cargo run --bin hush-new -- -vv daemon
# Watch for hotkey events in the log
```

### "Daemon exits immediately"

**Possible causes:**
1. Hotkey registration failed
2. Audio device not available
3. Model files not found

**Debug:**
```bash
# Run with verbose logging
cargo run --bin hush-new -- -vv daemon

# Check system info
cargo run --bin hush-new -- info

# Test components individually
cargo run --bin test-audio
```

### "Text appears in wrong place"

**X11 Focus Issues:**

The text is inserted at the **currently focused window**. Make sure:
1. Click in the target application first
2. Wait for window to have focus
3. Then press your hotkey

**Workaround for terminals:**
- Use `--print-only` and pipe output
- Or use Manual mode with explicit paste

---

## Advanced Usage

### Running as a System Service

```bash
# 1. Build release binary
cargo build --release --bin hush-new

# 2. Create systemd user service
cat > ~/.config/systemd/user/hush.service <<EOF
[Unit]
Description=Hush Voice-to-Text Daemon
After=graphical-session.target

[Service]
Type=simple
ExecStart=/home/YOUR_USER/Git/hush/target/release/hush-new daemon
Restart=on-failure

[Install]
WantedBy=default.target
EOF

# 3. Enable and start
systemctl --user daemon-reload
systemctl --user enable hush.service
systemctl --user start hush.service

# 4. Check status
systemctl --user status hush.service
```

### Auto-start on Login

```bash
# Enable the service to start on login
systemctl --user enable hush.service

# Disable auto-start
systemctl --user disable hush.service
```

### View Daemon Logs

```bash
# Follow live logs
journalctl --user -u hush.service -f

# View recent logs
journalctl --user -u hush.service -n 50
```

---

## Performance Tips

### CPU Usage
- Daemon is **idle** until you press the hotkey
- Recording uses ~5-10% CPU
- Transcription spikes to 50-100% CPU (brief, 1-2 seconds)
- With GPU: Transcription uses minimal CPU

### Memory Usage
- Base: ~50-100 MB (tiny model)
- Recording: +10-20 MB
- Transcription: +50-100 MB (temporary spike)

### Battery Impact
- Idle: Negligible (0%)
- Active use: Moderate (transcription is CPU intensive)
- Use **tiny model** for battery savings
- Use **GPU** if available for efficiency

---

## Comparison: Daemon vs Manual Mode

| Feature | Daemon Mode | Manual Mode |
|---------|-------------|-------------|
| **Convenience** | ⭐⭐⭐⭐⭐ Always ready | ⭐⭐⭐ Need to switch to terminal |
| **Setup** | ⚠️ May need troubleshooting | ✅ Always works |
| **System Impact** | 🔵 Runs in background | 🟢 Only when needed |
| **Hotkey Support** | ✅ Global system hotkey | ❌ Uses Enter key |
| **Reliability** | ⚠️ Depends on system | ✅ Very reliable |
| **Best For** | Power users, frequent use | Testing, restricted systems |

**Recommendation:**
- **Try Daemon first** - if it works, it's the best experience
- **Fall back to Manual** - if hotkeys don't work on your system
- **Use One-Shot** - for GNOME with custom shortcuts

---

## Example Workflows

### Workflow 1: Coding Assistant
```bash
# Start daemon in morning
cargo run --release --bin hush-new -- daemon

# Throughout the day:
# 1. Writing code comments
#    - F10 (hold) → "This function validates user input" → Release
#    - Text appears as comment

# 2. Writing commit messages
#    - F10 (hold) → "Fix authentication bug in login handler" → Release
#    - Text appears in git commit editor
```

### Workflow 2: Email/Messages
```bash
# Open email client
# Click in message body
# F10 (hold) → "Hi John, thanks for reaching out..." → Release
# Text appears, ready to edit
```

### Workflow 3: Documentation
```bash
# Writing docs in editor
# F10 (hold) → "Installation requires the following dependencies" → Release
# Continue typing to edit/refine
```

---

## Best Practices

### ✅ Do:
- Test with `--mock` first to verify setup
- Use `F10` or `F11` for maximum compatibility
- Keep model files downloaded and updated
- Test in different apps to understand behavior
- Use verbose logging (`-vv`) when debugging

### ❌ Don't:
- Don't use common shortcuts (Ctrl+C, Ctrl+V, etc.)
- Don't expect instant results (transcription takes 1-2 seconds)
- Don't use in noisy environments (affects accuracy)
- Don't forget to click in target window first

---

## FAQ

**Q: Can I use multiple hotkeys?**
A: Not currently. You can only configure one hotkey combination.

**Q: Does it work on Wayland?**
A: Partially. Hotkey support is limited on Wayland. Text insertion works. Use Manual mode for best results.

**Q: Can I customize the hotkey behavior?**
A: Yes! Edit the `HotkeyTriggerAdapter` or use the config file to change the key combination.

**Q: Does it interfere with games?**
A: It can. Disable the daemon before gaming, or use a function key (F10-F12) that games rarely use.

**Q: How do I stop the daemon?**
A: Press `Ctrl+C` in the terminal, or use `systemctl --user stop hush.service` if running as a service.

**Q: Can I run multiple instances?**
A: No, hotkey registration will fail for the second instance.

---

## Summary

**Daemon Mode** is the most powerful way to use Hush:

✅ **Pros:**
- Always available with global hotkey
- No window switching needed
- Seamless integration into workflow
- Perfect for power users

⚠️ **Cons:**
- Hotkey registration can fail (especially GNOME)
- Runs in background (uses some resources)
- May conflict with other apps

**If daemon mode doesn't work**, use **Manual mode** - it's just as functional, just requires terminal access.

---

**Ready to try?**

```bash
cargo run --bin hush-new -- daemon
```

Press `F10` and start talking! 🎙️
