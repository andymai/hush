# UInput Quick Reference for Hush

## TL;DR Setup (Most Users)

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

## Quick Fix for Permission Issues

```bash
# Temporary (until reboot)
sudo chmod 666 /dev/uinput

# Permanent
sudo usermod -a -G input $USER
# Then log out and back in
```

## Verification Commands

```bash
# Check if uinput is loaded
lsmod | grep uinput

# Check device permissions
ls -la /dev/uinput

# Check if you're in input group
groups | grep input

# Test with Hush
RUST_LOG=debug cargo run
# Look for "Uinput keyboard: ready"
```

## Priority System

1. **UInput** (best) - Works everywhere, including VMs and secure fields
2. **X11/Enigo** (good) - Works with most desktop applications  
3. **Clipboard** (fallback) - Works but limited to paste-supporting apps

## Common Issues

| Problem | Solution |
|---------|----------|
| `/dev/uinput` not found | `sudo modprobe uinput` |
| Permission denied | Add user to `input` group |
| Module not loading at boot | Add to `/etc/modules-load.d/uinput.conf` |
| Works sometimes | Expected - secure apps may block input |

## Need More Help?

See the full [UInput Setup Guide](uinput-setup.md) for detailed troubleshooting and advanced configuration.