# T-040: Create macOS Installation and User Documentation

**Priority:** Medium
**Effort:** Low-Medium (3-4 hours)
**Type:** Documentation
**Status:** Available
**Created:** 2025-11-19

---

## Problem Statement

Current documentation is Linux-focused. Need comprehensive macOS documentation covering:
- Installation instructions
- System requirements
- Permission setup (Accessibility, Microphone)
- Building from source with Metal GPU support
- Troubleshooting common issues
- Performance expectations

---

## Goals

1. ✅ Create `INSTALL-MACOS.md` with detailed installation steps
2. ✅ Update main `README.md` with macOS support
3. ✅ Add `TROUBLESHOOTING-MACOS.md` for common issues
4. ✅ Document permission requirements clearly
5. ✅ Add Metal GPU build instructions
6. ✅ Include performance benchmarks
7. ✅ Add screenshots/examples

---

## Documentation Structure

### File Layout

```
docs/
├── macos/
│   ├── INSTALL.md           # Installation guide
│   ├── PERMISSIONS.md       # Permission setup
│   ├── TROUBLESHOOTING.md   # Common issues
│   └── PERFORMANCE.md       # GPU acceleration
└── architecture/            # Existing
```

---

## Implementation Steps

### 1. Create Installation Guide

**New file: `docs/macos/INSTALL.md`**

```markdown
# Installing Hush on macOS

Hush is a fast, accurate voice-to-text application that works great on macOS, with native Metal GPU acceleration for Apple Silicon.

## System Requirements

### Minimum
- macOS 11.0 (Big Sur) or later
- 8 GB RAM
- 2 GB free disk space (for Whisper models)
- Microphone access

### Recommended
- macOS 13.0 (Ventura) or later
- Apple Silicon (M1, M2, M3+) for Metal GPU acceleration
- 16 GB RAM
- 5 GB free disk space

### Supported Architectures
- ✅ Apple Silicon (M1, M2, M3, M4) - Full Metal GPU support
- ✅ Intel Macs - CPU-only mode

## Installation Methods

### Method 1: Homebrew (Recommended)

```bash
# Coming soon - not yet available
brew install hush
```

### Method 2: Pre-built Binary

```bash
# Download latest release
curl -L https://github.com/your-username/hush/releases/latest/download/hush-macos-aarch64.tar.gz -o hush.tar.gz

# Extract
tar xzf hush.tar.gz

# Move to /usr/local/bin
sudo mv hush /usr/local/bin/

# Verify installation
hush --version
```

### Method 3: Build from Source

#### Prerequisites

Install Xcode Command Line Tools:
```bash
xcode-select --install
```

Install Rust (if not already installed):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

#### Build

```bash
# Clone repository
git clone https://github.com/your-username/hush.git
cd hush

# Build with Metal GPU support (Apple Silicon)
cargo build --release --features metal

# OR: Build CPU-only (Intel Mac or testing)
cargo build --release --no-default-features

# Install to /usr/local/bin
sudo cp target/release/hush /usr/local/bin/

# Verify
hush --version
```

#### Build Times
- Apple Silicon (M2): ~5-10 minutes
- Intel Mac: ~10-15 minutes

## Initial Setup

### 1. Download Whisper Model

```bash
# Download recommended model (base.en - 150MB)
hush download-model base.en

# Options:
# - tiny.en (75MB) - Fastest, lower accuracy
# - base.en (150MB) - Good balance (recommended)
# - small.en (500MB) - Better accuracy
# - medium.en (1.5GB) - Best accuracy, slower
```

### 2. Grant Permissions

Hush requires two permissions:

#### Microphone Access
On first run, macOS will prompt for microphone access. Click "OK".

Manual grant:
1. System Settings → Privacy & Security → Microphone
2. Enable checkbox for "hush" or your terminal app

#### Accessibility Access (Required for Text Insertion)
Hush needs Accessibility permissions to insert transcribed text.

**Setup:**
1. Run `hush check-permissions`
2. If not granted, macOS will show instructions
3. System Settings → Privacy & Security → Accessibility
4. Click the "+" button
5. Navigate to `/usr/local/bin/hush` (or your terminal app)
6. Enable the checkbox

**Important:** Restart the app after granting permissions.

### 3. Configure Hotkey

Default hotkey: `Cmd+Alt+V`

To customize:
```bash
# Edit config file
open ~/.config/hush/config.toml

# Change hotkey line:
hotkey = "Cmd+Shift+V"  # Or your preferred combo
```

### 4. Test Installation

```bash
# Check device and permissions
hush device-info
hush check-permissions

# Start listening
hush listen

# Press Cmd+Alt+V, speak, verify text appears
```

## GPU Acceleration

### Apple Silicon (M1/M2/M3/M4)

Metal GPU acceleration is **enabled by default** on Apple Silicon.

**Performance:**
- First token latency: ~120ms (6-8x faster than CPU)
- Continuous streaming: ~50ms per word
- Model loading: ~1-2 seconds

**Verify Metal is active:**
```bash
hush device-info
# Expected output: "✅ Metal GPU: Apple Silicon (Apple M2)"
```

### Intel Macs

Metal GPU is not effective on Intel Macs. CPU-only mode is recommended.

**Performance:**
- First token latency: ~800ms
- Continuous streaming: ~200ms per word
- Model loading: ~2-3 seconds

**Build for Intel:**
```bash
cargo build --release --no-default-features
```

## Uninstallation

```bash
# Remove binary
sudo rm /usr/local/bin/hush

# Remove config and models
rm -rf ~/.config/hush
rm -rf ~/.local/share/hush

