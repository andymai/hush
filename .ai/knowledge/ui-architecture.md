# UI Architecture Patterns

**Purpose:** Architectural guidelines for Hush's UI expansion to enable future enhancements
**Focus:** Structure, patterns, and extensibility (not visual design)

---

## Core Architecture Principles

### 1. Separation of Concerns

**Module Structure:**
```
src/settings/
├── mod.rs              # Module exports
├── window.rs           # Main settings window (UI shell)
├── state.rs            # Settings state management
├── config.rs           # Configuration persistence
├── tabs/
│   ├── mod.rs
│   ├── general.rs      # General settings tab
│   ├── models.rs       # Model management tab
│   ├── audio.rs        # Audio device picker tab
│   ├── hotkeys.rs      # Hotkey configuration tab
│   └── advanced.rs     # Advanced settings tab
└── wizard.rs           # First-run setup wizard
```

**Pattern:** Each tab is an independent module with its own state and logic.

**Benefits:**
- Easy to add new tabs
- Each tab can be developed/tested independently
- Clear ownership and boundaries

### 2. State Management Pattern

**Three-Layer State Model:**

```rust
// 1. Runtime state (in-memory, fast)
pub struct SettingsState {
    pub current_model: ModelId,
    pub audio_device: DeviceId,
    pub hotkey: KeyBinding,
    // ... other runtime state
}

// 2. Pending changes (user edits, not yet applied)
pub struct PendingChanges {
    pub model: Option<ModelId>,
    pub audio: Option<DeviceId>,
    pub hotkey: Option<KeyBinding>,
    // ... tracks modifications
}

// 3. Persisted config (on disk)
pub struct SettingsConfig {
    // Serializable to TOML/JSON
    // Located at ~/.config/hush/settings.toml
}
```

**State Flow:**
```
User Edit → PendingChanges → Apply Button → SettingsState → SettingsConfig (save)
                             ↓
                          Cancel Button → Discard PendingChanges
```

**Key Pattern:** "Edit → Preview → Apply" workflow
- Changes are staged, not immediately applied
- User can preview effects before committing
- Cancel reverts to previous state

### 3. Configuration Persistence

**Location:** `~/.config/hush/settings.toml`

**Structure:**
```toml
[general]
theme = "dark"
overlay_position = "top-center"
overlay_opacity = 0.95

[model]
active = "base"
auto_download = false

[audio]
input_device = "default"
sample_rate = 16000

[hotkey]
record = "Ctrl+Alt+V"

[llm]
provider = "anthropic"
model = "claude-sonnet-4"
api_key_stored = true  # Actual key in system keyring
```

**Patterns:**
- Sectioned by feature area
- Flat structure (avoid deep nesting)
- Sensible defaults for missing keys
- Version field for migration support
- Secrets stored separately (system keyring, not TOML)

### 4. Tab Interface Pattern

**Each tab implements:**

```rust
pub trait SettingsTab {
    /// Render the tab content
    fn show(&mut self, ui: &mut egui::Ui, state: &mut SettingsState);

    /// Get pending changes (for Apply button)
    fn pending_changes(&self) -> Option<PendingChanges>;

    /// Apply pending changes to state
    fn apply(&mut self, state: &mut SettingsState) -> Result<(), SettingsError>;

    /// Discard pending changes
    fn cancel(&mut self);

    /// Tab title for UI
    fn title(&self) -> &str;

    /// Validation before allowing Apply
    fn validate(&self) -> Result<(), String>;
}
```

**Benefits:**
- Consistent interface across all tabs
- Easy to add new tabs (just implement trait)
- Centralized Apply/Cancel logic in window
- Built-in validation hook

### 5. Window Lifecycle Management

**Pattern: Managed Window State**

```rust
pub struct SettingsWindow {
    open: bool,                    // Is window open?
    size: Vec2,                    // Current size
    position: Option<Pos2>,        // Position (None = centered)
    active_tab: TabId,             // Which tab is selected
    tabs: Vec<Box<dyn SettingsTab>>, // All tabs
    state: SettingsState,          // Current state
}

impl SettingsWindow {
    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }

        egui::Window::new("Hush Settings")
            .open(&mut self.open)
            .default_size(self.size)
            .resizable(true)
            .show(ctx, |ui| {
                self.render_tabs(ui);
                self.render_buttons(ui);
            });

        // Save position on close
        if !self.open {
            self.save_position();
        }
    }
}
```

**Key Features:**
- Window state persists between sessions
- Position/size saved automatically
- Proper cleanup on close

### 6. Integration Points

**How settings window integrates with main app:**

