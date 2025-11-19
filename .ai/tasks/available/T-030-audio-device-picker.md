# Task: Audio Device Picker with Live Preview

## Description

Add an "Audio" tab to the settings window that lists available audio input devices, allows selection, and provides live audio level preview. This replaces the CLI-only audio device selection.

## Requirements

### Audio Tab UI
- [ ] Create "Audio" tab in settings window
- [ ] Display list of available audio input devices:
  - Device name (e.g., "USB Microphone", "Built-in Audio")
  - Device ID/index
  - Sample rate and format info
  - Current device indicator (radio button)
- [ ] Add device selection (radio buttons)
- [ ] Implement "Set as Default" button
- [ ] Show currently selected device prominently

### Live Audio Preview
- [ ] Add live audio level meter for selected device:
  - Horizontal bar showing current amplitude
  - Color-coded levels (green = good, yellow = loud, red = clipping)
  - Peak hold indicator
  - dB scale display (optional)
- [ ] Implement "Test Recording" button:
  - Records 3 seconds of audio
  - Shows waveform visualization
  - Plays back recording
  - Shows transcription of test audio
- [ ] Add volume/gain adjustment slider (if supported by device)

### Device Management
- [ ] Create `src/settings/audio_devices.rs` for device enumeration
- [ ] Detect and list all available audio input devices
- [ ] Handle device changes (device plugged/unplugged)
- [ ] Refresh device list button
- [ ] Show device status (available, in use, error)

### Integration
- [ ] Wire device selection to audio capture system
- [ ] Update config file when device changes
- [ ] Test audio capture with new device before applying
- [ ] Handle device errors gracefully (device not found, access denied)
- [ ] Apply device changes without requiring restart

### Advanced Settings (Optional)
- [ ] Sample rate selection (8kHz, 16kHz, 44.1kHz, 48kHz)
- [ ] Audio format selection (mono/stereo)
- [ ] Buffer size configuration
- [ ] Noise gate threshold
- [ ] Auto-gain control toggle

## Success Criteria

- `cargo check` passes
- `cargo test` passes
- `cargo clippy -- -D warnings` passes
- `cargo fmt` applied
- Audio tab displays all available input devices
- Live audio level meter works in real-time
- Test recording captures and plays back audio
- Device selection persists across restarts
- No audio glitches or dropouts
- Device changes handled gracefully
- Settings apply without restart

## Context

This is Phase 2, Step 3 of the UI expansion project. It builds on T-027 (Enhanced Settings Window) and makes audio configuration accessible to all users, not just CLI-comfortable developers.

**Current state:**
- CLI status: `./hush status --devices`
- Audio capture: `src/audio/mod.rs`
- Uses `cpal` crate for device enumeration
- Audio feedback: `src/audio/feedback.rs`

**Audio device requirements:**
- Must support input (not output)
- Prefer 16kHz mono for Whisper
- Handle various sample rates and formats

## Files to Create

- `src/settings/audio_devices.rs` - Device enumeration and management
- `src/settings/audio_tab.rs` - Audio tab UI implementation
- `src/settings/audio_preview.rs` - Live audio level meter

## Files to Check/Modify

- `src/settings/window.rs` - Add audio tab
- `src/audio/mod.rs` - Device selection integration
- `src/settings/config.rs` - Persist device selection
- `Cargo.toml` - Ensure cpal has needed features

## Reference Documentation

- cpal device enumeration: https://docs.rs/cpal/latest/cpal/
- Audio capture implementation: `src/audio/mod.rs`
- egui widgets for sliders/meters: https://docs.rs/egui
- Audio feedback system: `src/audio/feedback.rs`

## Estimated Complexity

**Medium-High** - Real-time audio level monitoring, device enumeration, test recording, UI integration. Estimated 2-3 days.

## Dependencies

- **Requires:** T-027 (Enhanced Settings Window) completed

## Testing Checklist

- [ ] List devices with multiple microphones connected
- [ ] Test with USB microphone
- [ ] Test with built-in microphone
- [ ] Test with no microphone connected
- [ ] Test device hotplug (plug in USB mic while running)
- [ ] Test device unplug (remove active device)
- [ ] Test recording with different devices
- [ ] Verify audio levels are accurate
- [ ] Test on different Linux audio systems (PulseAudio, PipeWire, ALSA)

## Future Enhancements

- Audio input source selection (if device has multiple)
- Echo cancellation toggle
- Noise suppression options
- Audio quality presets (low/medium/high)
- Device profiles (save settings per device)
- Visualize audio spectrum analyzer