# Remove logs
rm -rf ~/Library/Logs/hush
```

## Next Steps

- [Permission Setup Guide](PERMISSIONS.md)
- [Troubleshooting](TROUBLESHOOTING.md)
- [Performance Tuning](PERFORMANCE.md)
- [Configuration Guide](../CONFIGURATION.md)

## Getting Help

- GitHub Issues: https://github.com/your-username/hush/issues
- Documentation: https://github.com/your-username/hush/docs
- Discord: https://discord.gg/your-invite (if available)
```

### 2. Create Permission Guide

**New file: `docs/macos/PERMISSIONS.md`**

```markdown
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

## Optional Permissions

### 3. Screen Recording (Future)

Not currently used, but may be requested in future versions for advanced features.

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
3. Check `hush device-info` shows input device

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

### Permission Prompts

macOS will prompt for permissions at appropriate times:
- Microphone: First run
- Accessibility: Never auto-prompts (manual grant required)

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
- [Hush Privacy Policy](../PRIVACY.md)
```

### 3. Create Troubleshooting Guide

**New file: `docs/macos/TROUBLESHOOTING.md`**

```markdown
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
hush device-info
# Should show: "✅ Metal GPU: Apple Silicon"
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
hush list-devices

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
hush download-model base.en

# Verify download
ls -lh ~/.local/share/hush/models/
```

### Model download stuck or slow

**Use mirrors:**
```bash
# Set Hugging Face mirror
export HF_ENDPOINT=https://huggingface.co

# Download manually
hush download-model base.en --force
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
hush --create-config
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

2. **Run diagnostics:**
   ```bash
   hush diagnose  # (future feature)
   ```

3. **Search existing issues:**
   https://github.com/your-username/hush/issues

4. **Create new issue:**
   Include:
   - macOS version: `sw_vers`
   - Hush version: `hush --version`
   - CPU architecture: `uname -m`
   - Logs (redact sensitive info)
   - Steps to reproduce

5. **Community support:**
   - Discord: https://discord.gg/your-invite (if available)
   - Discussions: https://github.com/your-username/hush/discussions
```

### 4. Update Main README

**Update: `README.md`** (add macOS section)

```markdown
## Platform Support

### Linux
- ✅ X11 (primary)
- ✅ Wayland (via XWayland)
- ✅ NVIDIA GPUs (CUDA acceleration)
- ✅ AMD/Intel GPUs (ROCm support planned)

### macOS
- ✅ macOS 11.0 (Big Sur) and later
- ✅ Apple Silicon (M1, M2, M3, M4) with Metal GPU acceleration
- ✅ Intel Macs (CPU-only mode)
- ✅ Native system tray integration
- ✅ Full Accessibility API support

### Windows
- 🚧 Planned (not yet implemented)

## Installation

### Linux
See [INSTALL.md](docs/INSTALL.md)

### macOS
See [docs/macos/INSTALL.md](docs/macos/INSTALL.md)

Quick start:
```bash
# Build with Metal GPU support (Apple Silicon)
cargo build --release --features metal

# Run
./target/release/hush listen
```

## Performance

| Platform | GPU | Latency | Notes |
|----------|-----|---------|-------|
| Linux + NVIDIA | CUDA | ~80ms | Recommended for best performance |
| macOS + Apple Silicon | Metal | ~120ms | Native Metal acceleration |
| macOS + Intel | CPU | ~800ms | No GPU acceleration |
| Linux + CPU | CPU | ~800ms | Baseline performance |
```

---

## Success Criteria

- [ ] All documentation files created
- [ ] Installation guide is clear and comprehensive
- [ ] Permission guide covers all scenarios
- [ ] Troubleshooting guide addresses common issues
- [ ] README.md updated with macOS support
- [ ] Documentation reviewed for accuracy
- [ ] Links between docs work correctly
- [ ] Screenshots/examples added (if applicable)

---

## Verification Steps

```bash
# 1. Check all files exist
ls -la docs/macos/

# 2. Verify markdown syntax
# Use markdownlint or similar

# 3. Test instructions on real macOS machine
# Follow INSTALL.md step-by-step

# 4. Verify links work
# Click through all internal links

# 5. Spell check
# Use aspell or similar
```

---

## Files to Create

- `docs/macos/INSTALL.md` (~400 lines)
- `docs/macos/PERMISSIONS.md` (~300 lines)
- `docs/macos/TROUBLESHOOTING.md` (~400 lines)
- `docs/macos/PERFORMANCE.md` (~200 lines, optional)

## Files to Modify

- `README.md` - Add macOS section
- `docs/ARCHITECTURE.md` - Note platform-specific adapters
- `.github/ISSUE_TEMPLATE.md` - Add macOS template

---

## Dependencies

**Blocked by:**
- T-035 (text adapter must exist to document)
- T-036 (Metal support must exist)
- T-037 (tray adapter must exist)
- T-039 (tests validate instructions work)

---

## Optional Enhancements

- Add screenshots of permission dialogs
- Create video walkthrough
- Add FAQ section
- Create quick-start guide (1-page)
- Add comparison table (vs Dragon, vs Whisper.cpp, etc.)

Priority: Low (can be added post-MVP)

---

## Style Guide

- Use clear, concise language
- Include code examples for all commands
- Provide expected output for verification steps
- Use emoji sparingly for visual markers (✅, ❌, ⚠️, 🔐)
- Structure with clear headings and hierarchy
- Include "Why" explanations for requirements
- Link to official Apple docs where relevant

---

## Review Checklist

Before marking complete:
- [ ] All instructions tested on real macOS machine
- [ ] Both Apple Silicon and Intel Macs covered
- [ ] All permission flows documented
- [ ] Troubleshooting covers issues from testing
- [ ] Links to external resources are valid
- [ ] Code examples are accurate
- [ ] Formatting is consistent
- [ ] No Linux-specific instructions in macOS docs
