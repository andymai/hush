# macOS Troubleshooting Guide

Common issues and solutions for Hush on macOS.

## Installation Issues

### "xcode-select: command not found"

Install Xcode Command Line Tools:
```bash
xcode-select --install
```

### "cargo: command not found"

Install Rust:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### Build fails with "framework not found"

```bash
# Install Xcode Command Line Tools
xcode-select --install

# Reset if already installed
sudo xcode-select --reset
```

## Permission Issues

### Text insertion doesn't work

See [PERMISSIONS.md](PERMISSIONS.md) for detailed steps.

Quick fix:
```bash
hush check-permissions
# Follow instructions, then RESTART Hush
```

### Microphone access denied

1. System Settings → Privacy & Security → Microphone
2. Enable "hush" or Terminal app
3. Restart Hush

## Performance Issues

### Slow transcription on Apple Silicon

**Expected:** ~120ms first token
**If slower:** Check GPU detection

```bash
hush status --devices
# Should show: "Metal GPU: Apple Silicon"
```

**If shows CPU:**
```bash
# Rebuild with Metal feature
cargo build --release --features metal
```

### Slow transcription on Intel Mac

**Expected:** ~800ms (CPU-only)
**This is normal** - Intel Macs don't benefit from Metal GPU.

**Options:**
- Use smaller model: `tiny.en` (~500ms)
- Upgrade to Apple Silicon Mac
- Accept slower performance

### High CPU usage when idle

**Expected:** <1% when not recording

**If higher:**
1. Check for background processes: `ps aux | grep hush`
2. Check logs: `tail -f ~/Library/Logs/hush/hush.log`
3. Report issue with logs attached

## Hotkey Issues

### Hotkey not triggering

**Symptoms:**
- Press Cmd+Alt+V, nothing happens
- No overlay appears

**Solutions:**

1. **Check hotkey conflicts:**
   ```bash
   # Some apps reserve Cmd+Alt+V
   # Try different combo in config:
   open ~/.config/hush/config.toml
   # Change: hotkey = "Cmd+Shift+V"
   ```

2. **Check logs for errors:**
   ```bash
   tail -f ~/Library/Logs/hush/hush.log
   # Look for "hotkey" related errors
   ```

3. **Restart Hush:**
   ```bash
   pkill hush
   hush listen
   ```

4. **Check macOS hotkey access:**
   - Some security software blocks global hotkeys
   - Verify no other app uses same combo

### Hotkey works sporadically

**Possible cause:** Main thread issue (rare)

**Solution:**
```bash
# Check Hush logs
tail -100 ~/Library/Logs/hush/hush.log | grep -i "thread\|hotkey"

# Report issue with logs
```

## Audio Issues

### "No input device found"

**Check:**
```bash
# List audio devices
hush status --devices

# Check system default
# System Settings → Sound → Input
```

**Solution:**
- Ensure microphone is connected and selected
- Grant Microphone permission (see PERMISSIONS.md)
- Restart Hush

### Audio cutting out during recording

**Possible causes:**
- Buffer overflow (too much audio)
- CPU throttling
- Microphone hardware issue

**Solutions:**
1. Use shorter recordings (<30 seconds)
2. Check CPU usage: `top | grep hush`
3. Test microphone in other apps
4. Check logs for "audio" errors

### Background noise transcribed

**This is expected** - Whisper transcribes all audio.

**Solutions:**
- Use push-to-talk mode (default)
- Speak clearly and close to mic
- Use noise-canceling microphone
- Adjust audio sensitivity (future feature)

## System Tray Issues

### Tray icon not appearing

**Check:**
```bash
# Ensure system-tray feature enabled
cargo build --release --features system-tray
```

**Solutions:**
1. Restart Hush
2. Check menu bar isn't full (hide other icons)
3. Check logs: `grep -i "tray" ~/Library/Logs/hush/hush.log`

### Tray icon stuck in wrong state

**Example:** Shows "recording" but not actually recording

**Solution:**
```bash
# Restart Hush
pkill hush
hush listen
```

## Model Issues

### "Model not found" error

```bash
# Download model
hush models download base.en

# Verify download
ls -lh ~/.local/share/hush/models/
```

### Model download stuck or slow

**Use alternative download:**
```bash
# Set Hugging Face endpoint
export HF_ENDPOINT=https://huggingface.co

# Download manually
hush models download base.en --force
```

### Model loading is slow

**Expected:**
- First load: 2-5 seconds (model loads into RAM/GPU)
- Subsequent: <1 second (cached)

**If slower:**
- Check available RAM: `vm_stat`
- Close memory-intensive apps
- Use smaller model: `tiny.en`

## Crash Issues

### App crashes on start

**Collect crash logs:**
```bash
# macOS crash logs location
ls -lt ~/Library/Logs/DiagnosticReports/ | grep hush | head -5

# View latest crash log
cat "$(ls -t ~/Library/Logs/DiagnosticReports/hush* | head -1)"
```

**Common causes:**
1. Missing permissions → Grant Accessibility
2. Corrupted model → Re-download model
3. Incompatible macOS version → Check requirements

**Report with:**
- Crash log (redact any sensitive info)
- `hush --version`
- `sw_vers` output
- Steps to reproduce

### App freezes during transcription

**Symptoms:**
- UI unresponsive
- High CPU usage

**Emergency recovery:**
```bash
# Force quit
pkill -9 hush

# Check logs
tail -100 ~/Library/Logs/hush/hush.log

# Restart with debug logging
RUST_LOG=debug hush listen 2>&1 | tee hush-debug.log
```

## Configuration Issues

### Config file not found

**Expected location:** `~/.config/hush/config.toml`

**Create default:**
```bash
mkdir -p ~/.config/hush
# Hush will create default config on next run
```

### Config changes not taking effect

**Solution:**
```bash
# Restart Hush after config changes
pkill hush
hush listen
```

### Reset to defaults

```bash
# Backup current config
cp ~/.config/hush/config.toml ~/.config/hush/config.toml.backup

# Delete config (will regenerate on next run)
rm ~/.config/hush/config.toml

# Restart
hush listen
```

## Known Issues

### Issue: Secure Input Mode

**Symptom:** Text insertion doesn't work in password fields

**Status:** Expected behavior
**Reason:** macOS blocks text insertion in secure input fields for security
**Workaround:** Type passwords manually

### Issue: Sandboxed Apps

**Symptom:** Some apps don't receive transcribed text

**Apps affected:** Some Mac App Store apps with strict sandboxing

**Workaround:** Use native (non-sandboxed) versions of apps

### Issue: Virtual Machines

**Symptom:** Permissions don't work in VMs (Parallels, VMware, etc.)

**Status:** Limited support
**Reason:** VMs have restricted access to macOS accessibility APIs

## Getting More Help

1. **Check logs:**
   ```bash
   tail -100 ~/Library/Logs/hush/hush.log
   ```

2. **Search existing issues:**
   https://github.com/andymai/hush/issues

3. **Create new issue:**
   Include:
   - macOS version: `sw_vers`
   - Hush version: `hush --version`
   - CPU architecture: `uname -m`
   - Logs (redact sensitive info)
   - Steps to reproduce

4. **Community support:**
   - Discussions: https://github.com/andymai/hush/discussions