```rust
// In main application loop
pub struct HushApp {
    overlay: OverlayWindow,
    settings: Option<SettingsWindow>,  // Created on demand
    tray: Option<SystemTray>,
    hotkey_manager: HotkeyManager,
}

impl HushApp {
    fn handle_events(&mut self, ctx: &egui::Context) {
        // Open settings via hotkey
        if self.hotkey_manager.settings_key_pressed() {
            self.open_settings();
        }

        // Open settings via tray menu
        if let Some(tray) = &self.tray {
            if tray.settings_clicked() {
                self.open_settings();
            }
        }

        // Render settings if open
        if let Some(settings) = &mut self.settings {
            settings.show(ctx);
        }
    }

    fn open_settings(&mut self) {
        if self.settings.is_none() {
            self.settings = Some(SettingsWindow::new());
        }
    }
}
```

**Pattern:** Lazy initialization
- Settings window created only when needed
- Saves memory when not in use
- Fast to create (< 16ms)

### 7. Async Operations Pattern

**Problem:** Downloads, API calls, device enumeration can block UI

**Solution: Channels + Background Tasks**

```rust
use tokio::sync::mpsc;

pub struct ModelDownloader {
    progress_rx: mpsc::Receiver<DownloadProgress>,
    cancel_tx: mpsc::Sender<()>,
}

impl ModelDownloader {
    pub fn start_download(&mut self, model: ModelId) {
        let (progress_tx, progress_rx) = mpsc::channel(100);
        let (cancel_tx, cancel_rx) = mpsc::channel(1);

        tokio::spawn(async move {
            // Download in background
            download_model(model, progress_tx, cancel_rx).await;
        });

        self.progress_rx = progress_rx;
        self.cancel_tx = cancel_tx;
    }

    pub fn render_progress(&mut self, ui: &mut egui::Ui) {
        // Non-blocking check for progress updates
        while let Ok(progress) = self.progress_rx.try_recv() {
            self.current_progress = progress;
        }

        ui.add(egui::ProgressBar::new(self.current_progress));

        // Cancel button
        if ui.button("Cancel").clicked() {
            let _ = self.cancel_tx.try_send(());
        }
    }
}
```

**Key Patterns:**
- UI thread never blocks
- Progress updates via channels
- Cancellation via channel
- Request repaint when progress updates

### 8. Error Handling

**Pattern: User-Friendly Error Display**

```rust
pub enum SettingsError {
    ModelNotFound(String),
    AudioDeviceUnavailable(String),
    HotkeyConflict(String),
    ConfigSaveFailed(std::io::Error),
    // ... more variants
}

impl SettingsError {
    pub fn user_message(&self) -> String {
        match self {
            Self::ModelNotFound(model) => {
                format!("Model '{}' not found. Download it first.", model)
            }
            Self::AudioDeviceUnavailable(device) => {
                format!("Audio device '{}' is not available. Check connections.", device)
            }
            Self::HotkeyConflict(existing) => {
                format!("Hotkey conflicts with '{}'. Choose a different key.", existing)
            }
            Self::ConfigSaveFailed(err) => {
                format!("Failed to save settings: {}. Check file permissions.", err)
            }
        }
    }

    pub fn show_dialog(&self, ui: &mut egui::Ui) {
        egui::Window::new("Error")
            .collapsible(false)
            .show(ui.ctx(), |ui| {
                ui.colored_label(egui::Color32::RED, "⚠");
                ui.label(self.user_message());

                if ui.button("OK").clicked() {
                    // Close dialog
                }

                if ui.button("View Details").clicked() {
                    // Show technical details
                }
            });
    }
}
```

**Key Features:**
- User-friendly messages (not debug strings)
- Actions to resolve errors
- Technical details available but hidden by default

### 9. Testing Strategy

**Unit Tests:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_pending_changes() {
        let mut state = SettingsState::default();
        let mut tab = GeneralTab::new();

        tab.set_theme(Theme::Dark);
        tab.apply(&mut state).unwrap();

        assert_eq!(state.theme, Theme::Dark);
    }

    #[test]
    fn test_cancel_reverts_changes() {
        let mut tab = GeneralTab::new();
        tab.set_theme(Theme::Dark);
        tab.cancel();

        assert!(tab.pending_changes().is_none());
    }
}
```

**Integration Tests:**
```rust
#[test]
fn test_settings_persistence() {
    let mut window = SettingsWindow::new();
    window.state.theme = Theme::Dark;
    window.save().unwrap();

    let loaded = SettingsWindow::load().unwrap();
    assert_eq!(loaded.state.theme, Theme::Dark);
}
```

**Mock UI Testing:**
```rust
#[test]
fn test_tab_rendering() {
    let mut tab = GeneralTab::new();
    let ctx = egui::Context::default();

    // Simulate UI rendering (without window)
    ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            tab.show(ui, &mut SettingsState::default());
        });
    });

    // Assert no panics, proper render
}
```

### 10. Extensibility Patterns

**Adding a New Tab:**

1. Create `src/settings/tabs/my_tab.rs`
2. Implement `SettingsTab` trait
3. Register in `src/settings/window.rs`:

```rust
impl SettingsWindow {
    pub fn new() -> Self {
        Self {
            tabs: vec![
                Box::new(GeneralTab::new()),
                Box::new(ModelsTab::new()),
                Box::new(AudioTab::new()),
                Box::new(HotkeysTab::new()),
                Box::new(MyTab::new()),  // <-- Add here
            ],
            // ...
        }
    }
}
```

**Adding a New Setting:**

1. Add field to `SettingsState`:
```rust
pub struct SettingsState {
    pub my_new_setting: MyType,
    // ...
}
```

2. Add to config serialization:
```rust
pub struct SettingsConfig {
    pub my_new_setting: MyType,
    // ...
}
```

3. Add UI controls in appropriate tab:
```rust
impl SettingsTab for GeneralTab {
    fn show(&mut self, ui: &mut egui::Ui, state: &mut SettingsState) {
        ui.checkbox(&mut state.my_new_setting, "My New Setting");
    }
}
```

**Pattern:** Settings are just fields in a struct. No special registration needed.

### 11. Performance Considerations

**egui Best Practices:**

```rust
// ✗ BAD: Recompute every frame
fn show(&mut self, ui: &mut egui::Ui) {
    let devices = enumerate_audio_devices();  // Expensive!
    for device in devices {
        ui.label(device.name());
    }
}

