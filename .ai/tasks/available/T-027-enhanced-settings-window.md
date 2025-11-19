# Task: Enhanced Settings Window - Core Infrastructure

## Description

Create a comprehensive settings window separate from the overlay panel. This will be a proper egui window (not overlay) that provides access to all Hush configuration options. This task focuses on the core window infrastructure and basic UI layout.

## Requirements

### Core Window Infrastructure
- [ ] Create `src/settings/mod.rs` module
- [ ] Create `src/settings/window.rs` with `SettingsWindow` struct
- [ ] Implement egui window (non-overlay) with proper lifecycle
- [ ] Add window state management (open/closed, position, size)
- [ ] Implement window persistence (save position/size between sessions)
- [ ] Add proper window decorations and close button

### Basic UI Layout
- [ ] Create tabbed interface with sections: General, Models, Audio, Hotkeys, Advanced
- [ ] Implement "General" tab with basic settings:
  - Display theme (Dark/Light)
  - Overlay position selector
  - Overlay opacity slider
  - Show button when idle toggle
- [ ] Add "Apply" and "Cancel" buttons
- [ ] Implement settings state management (pending vs. applied changes)

### Integration
- [ ] Add settings window trigger from system tray (if enabled)
- [ ] Add hotkey to open settings (e.g., Ctrl+Alt+S)
- [ ] Update overlay to include "Settings" button/menu
- [ ] Wire settings changes to existing `OverlayConfig`

### Configuration Persistence
- [ ] Create `src/settings/config.rs` for settings serialization
- [ ] Implement save/load from `~/.config/hush/settings.toml`
- [ ] Migrate existing overlay config to new settings system
- [ ] Add config validation and error handling

## Success Criteria

- `cargo check` passes
- `cargo test` passes
- `cargo clippy -- -D warnings` passes
- `cargo fmt` applied
- Settings window opens and closes properly
- Window position/size persists between sessions
- General tab settings work correctly
- Settings save to and load from config file
- No regression in existing overlay functionality

## Context

This is Phase 1 of the UI expansion project. The settings window will serve as the foundation for all subsequent UI enhancements (model management, audio picker, hotkey config, etc.).

**Current state:**
- Basic settings in overlay panel: `src/overlay/ui.rs` (lines ~242-328)
- Overlay config: `src/overlay/state.rs`
- System tray: Optional ksni support in `Cargo.toml`

## Files to Create

- `src/settings/mod.rs` - Module exports
- `src/settings/window.rs` - Main settings window implementation
- `src/settings/config.rs` - Settings serialization/persistence
- `src/settings/state.rs` - Settings state management

## Files to Check/Modify

- `src/overlay/ui.rs` - Reference for existing settings UI
- `src/overlay/state.rs` - OverlayConfig integration
- `src/main.rs` or `src/application/hush_app.rs` - Integration point
- `Cargo.toml` - May need additional egui features

## Reference Documentation

- egui docs: https://docs.rs/egui
- egui examples: https://github.com/emilk/egui/tree/master/examples
- `.ai/knowledge/rust-ui-frameworks-research.md` - UI framework research
- `.ai/knowledge/conventions.md` - Coding standards

## Estimated Complexity

**Medium** - Requires new module structure, window lifecycle management, config persistence, but follows established egui patterns. Estimated 1-2 days.

## Dependencies

None - This is the foundation task for UI expansion.

## Future Tasks This Enables

- T-028: Model management UI
- T-029: Model download UI
- T-030: Audio device picker
- T-031: Hotkey configurator
- T-032: LLM configuration UI
