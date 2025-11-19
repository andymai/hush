# UI Architecture Reference for Settings Window

**Purpose:** Document actual Hush UI patterns to guide settings window implementation
**Based on:** Real code in `src/overlay/` (not speculative)

---

## Core Patterns Discovered in Hush Overlay

### 1. State Machine with Enum

**Pattern Found:** `src/overlay/state.rs:5-25`

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum OverlayState {
    Idle,
    Recording { start_time: Instant, amplitude: f32 },
    Processing { message: String },
    Success { text: String, show_until: Instant },
    Error { message: String, show_until: Instant },
    Settings,
}
```

**Why this pattern:**
- Each variant represents a distinct UI mode
- Associated data stored inline (no separate struct needed)
- Transitions are explicit via constructor methods
- Clone-able for sharing across threads

**Apply to settings window:**
```rust
// Proposed: src/settings/state.rs
#[derive(Debug, Clone, PartialEq)]
pub enum SettingsWindowState {
    Closed,
    Open { active_tab: TabId },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TabId {
    General,
    Models,
    Audio,
    Hotkeys,
    Advanced,
}
```

### 2. Functional Rendering (Not OOP)

**Pattern Found:** `src/overlay/ui.rs:23-87`

```rust
pub fn render_overlay(
    ctx: &Context,
    state: &OverlayState,
    config: &OverlayConfig,
) -> OverlayAction {
    // ...
    match state {
        OverlayState::Idle => render_idle_state(ui, config),
        OverlayState::Recording { .. } => render_recording_state(ui, ...),
        OverlayState::Settings => render_settings_state(ui, config),
        // ...
    }
}
```

**Why this pattern:**
- Pure functions (easy to test)
- Pattern matching ensures all states handled
- No trait objects or dynamic dispatch
- Clear control flow

**Apply to settings window:**
```rust
// Proposed: src/settings/ui.rs
pub fn render_settings_window(
    ctx: &egui::Context,
    state: &mut SettingsWindowState,
    config: &Config,
) -> SettingsAction {
    if let SettingsWindowState::Open { active_tab } = state {
        egui::Window::new("Hush Settings")
            .show(ctx, |ui| {
                render_tabs(ui, active_tab);

                match active_tab {
                    TabId::General => render_general_tab(ui, config),
                    TabId::Models => render_models_tab(ui, config),
                    TabId::Audio => render_audio_tab(ui, config),
                    // ...
                }
            });
    }

    SettingsAction::None
}
```

### 3. Action Enum for UI Events

**Pattern Found:** `src/overlay/ui.rs:90-97`

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum OverlayAction {
    None,
    StartRecording,
    Settings,
    CloseSettings,
    ToggleTheme,
}
```

**Why this pattern:**
- Decouples UI from application logic
- Render function returns intent, not side effects
- Easy to test (just check return value)
- Clear API contract

**Apply to settings window:**
```rust
// Proposed: src/settings/ui.rs
#[derive(Debug, Clone, PartialEq)]
pub enum SettingsAction {
    None,
    ApplyChanges { new_config: Config },
    CancelChanges,
    SwitchTab { tab: TabId },
    OpenModelDownloader,
    TestAudioDevice { device_id: String },
}
```

### 4. Config Struct Pattern

**Pattern Found:** `src/overlay/state.rs:133-149`

```rust
#[derive(Debug, Clone)]
pub struct OverlayConfig {
    pub width: f32,
    pub height: f32,
    pub position: OverlayPosition,
    pub opacity: f32,
    pub auto_hide_duration: Duration,
    pub show_button_when_idle: bool,
    pub theme: OverlayTheme,
}
```

**Also:** `src/config/settings.rs` has serializable configs with serde

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub audio: AudioConfig,
    pub transcription: TranscriptionConfig,
    pub hotkey: HotkeyConfig,
    // ...
}
```

**Why this pattern:**
- Flat structure (no deep nesting)
- Public fields (simple access)
- Derives Clone for sharing
- Serde for persistence

**Apply to settings window:**
```rust
// Extend existing: src/config/settings.rs
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub audio: AudioConfig,
    pub transcription: TranscriptionConfig,
    pub hotkey: HotkeyConfig,
    pub overlay: OverlayConfig,  // NEW: Add this
    pub ui: UiConfig,             // NEW: Add this
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UiConfig {
    pub theme: ThemeMode,  // Dark, Light, System
    pub settings_window_size: (f32, f32),
    pub settings_window_position: Option<(f32, f32)>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum ThemeMode {
    Dark,
    Light,
    System,  // NEW: Detect system theme
}
```

### 5. State Methods Pattern

**Pattern Found:** `src/overlay/state.rs:28-124`

```rust
impl OverlayState {
    pub fn start_recording() -> Self { /* ... */ }
    pub fn processing(message: impl Into<String>) -> Self { /* ... */ }
    pub fn success(text: impl Into<String>, auto_hide_after: Duration) -> Self { /* ... */ }
    pub fn should_hide(&self) -> bool { /* ... */ }
    pub fn is_idle(&self) -> bool { /* ... */ }
}
```

**Why this pattern:**
- Constructor methods for state transitions
- Query methods for state checks
- Encapsulates state logic
- Self-documenting API

**Apply to settings window:**
```rust
impl SettingsWindowState {
    pub fn open_with_tab(tab: TabId) -> Self {
        Self::Open { active_tab: tab }
    }

    pub fn close() -> Self {
        Self::Closed
    }

    pub fn is_open(&self) -> bool {
        matches!(self, Self::Open { .. })
    }

    pub fn active_tab(&self) -> Option<TabId> {
        match self {
            Self::Open { active_tab } => Some(*active_tab),
            Self::Closed => None,
        }
    }
}
```

### 6. Theme Application Pattern

**Pattern Found:** `src/overlay/ui.rs:99-114`

```rust
fn apply_theme(ctx: &Context, theme: OverlayTheme) {
    let mut style = (*ctx.style()).clone();

    match theme {
        OverlayTheme::Dark => {
            style.visuals.window_fill = Color32::from_rgba_unmultiplied(20, 20, 20, 240);
            // ...
        },
        OverlayTheme::Light => {
            style.visuals.window_fill = Color32::from_rgba_unmultiplied(240, 240, 240, 240);
            // ...
        },
    }

    ctx.set_style(style);
}
```

**Apply to settings window with system theme:**
```rust
// NEW: System theme detection
fn detect_system_theme() -> OverlayTheme {
    // Use dark-light crate or platform detection
    if dark_light::detect() == dark_light::Mode::Dark {
        OverlayTheme::Dark
    } else {
        OverlayTheme::Light
    }
}

fn apply_theme_mode(ctx: &Context, mode: ThemeMode) {
    let resolved_theme = match mode {
        ThemeMode::Dark => OverlayTheme::Dark,
        ThemeMode::Light => OverlayTheme::Light,
        ThemeMode::System => detect_system_theme(),
    };

    apply_theme(ctx, resolved_theme);
}
```

---

## Proposed Settings Window Architecture

Based on discovered patterns, here's the recommended structure:

### Module Structure
```
src/settings/
├── mod.rs              # Exports
├── state.rs            # SettingsWindowState enum
├── ui.rs               # render_settings_window() + tab renderers
├── actions.rs          # SettingsAction enum
└── persistence.rs      # Save/load window position/size
```

### State Management Flow

```
User opens settings (hotkey/tray)
  ↓
state = SettingsWindowState::open_with_tab(TabId::General)
  ↓
render_settings_window(ctx, &mut state, config)
  ↓
Pattern match on state.active_tab
  ↓
Render appropriate tab (pure function)
  ↓
Return SettingsAction (Apply/Cancel/SwitchTab)
  ↓
Handle action in main app loop
  ↓
Update config / transition state
```

### Config Persistence

Follow existing pattern from `src/config/settings.rs`:

```rust
// Config already has serde - just extend it
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub audio: AudioConfig,
    pub transcription: TranscriptionConfig,
    pub hotkey: HotkeyConfig,
    pub ui: UiConfig,  // NEW
}

