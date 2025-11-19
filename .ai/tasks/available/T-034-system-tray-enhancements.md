# Task: System Tray Enhancements - Quick Access Menu

## Description

Enhance the existing system tray integration (ksni) with a comprehensive menu that provides quick access to common actions, settings, and status information. This makes Hush more accessible and convenient for everyday use.

## Requirements

### System Tray Icon
- [ ] Improve system tray icon:
  - Default icon (idle state)
  - Recording icon (animated or different color)
  - Processing icon (spinner or different state)
  - Error icon (red or warning indicator)
- [ ] Add icon hover tooltip showing status
- [ ] Update icon in real-time based on application state

### Context Menu Structure
- [ ] Create comprehensive system tray menu:
  ```
  Hush
  ├── 🎤 Start Recording (or show "Recording..." if active)
  ├── ⏸️  Pause/Resume (if recording)
  ├── ──────────────
  ├── Model ▸
  │   ├── ● Base (current)
  │   ├── ○ Tiny
  │   ├── ○ Small
  │   └── ──────────────
  │       └── Manage Models...
  ├── Audio Device ▸
  │   ├── ● USB Microphone (current)
  │   ├── ○ Built-in Audio
  │   └── ──────────────
  │       └── Audio Settings...
  ├── ──────────────
  ├── ✨ LLM Polishing: Enabled
  ├── 🎯 Editing Mode: Medium
  ├── ──────────────
  ├── ⚙️  Settings...
  ├── 📊 Status
  ├── 📋 History
  ├── ──────────────
  ├── About Hush
  ├── Help & Documentation
  └── Quit
  ```

### Quick Actions
- [ ] Implement "Start Recording" action:
  - Triggers recording immediately (without hotkey)
  - Shows recording duration in menu
  - Changes to "Stop Recording" when active
- [ ] Add pause/resume functionality:
  - Pause recording mid-session
  - Resume recording
  - Show paused state clearly
- [ ] Add "Copy Last Transcription" action
- [ ] Add "Undo Last Insertion" action

### Quick Settings - Model Selection
- [ ] Add model submenu showing:
  - All downloaded models
  - Current model marked with ●
  - Other models marked with ○
  - "Manage Models..." opens settings
- [ ] Implement quick model switching:
  - Click model to switch
  - Show loading indicator
  - Update menu when switch complete
- [ ] Show model info on hover (size, speed)

### Quick Settings - Audio Device
- [ ] Add audio device submenu showing:
  - All available input devices
  - Current device marked with ●
  - Other devices marked with ○
  - "Audio Settings..." opens settings
- [ ] Implement quick device switching:
  - Click device to switch
  - Test device briefly before applying
  - Update menu when switch complete
- [ ] Show device status (available, in use, error)

### Status Display
- [ ] Add "Status" menu item that shows:
  - Current state (Idle, Recording, Processing)
  - Active model
  - Active audio device
  - LLM status (Enabled/Disabled)
  - Hotkey binding
  - Last transcription timestamp
- [ ] Make status info copyable (for bug reports)

### LLM Quick Toggle
- [ ] Add LLM polishing toggle:
  - Shows current state (Enabled/Disabled)
  - Click to toggle on/off
  - Update menu checkmark
  - Save to config
- [ ] Add editing mode submenu:
  - Light, Medium, Aggressive
  - Current mode marked with ●
  - Click to switch modes

### History Access
- [ ] Add "History" menu item:
  - Opens history viewer (future task)
  - Or shows recent transcriptions (last 5)
  - Click to copy transcription
  - Click to re-insert transcription

### Help and Documentation
- [ ] Add "Help & Documentation" submenu:
  - Quick Start Guide
  - Keyboard Shortcuts
  - Troubleshooting
  - Report Bug (opens GitHub)
  - Check for Updates
- [ ] Add "About Hush" dialog:
  - Version number
  - Build information
  - License
  - Credits

### Notifications Integration
- [ ] Send system notifications for:
  - Transcription complete
  - Model download complete
  - Errors (with details)
  - First-time setup reminders
- [ ] Use existing notify-rust integration
- [ ] Add notification preferences in settings

### State Synchronization
- [ ] Sync tray menu with application state:
  - Update when overlay changes
  - Update when settings change
  - Update when recording starts/stops
- [ ] Update menu items in real-time
- [ ] Disable unavailable actions (e.g., "Stop Recording" when idle)

## Success Criteria

- `cargo check` passes
- `cargo test` passes
- `cargo clippy -- -D warnings` passes
- `cargo fmt` applied
- System tray icon displays and updates correctly
- All menu items work as expected
- Quick settings (model, audio) switch correctly
- State synchronization works in real-time
- Notifications appear for important events
- No UI glitches or hanging menu
- Menu is responsive and fast

## Context

This is Phase 4 of the UI expansion project. It enhances the existing ksni system tray integration to provide quick access to common actions without opening the full settings window.

**Current state:**
- Basic system tray: Optional ksni integration (Cargo.toml line 66)
- No comprehensive menu
- Limited tray functionality

**Design philosophy:**
- Quick access to frequently used features
- Avoid opening settings window for simple changes
- Show status at a glance
- Complement (not replace) settings window

## Files to Create

- `src/settings/tray_menu.rs` - System tray menu implementation
- `src/settings/tray_actions.rs` - Menu action handlers
- `src/settings/tray_notifications.rs` - Notification management

## Files to Check/Modify

- `Cargo.toml` - Ensure ksni and notify-rust are available
- `src/application/hush_app.rs` - System tray integration
- `src/settings/window.rs` - Settings window integration
- `src/overlay/state.rs` - State synchronization

## Reference Documentation

- ksni docs: https://docs.rs/ksni/latest/ksni/
- notify-rust docs: https://docs.rs/notify-rust/latest/notify_rust/
- System tray best practices: freedesktop.org specifications
- `.ai/knowledge/rust-ui-frameworks-research.md` - UI research

## Estimated Complexity

**Medium** - Menu structure, state synchronization, notifications, icon management. Estimated 2 days.

## Dependencies

- **Requires:** T-027 (Enhanced Settings Window) completed
- **Requires:** T-028 (Model Management UI) completed
- **Requires:** T-030 (Audio Device Picker) completed
- **Benefits From:** T-032 (LLM Configuration UI) completed

## Testing Checklist

- [ ] Test system tray icon appears in taskbar
- [ ] Test icon updates when state changes
- [ ] Test all menu items
- [ ] Test model quick switch
- [ ] Test audio device quick switch
- [ ] Test LLM toggle
- [ ] Test editing mode switch
- [ ] Test "Start Recording" from tray
- [ ] Test "Stop Recording" from tray
- [ ] Test notifications appear correctly
- [ ] Test menu on different desktop environments (GNOME, KDE, XFCE)
- [ ] Test with ksni feature disabled (graceful degradation)

## Platform Considerations

- **Desktop Environments:**
  - GNOME: System tray in top panel
  - KDE Plasma: System tray in bottom panel
  - XFCE: System tray in panel
  - i3/sway: Status bar integration
- **D-Bus Requirement:** ksni requires D-Bus (libdbus-1-dev)
- **Fallback:** If ksni unavailable, show CLI reminder

## Future Enhancements

- Custom tray icon themes
- Configurable menu items (user can add/remove)
- Keyboard shortcuts for tray actions
- System tray mini-overlay (hover shows stats)
- Tray icon badge showing pending notifications
- Voice command to open tray menu
- Integration with GNOME Shell extensions
