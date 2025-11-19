# UInput Setup and Reference Guide

**Last Updated:** 2025-11-19
**Status:** Active reference for AI agents

Comprehensive guide for Linux UInput text insertion in Hush.

## Overview

Hush uses a priority-based text insertion system:

1. **UInput (Primary)** - Kernel-level keyboard emulation (universal compatibility)
2. **X11/Enigo (Fallback)** - X11 window system simulation (good compatibility)
3. **Clipboard (Last Resort)** - Copy-paste method (limited compatibility)

UInput provides kernel-level keyboard emulation that works universally across all applications, including VMs, secure password fields, and applications that don't accept simulated input.

## Quick Setup (TL;DR)

```bash
# 1. Add yourself to input group
sudo usermod -a -G input $USER

# 2. Load uinput module
sudo modprobe uinput

# 3. Make uinput load at boot
echo 'uinput' | sudo tee /etc/modules-load.d/uinput.conf

# 4. Log out and log back in
# 5. Test with: groups | grep input
```

## Detailed Setup Methods

### Method 1: Temporary Setup (Session Only)

```bash
# Load the uinput kernel module
sudo modprobe uinput

# Give current user write access to uinput device
sudo chmod 666 /dev/uinput
```

**Note:** This setup is temporary and will be lost after a reboot.

### Method 2: Add User to Input Group (Recommended)

```bash
# Add your user to the input group
sudo usermod -a -G input $USER

# Ensure uinput module is loaded
sudo modprobe uinput

# Log out and log back in for group changes to take effect
```

After logging back in, verify the setup:
```bash
# Check if you're in the input group
groups | grep input

# Check if uinput device exists and is accessible
ls -la /dev/uinput
```

### Method 3: Persistent Setup with udev Rules

For a more permanent solution, create a udev rule:

```bash
# Create a udev rule for uinput permissions
sudo tee /etc/udev/rules.d/99-uinput.rules << 'EOF'
SUBSYSTEM=="misc", KERNEL=="uinput", GROUP="input", MODE="0664", TAG+="uaccess"
EOF

# Reload udev rules
sudo udevadm control --reload-rules && sudo udevadm trigger

# Ensure uinput module loads at boot
echo 'uinput' | sudo tee /etc/modules-load.d/uinput.conf

# Load module immediately
sudo modprobe uinput
```

## Verification

Test that uinput is working correctly:

```bash
# Check if uinput device exists
ls -la /dev/uinput

# Check if uinput module is loaded
lsmod | grep uinput

# Check if you're in input group
groups | grep input

# Test with Hush
RUST_LOG=debug cargo run
# Look for "Uinput keyboard: ready"
```

## Troubleshooting

### Quick Fixes Table

| Problem | Solution |
|---------|----------|
| `/dev/uinput` not found | `sudo modprobe uinput` |
| Permission denied | Add user to `input` group |
| Module not loading at boot | Add to `/etc/modules-load.d/uinput.conf` |
| Works sometimes | Expected - secure apps may block input |

### Problem: `/dev/uinput` not found

**Solution:**
```bash
# Check if uinput module is loaded
lsmod | grep uinput

# If not found, load the module
sudo modprobe uinput

# For permanent loading, add to modules
echo 'uinput' | sudo tee /etc/modules-load.d/uinput.conf
```

### Problem: Permission denied accessing `/dev/uinput`

**Check current permissions:**
```bash
ls -la /dev/uinput
# Should show: crw-rw---- 1 root input 10, 223 ...
```

**Solutions:**

1. **Temporary fix:**
   ```bash
   sudo chmod 666 /dev/uinput
   ```

2. **User group fix (recommended):**
   ```bash
   sudo usermod -a -G input $USER
   # Then log out and log back in
   ```

3. **udev rule fix (persistent):**
   ```bash
   sudo tee /etc/udev/rules.d/99-uinput.rules << 'EOF'
   SUBSYSTEM=="misc", KERNEL=="uinput", GROUP="input", MODE="0664", TAG+="uaccess"
   EOF
   sudo udevadm control --reload-rules && sudo udevadm trigger
   ```

### Problem: UInput events not working despite successful setup

**Possible causes and solutions:**

1. **Wayland vs X11:**
   - UInput works with both, but some Wayland compositors may have restrictions
   - Check your session type: `echo $XDG_SESSION_TYPE`
   - If using Wayland, ensure your compositor supports input event injection

2. **Security restrictions:**
   - Some applications (like secure password fields) may still block input
   - This is expected security behavior and not a uinput issue

3. **Check Hush logs:**
   ```bash
   # Run Hush with debug logging to see detailed uinput status
   RUST_LOG=debug cargo run
   ```

### Problem: Works for some applications but not others

**This is expected behavior.** Some applications may:
- Have additional security measures
- Run in isolated environments (VMs, containers)
- Use custom input handling