// Load/save follows existing patterns
pub fn load_config() -> Result<Config> {
    let path = config_path()?;
    if path.exists() {
        let contents = fs::read_to_string(path)?;
        Ok(toml::from_str(&contents)?)
    } else {
        Ok(Config::default())
    }
}

pub fn save_config(config: &Config) -> Result<()> {
    let path = config_path()?;
    let toml = toml::to_string_pretty(config)?;
    fs::write(path, toml)?;
    Ok(())
}
```

### Integration with Main App

Follow overlay pattern from `src/overlay/window.rs`:

```rust
pub struct HushApp {
    overlay: OverlayWindow,
    settings_state: SettingsWindowState,  // NEW
    config: Arc<Mutex<Config>>,
}

impl HushApp {
    fn render(&mut self, ctx: &egui::Context) {
        // Render overlay (existing)
        self.overlay.render(ctx);

        // Render settings window (new)
        let action = render_settings_window(
            ctx,
            &mut self.settings_state,
            &self.config.lock()
        );

        // Handle actions
        match action {
            SettingsAction::ApplyChanges { new_config } => {
                *self.config.lock() = new_config;
                save_config(&new_config).ok();
                self.settings_state = SettingsWindowState::Closed;
            }
            SettingsAction::CancelChanges => {
                self.settings_state = SettingsWindowState::Closed;
            }
            // ...
        }
    }
}
```

---

## Anti-Patterns to Avoid

Based on Hush codebase analysis:

### ❌ Don't: Use trait objects for tabs
```rust
// BAD - Not the Hush way
trait SettingsTab {
    fn render(&mut self, ui: &mut Ui);
}
let tabs: Vec<Box<dyn SettingsTab>> = vec![...];
```

### ✅ Do: Use enum and match
```rust
// GOOD - Follows Hush overlay pattern
enum TabId { General, Models, Audio }
match active_tab {
    TabId::General => render_general_tab(ui, config),
    TabId::Models => render_models_tab(ui, config),
    // ...
}
```

### ❌ Don't: Mutate config in render functions
```rust
// BAD - Side effects in render
fn render_general_tab(ui: &mut Ui, config: &mut Config) {
    if ui.checkbox(&mut config.show_button, "Show button").changed() {
        save_config(config);  // Side effect!
    }
}
```

### ✅ Do: Return actions, handle in app loop
```rust
// GOOD - Pure render, return intent
fn render_general_tab(ui: &mut Ui, config: &Config) -> SettingsAction {
    let mut new_config = config.clone();
    if ui.checkbox(&mut new_config.show_button, "Show button").changed() {
        return SettingsAction::ApplyChanges { new_config };
    }
    SettingsAction::None
}
```

### ❌ Don't: Deep nested config structures
```rust
// BAD - Hard to access, serialize
struct Config {
    pub ui: UiConfig {
        pub settings: SettingsConfig {
            pub window: WindowConfig {
                pub general: GeneralConfig { ... }
            }
        }
    }
}
```

### ✅ Do: Flat structure with sections
```rust
// GOOD - Follows existing Config pattern
struct Config {
    pub audio: AudioConfig,
    pub transcription: TranscriptionConfig,
    pub ui: UiConfig,  // Only 2 levels max
}
```

---

## Testing Patterns

Based on Hush test structure:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_transitions() {
        let mut state = SettingsWindowState::Closed;
        assert!(!state.is_open());

        state = SettingsWindowState::open_with_tab(TabId::General);
        assert!(state.is_open());
        assert_eq!(state.active_tab(), Some(TabId::General));
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml = toml::to_string(&config).unwrap();
        let loaded: Config = toml::from_str(&toml).unwrap();
        assert_eq!(config.ui.theme, loaded.ui.theme);
    }
}
```

