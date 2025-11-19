# Task: Hotkey Configurator with Conflict Detection

## Description

Add a "Hotkeys" tab to the settings window that allows users to customize the push-to-talk hotkey and other keyboard shortcuts. Include conflict detection, validation, and a visual hotkey recorder.

## Requirements

### Hotkeys Tab UI
- [ ] Create "Hotkeys" tab in settings window
- [ ] Display list of configurable hotkeys:
  - Push-to-talk (currently: Ctrl+Alt+V)
  - Open settings (proposed: Ctrl+Alt+S)
  - Toggle overlay visibility (proposed: Ctrl+Alt+H)
  - Quick undo (optional: Ctrl+Alt+Z)
- [ ] Show current hotkey binding for each action
- [ ] Add "Change" button next to each hotkey
- [ ] Show hotkey status (active, disabled, conflict)
- [ ] Add "Reset to Defaults" button

### Hotkey Recorder
- [ ] Implement modal dialog for hotkey capture:
  - Show "Press keys..." prompt
  - Capture key combination in real-time
  - Display pressed keys as user types (e.g., "Ctrl+Alt+")
  - Validate hotkey (minimum 2 modifiers + 1 key)
  - Cancel button to abort
  - Apply button to confirm
- [ ] Support modifier keys: Ctrl, Alt, Shift, Super/Meta
- [ ] Support letter keys (A-Z)
- [ ] Support function keys (F1-F12)
- [ ] Support number keys (0-9)
- [ ] Block invalid combinations (e.g., single key, only modifiers)

### Conflict Detection
- [ ] Check for conflicts with:
  - Other Hush hotkeys (show warning)
  - Common system hotkeys (show warning, allow override)
  - Desktop environment hotkeys (detect if possible)
- [ ] Show conflict warning dialog:
  - Which action currently uses this hotkey
  - Option to swap hotkeys
  - Option to override (with warning)
  - Option to cancel
- [ ] Maintain hotkey uniqueness within Hush

### Hotkey Management
- [ ] Create `src/settings/hotkey_config.rs` for hotkey management
- [ ] Implement hotkey validation logic
- [ ] Store hotkeys in config file (human-readable format)
- [ ] Update `global-hotkey` integration with new bindings
- [ ] Unregister old hotkey before registering new one
- [ ] Handle hotkey registration failures gracefully
- [ ] Test hotkey immediately after applying

### System Integration
- [ ] Wire to existing hotkey system in `src/adapters/hotkey/`
- [ ] Support X11 and Wayland hotkey registration
- [ ] Handle platform-specific modifiers (Super/Meta)
- [ ] Show platform-specific warnings if needed
- [ ] Restart hotkey listener when config changes

### User Experience
- [ ] Show hotkey in human-readable format (e.g., "Ctrl+Alt+V")
- [ ] Add tooltips explaining hotkey requirements
- [ ] Disable conflicting system hotkeys warning if user proceeds
- [ ] Provide common hotkey suggestions
- [ ] Add "Test" button to verify hotkey works

## Success Criteria

- `cargo check` passes
- `cargo test` passes
- `cargo clippy -- -D warnings` passes
- `cargo fmt` applied
- Hotkeys tab displays correctly in settings window
- Hotkey recorder captures key combinations accurately
- Conflict detection works and prevents duplicates
- Custom hotkeys persist across restarts
- Hotkeys work immediately after applying (no restart needed)
- System hotkey conflicts are detected and warned
- Invalid hotkeys are rejected with clear messages

## Context

This is Phase 2, Step 4 of the UI expansion project. It builds on T-027 (Enhanced Settings Window) and addresses a major limitation: hardcoded hotkeys.

**Current state:**
- Hardcoded hotkey: Ctrl+Alt+V (in code)
- Hotkey management: `src/adapters/hotkey/`
- Uses `global-hotkey` crate (version 0.6)
- X11 and Wayland support via `winit` (version 0.29)

**Hotkey requirements:**
- Must be global (work when Hush not focused)
- Should require modifiers to avoid conflicts
- Must be unregistered properly on exit

## Files to Create

- `src/settings/hotkey_config.rs` - Hotkey configuration and validation
- `src/settings/hotkey_tab.rs` - Hotkeys tab UI implementation
- `src/settings/hotkey_recorder.rs` - Modal hotkey capture dialog

## Files to Check/Modify

- `src/settings/window.rs` - Add hotkeys tab
- `src/adapters/hotkey/` - Hotkey registration integration
- `src/settings/config.rs` - Persist hotkey configuration
- `Cargo.toml` - Ensure global-hotkey has needed features

## Reference Documentation

- global-hotkey docs: https://docs.rs/global-hotkey/0.6
- winit key codes: https://docs.rs/winit/0.29/winit/keyboard/
- Hotkey adapter: `src/adapters/hotkey/global_hotkey_adapter.rs`
- egui modal dialogs: https://docs.rs/egui (Window widget)

## Estimated Complexity

**Medium** - Hotkey capture, conflict detection, system integration. Estimated 2 days.

## Dependencies

- **Requires:** T-027 (Enhanced Settings Window) completed

## Testing Checklist

- [ ] Test hotkey recorder with various combinations
- [ ] Test with Ctrl+Alt+V (current default)
- [ ] Test with function keys (F1-F12)
- [ ] Test with number keys
- [ ] Test with Super/Meta key (Linux)
- [ ] Test conflict detection between Hush hotkeys
- [ ] Test hotkey registration failure handling
- [ ] Test hotkey unregistration on change
- [ ] Test hotkey persistence across restarts
- [ ] Test reset to defaults
- [ ] Test on X11 and Wayland (if possible)

## Common System Hotkeys to Warn About

- Ctrl+Alt+Del (system)
- Ctrl+Alt+F1-F12 (virtual terminals)
- Super+L (lock screen)
- Super+D (show desktop)
- Alt+Tab (window switching)
- Ctrl+Alt+T (terminal)

## Future Enhancements

- Multi-key sequences (e.g., Ctrl+K, then Ctrl+V)
- Mouse button hotkeys
- Hotkey profiles (different sets for different workflows)
- Import/export hotkey configuration
- System hotkey database for better conflict detection
- Record macro sequences