// ✓ GOOD: Cache expensive operations
fn show(&mut self, ui: &mut egui::Ui) {
    if self.devices_cache.is_none() {
        self.devices_cache = Some(enumerate_audio_devices());
    }

    for device in self.devices_cache.as_ref().unwrap() {
        ui.label(device.name());
    }

    if ui.button("Refresh").clicked() {
        self.devices_cache = None;  // Invalidate cache
    }
}
```

**Repaint Strategy:**

```rust
// Static UI (settings window) - low refresh rate
ctx.request_repaint_after(Duration::from_secs(1));

// Animated UI (progress bar) - high refresh rate
ctx.request_repaint_after(Duration::from_millis(16));  // 60 FPS

// Idle UI (no changes) - no repaints needed
// (egui automatically repaints on user input)
```

### 12. Future-Proofing

**Design Decisions for Extensibility:**

1. **Plugin-Style Tabs**
   - Each tab is self-contained
   - Easy to add/remove without affecting others
   - Could support dynamic loading in future

2. **Versioned Config**
   - Config file includes version field
   - Migration path for breaking changes
   - Backward compatibility possible

3. **Trait-Based Integration**
   - Core app depends on traits, not concrete types
   - Settings can be mocked for testing
   - Alternative implementations possible

4. **Event-Driven Updates**
   - Settings changes emit events
   - Other components can subscribe
   - Decoupled from UI code

---

## Common Patterns Summary

### State Management
- **Edit → Apply** pattern (staged changes)
- Separate runtime state from config state
- Three-layer model (runtime, pending, persisted)

### UI Structure
- Tab-based navigation
- Each tab is independent module
- Consistent trait interface

### Async Operations
- Channels for progress updates
- Background tasks for blocking work
- Non-blocking UI thread

### Error Handling
- User-friendly messages
- Recovery options
- Technical details available

### Testing
- Unit tests for logic
- Integration tests for persistence
- Mock UI rendering for UI code

### Extensibility
- Add tabs by implementing trait
- Add settings by adding fields
- No central registration required

---

## Anti-Patterns to Avoid

### ❌ Tightly Coupled Tabs
```rust
// BAD: ModelsTab directly accesses AudioTab
impl ModelsTab {
    fn render(&mut self, audio_tab: &AudioTab) {
        // Don't do this!
    }
}
```

**Solution:** Use shared state, not direct tab coupling.

### ❌ Blocking Operations in UI Thread
```rust
// BAD: Downloads block UI
if ui.button("Download").clicked() {
    download_model_sync();  // UI freezes!
}
```

**Solution:** Use async + channels pattern (see section 7).

### ❌ Settings Spread Across Multiple Files
```rust
// BAD: Config in multiple places
~/.config/hush/settings.toml
~/.config/hush/models.toml
~/.config/hush/audio.toml
```

**Solution:** Single config file with sections.

### ❌ No Validation Before Apply
```rust
// BAD: Apply without validation
fn apply(&mut self) {
    self.state.hotkey = self.pending_hotkey;  // May be invalid!
}
```

**Solution:** Validate in `validate()` method, check before apply.

---

## Reference Implementations

See these Hush files for examples:
- **Window lifecycle**: `src/overlay/window.rs`
- **State management**: `src/overlay/state.rs`
- **Config persistence**: `src/cli/config.rs`
- **Trait-based design**: `src/core/traits.rs`
- **Async operations**: `src/transcription/whisper.rs`

---

**Summary:** These patterns create a maintainable, extensible UI architecture that can grow with Hush's needs while keeping complexity manageable.