---

## System Theme Detection

New requirement: Respect system dark mode

```bash
# Add dependency to Cargo.toml
dark-light = "1.0"
```

```rust
use dark_light::Mode;

pub fn detect_system_theme() -> OverlayTheme {
    match dark_light::detect() {
        Mode::Dark => OverlayTheme::Dark,
        Mode::Light => OverlayTheme::Light,
        Mode::Default => OverlayTheme::Dark, // Fallback
    }
}

// In UI config
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum ThemeMode {
    Dark,
    Light,
    System,  // Auto-detect
}

// Apply in render
fn apply_ui_theme(ctx: &egui::Context, mode: ThemeMode) {
    let theme = match mode {
        ThemeMode::Dark => OverlayTheme::Dark,
        ThemeMode::Light => OverlayTheme::Light,
        ThemeMode::System => detect_system_theme(),
    };
    apply_theme(ctx, theme);
}
```

---

## Summary: Pattern Checklist

When implementing settings window, follow these discovered patterns:

- [ ] Use enum for state (`SettingsWindowState`)
- [ ] Use enum for actions (`SettingsAction`)
- [ ] Pure render functions (no side effects)
- [ ] Pattern matching for tab selection
- [ ] Flat config structure with serde
- [ ] State methods for transitions
- [ ] Return actions from render, handle in app loop
- [ ] Clone configs for editing (not mutate in place)
- [ ] Save/load following existing `Config` pattern
- [ ] System theme detection for `ThemeMode::System`

**Reference implementations:**
- State pattern: `src/overlay/state.rs`
- Render pattern: `src/overlay/ui.rs`
- Config pattern: `src/config/settings.rs`
- Window pattern: `src/overlay/window.rs`