**Solutions:**
- Check Hush logs to see which insertion method was used
- These applications should automatically fallback to clipboard insertion
- Consider the application's security requirements (this may be intentional)

## System Requirements

### Supported Systems
- **Linux distributions:** Ubuntu, Debian, Fedora, Arch Linux, etc.
- **Kernel version:** 2.6+ (uinput has been stable for many years)
- **Desktop environments:** GNOME, KDE, XFCE, i3, Sway, etc.
- **Display servers:** X11 and Wayland

### Dependencies
- **uinput kernel module** (usually included in all modern Linux distributions)
- **Appropriate permissions** to access `/dev/uinput`

## Security Considerations

### Why UInput Requires Permissions

UInput allows creating virtual input devices that can send keyboard and mouse events to any application. This is a powerful capability that could be misused, so Linux restricts access to:

1. **Root user** (full access)
2. **Users in the `input` group** (recommended)
3. **Specific udev rules** (custom setups)

### Is This Safe?

Yes, when properly configured:

- **Legitimate use:** Hush only uses uinput to type transcribed speech
- **No data collection:** UInput only sends events, it doesn't read keystrokes
- **Process isolation:** Only the Hush process gets uinput access
- **Standard practice:** Many legitimate applications use uinput (accessibility tools, automation, gaming)

### Best Practices

1. **Use the input group method** rather than world-writable permissions
2. **Only grant access to trusted users** who need voice-to-text functionality
3. **Monitor system logs** if you're concerned about uinput usage
4. **Keep Hush updated** to ensure you have the latest security fixes

## Advanced Configuration

### Custom UInput Device Settings

Hush creates a virtual keyboard device with these characteristics:
- **Device name:** "Hush Voice Keyboard"
- **Bus type:** USB (virtual)
- **Supported keys:** Full US keyboard layout + common symbols
- **Event timing:** Configurable delays between keystrokes

### Integration with Voice-to-Text Pipeline

The uinput integration is automatic:
1. **Initialization:** Hush attempts to create uinput device at startup
2. **Fallback:** If uinput fails, falls back to X11 simulation automatically
3. **Method selection:** Chooses optimal insertion method per application
4. **Error handling:** Comprehensive error messages guide users to solutions

### Implementation Details

**For AI agents modifying text insertion code:**

Key files to check:
```bash
# UInput implementation
rg "UInputKeyboard" src/text/

# Text insertion strategy
rg "InsertionMethod" src/text/insertion.rs

# Error handling
rg "TextOutputError" src/core/error.rs
```

The multi-method fallback system:
1. Try UInput first
2. If UInput unavailable, try X11/enigo
3. If both fail, fall back to clipboard
4. Log which method was used for debugging

## FAQ

### Q: Do I need to restart after setup?

**A:** Only if you use the "add user to input group" method. In that case:
1. Log out completely
2. Log back in
3. Verify with: `groups | grep input`

### Q: Will this affect system security?

**A:** No, when properly configured. UInput access is limited to:
- Users you explicitly grant access to
- Applications those users choose to run
- The specific functionality they request (keyboard events only)

### Q: Can I revoke uinput access later?

**A:** Yes, easily:
```bash
# Remove user from input group
sudo gpasswd -d $USER input

# Remove udev rules if created
sudo rm /etc/udev/rules.d/99-uinput.rules
sudo udevadm control --reload-rules

# Unload module if desired
sudo modprobe -r uinput
```

### Q: What if my distribution doesn't have uinput?

**A:** This is extremely rare. UInput has been part of the Linux kernel for over a decade. If truly missing:
```bash
# Check kernel config (should show CONFIG_INPUT_UINPUT=y or =m)
cat /boot/config-$(uname -r) | grep UINPUT

# If missing, you may need to install a different kernel or use a different distribution
```

### Q: Can I use this with remote desktop or VMs?

**A:** Yes, but with caveats:
- **Host system:** UInput works normally
- **VM guest:** Depends on VM software and configuration
- **Remote desktop:** Depends on RDP/VNC client capabilities
- **Fallback:** Hush will automatically fall back to other methods if uinput doesn't work

## Implementation Notes for AI Agents

When working on UInput functionality:

1. **Always verify existing code first:**
   ```bash
   rg "UInputKeyboard" src/
   rg "uinput" src/text/
   ```

2. **Check error handling patterns:**
   ```bash
   rg "TextOutputError" src/core/error.rs
   ```

3. **Test fallback behavior:**
   - Simulate UInput unavailable
   - Verify X11 fallback works
   - Test clipboard fallback

4. **Never assume UInput is available:**
   - Always implement graceful fallback
   - Provide clear error messages
   - Guide users to setup solutions

## Reference

- **Location:** `src/text/uinput_keyboard.rs` - UInput implementation
- **Location:** `src/text/insertion.rs` - Multi-method text insertion
- **Location:** `src/core/error.rs` - Text output errors
