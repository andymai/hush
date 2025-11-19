# T-037: Implement macOS System Tray Adapter

**Priority:** Medium
**Effort:** Medium (4-6 hours)
**Type:** Platform Support - UI Integration
**Status:** Available
**Created:** 2025-11-19

---

## Problem Statement

System tray integration uses `ksni` (StatusNotifierItem/D-Bus), which is Linux-specific. macOS needs native NSStatusBar integration via AppKit/Cocoa.

### Current Implementation

```rust
// src/tray/adapters/ksni_adapter.rs
// Uses D-Bus protocol (Linux freedesktop standard)
```

This doesn't exist on macOS, which uses NSStatusBar in the menu bar.

---

## Goals

1. ✅ Create macOS SystemTray adapter using NSStatusBar
2. ✅ Implement menu bar icon with menu items
3. ✅ Support state updates (idle, recording, processing)
4. ✅ Handle menu item clicks (Start/Stop, History, Settings, Quit)
5. ✅ Use factory pattern for platform selection
6. ✅ Alternative: Use `tray-icon` crate for cross-platform abstraction

---

## Implementation Strategy

**Option A: Use `tray-icon` Crate** (RECOMMENDED)

- Cross-platform crate with macOS NSStatusBar support
- Simpler implementation, less macOS-specific code
- Well-maintained, active development
- Used by Tauri and other production apps

**Option B: Custom NSStatusBar Wrapper**

- Direct Cocoa/AppKit bindings
- More control over macOS-specific features
- More code to maintain
- Requires Objective-C FFI

**Recommendation:** Use `tray-icon` for MVP, can always add custom implementation later if needed.

---

## Implementation Steps (Option A: tray-icon)

### 1. Add Dependency

**Already done in T-033:**
```toml
[target.'cfg(target_os = "macos")'.dependencies]
tray-icon = { version = "0.14", optional = true }
```

### 2. Create macOS Tray Adapter

**New file: `src/tray/adapters/macos_tray_adapter.rs`**

```rust
use crate::core::traits::SystemTray;
use crate::core::error::HushError;
use crate::Result;
use async_trait::async_trait;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};
use tracing::{debug, info, warn};
use std::sync::Arc;
use parking_lot::Mutex;

pub struct MacOSTrayAdapter {
    tray_icon: TrayIcon,
    menu: Menu,
    state: Arc<Mutex<TrayState>>,
}

#[derive(Debug, Clone)]
enum TrayState {
    Idle,
    Recording,
    Processing,
    Error(String),
}

impl MacOSTrayAdapter {
    pub fn new() -> Result<Self> {
        info!("Initializing macOS system tray adapter");

        // Load icon (embedded or from file)
        let icon = Self::load_icon()?;

        // Create menu
        let menu = Self::create_menu()?;

        // Create tray icon
        let tray_icon = TrayIconBuilder::new()
            .with_tooltip("Hush - Voice to Text")
            .with_icon(icon)
            .with_menu(Box::new(menu.clone()))
            .build()
            .map_err(|e| HushError::Platform(format!("Failed to create tray icon: {:?}", e)))?;

        // Set up event handlers
        let state = Arc::new(Mutex::new(TrayState::Idle));
        Self::setup_event_handlers(state.clone());

        Ok(Self {
            tray_icon,
            menu,
            state,
        })
    }

    fn load_icon() -> Result<Icon> {
        // Load icon from embedded resource or file
        // For now, use a simple colored icon
        let icon_bytes = include_bytes!("../../../assets/icon.png");

        Icon::from_png(icon_bytes)
            .map_err(|e| HushError::Platform(format!("Failed to load tray icon: {:?}", e)))
    }

    fn create_menu() -> Result<Menu> {
        let menu = Menu::new();

        // Status item (non-clickable)
        let status_item = MenuItem::new("Status: Idle", false, None);
        menu.append(&status_item)
            .map_err(|e| HushError::Platform(format!("Failed to add menu item: {:?}", e)))?;

        // Separator
        menu.append(&PredefinedMenuItem::separator())
            .map_err(|e| HushError::Platform(format!("Failed to add separator: {:?}", e)))?;

        // Start/Stop Recording
        let start_item = MenuItem::new("Start Recording", true, Some("Cmd+Shift+V"));
        menu.append(&start_item)?;

        // View History
        let history_item = MenuItem::new("View History", true, None);
        menu.append(&history_item)?;

        // Settings
        let settings_item = MenuItem::new("Settings...", true, None);
        menu.append(&settings_item)?;

        // Separator
        menu.append(&PredefinedMenuItem::separator())?;

        // About
        let about_item = MenuItem::new("About Hush", true, None);
        menu.append(&about_item)?;

        // Quit
        let quit_item = PredefinedMenuItem::quit(Some("Quit Hush"));
        menu.append(&quit_item)?;

        Ok(menu)
    }

    fn setup_event_handlers(state: Arc<Mutex<TrayState>>) {
        // Spawn event listener
        std::thread::spawn(move || {
            let menu_channel = MenuEvent::receiver();

            loop {
                if let Ok(event) = menu_channel.recv() {
                    debug!("Tray menu event: {:?}", event);
                    Self::handle_menu_event(event, state.clone());
                }
            }
        });
    }

    fn handle_menu_event(event: MenuEvent, state: Arc<Mutex<TrayState>>) {
        // Handle menu item clicks
        // This would integrate with the main application logic
        debug!("Menu item clicked: {:?}", event.id);

        // Example: Toggle recording state
        // In real implementation, this would call application callbacks
    }

    fn update_icon_for_state(&mut self, state: &TrayState) -> Result<()> {
        // Update icon based on state (idle, recording, processing)
        let icon = match state {
            TrayState::Idle => Self::load_icon_idle()?,
            TrayState::Recording => Self::load_icon_recording()?,
            TrayState::Processing => Self::load_icon_processing()?,
            TrayState::Error(_) => Self::load_icon_error()?,
        };

        self.tray_icon.set_icon(Some(icon))
            .map_err(|e| HushError::Platform(format!("Failed to update icon: {:?}", e)))?;

        Ok(())
    }

    fn update_menu_for_state(&mut self, state: &TrayState) -> Result<()> {
        // Update menu items based on state
        // For example, change "Start Recording" to "Stop Recording" when active

        // This requires recreating the menu or updating specific items
        // Implementation depends on tray-icon API capabilities

        Ok(())
    }

    // Icon loading helpers (load different icons for different states)
    fn load_icon_idle() -> Result<Icon> {
        let bytes = include_bytes!("../../../assets/icon-idle.png");
        Icon::from_png(bytes)
            .map_err(|e| HushError::Platform(format!("Failed to load idle icon: {:?}", e)))
    }

    fn load_icon_recording() -> Result<Icon> {
        let bytes = include_bytes!("../../../assets/icon-recording.png");
        Icon::from_png(bytes)
            .map_err(|e| HushError::Platform(format!("Failed to load recording icon: {:?}", e)))
    }

    fn load_icon_processing() -> Result<Icon> {
        let bytes = include_bytes!("../../../assets/icon-processing.png");
        Icon::from_png(bytes)
            .map_err(|e| HushError::Platform(format!("Failed to load processing icon: {:?}", e)))
    }

    fn load_icon_error() -> Result<Icon> {
        let bytes = include_bytes!("../../../assets/icon-error.png");
        Icon::from_png(bytes)
            .map_err(|e| HushError::Platform(format!("Failed to load error icon: {:?}", e)))
    }
}

#[async_trait]
impl SystemTray for MacOSTrayAdapter {
    async fn set_status(&mut self, status: &str) -> Result<()> {
        info!("Setting tray status: {}", status);

        let state = match status {
            "idle" => TrayState::Idle,
            "recording" => TrayState::Recording,
            "processing" => TrayState::Processing,
            s if s.starts_with("error:") => TrayState::Error(s.to_string()),
            _ => TrayState::Idle,
        };

        *self.state.lock() = state.clone();
        self.update_icon_for_state(&state)?;
        self.update_menu_for_state(&state)?;

        Ok(())
    }

    async fn show_notification(&mut self, title: &str, message: &str) -> Result<()> {
        // Delegate to notification adapter
        // Or use tray_icon's notification support if available
        debug!("Tray notification: {} - {}", title, message);
        Ok(())
    }
}
```

### 3. Create Icon Assets

**Create: `assets/` directory with PNG icons**

For MVP, create simple programmatic icons:

```rust
// src/tray/adapters/icon_generator.rs

use image::{ImageBuffer, Rgba};
use tray_icon::Icon;

pub fn generate_idle_icon() -> Icon {
    // 32x32 icon with microphone symbol (gray)
    let img = ImageBuffer::from_fn(32, 32, |x, y| {
        // Simple microphone shape
        if is_microphone_pixel(x, y) {
            Rgba([128, 128, 128, 255])  // Gray
        } else {
            Rgba([0, 0, 0, 0])  // Transparent
        }
    });

    let bytes = img.into_vec();
    Icon::from_rgba(bytes, 32, 32).unwrap()
}

pub fn generate_recording_icon() -> Icon {
    // Red microphone icon
    let img = ImageBuffer::from_fn(32, 32, |x, y| {
        if is_microphone_pixel(x, y) {
            Rgba([255, 0, 0, 255])  // Red
        } else {
            Rgba([0, 0, 0, 0])
        }
    });

    let bytes = img.into_vec();
    Icon::from_rgba(bytes, 32, 32).unwrap()
}

fn is_microphone_pixel(x: u32, y: u32) -> bool {
    // Simple microphone shape: circle + line
    let cx = 16.0;
    let cy = 12.0;
    let r = 6.0;

    let dx = (x as f32 - cx).abs();
    let dy = (y as f32 - cy).abs();

    // Circle (microphone head)
    if dx * dx + dy * dy <= r * r {
        return true;
    }

    // Stem
    if x >= 14 && x <= 18 && y >= 18 && y <= 28 {
        return true;
    }

    false
}
```

### 4. Refactor Tray Module for Platform Selection

**Update: `src/tray/adapters/mod.rs`**

```rust
#[cfg(target_os = "linux")]
mod ksni_adapter;
#[cfg(target_os = "linux")]
pub use ksni_adapter::KsniAdapter;

#[cfg(target_os = "macos")]
mod macos_tray_adapter;
#[cfg(target_os = "macos")]
pub use macos_tray_adapter::MacOSTrayAdapter;

use crate::core::traits::SystemTray;
use crate::Result;

/// Factory function to create platform-appropriate tray adapter
pub fn create_system_tray() -> Result<Box<dyn SystemTray>> {
    #[cfg(target_os = "linux")]
    {
        Ok(Box::new(KsniAdapter::new()?))
    }

    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(MacOSTrayAdapter::new()?))
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Err(crate::core::error::HushError::Platform(
            "Unsupported platform for system tray".to_string()
        ))
    }
}
```

### 5. Update Application Code

```rust
// Replace direct instantiation with factory
use crate::tray::adapters::create_system_tray;
let system_tray = create_system_tray()?;
```

---

## Success Criteria

- [ ] MacOSTrayAdapter compiles on macOS
- [ ] Tray icon appears in macOS menu bar
- [ ] Menu items display correctly
- [ ] Menu item clicks trigger events
- [ ] Icon updates based on state (idle/recording/processing)
- [ ] Menu keyboard shortcuts work (Cmd+Shift+V)
- [ ] Factory pattern selects correct adapter per platform
- [ ] Linux builds unaffected (ksni adapter still works)

---

## Verification Steps

```bash
# On macOS:
# 1. Build with system-tray feature
cargo build --features system-tray

# 2. Run application
./target/debug/hush listen

# 3. Check menu bar for tray icon
# Should see microphone icon in top-right menu bar

# 4. Click tray icon, verify menu appears
# Expected: Status, Start Recording, History, Settings, About, Quit

# 5. Test state changes
# Start recording → icon should turn red
# Processing → icon should show processing indicator

# On Linux:
# Verify no regression
cargo build --features system-tray
./target/debug/hush listen
# Tray icon should appear in system tray (ksni)
```

---

## Files to Create

- `src/tray/adapters/macos_tray_adapter.rs` (~300 lines)
- `src/tray/adapters/icon_generator.rs` (~100 lines, optional)
- `assets/icon-*.png` (4 icons: idle, recording, processing, error)

## Files to Modify

- `src/tray/adapters/mod.rs` - Add platform selection factory
- `src/application/mod.rs` - Use factory instead of direct instantiation

---

## Dependencies

**Blocks:**
- T-039 (integration tests need tray working)
- T-040 (documentation needs tray instructions)

**Blocked by:**
- T-033 (needs tray-icon dependency)

---

## References

- **tray-icon crate**: https://docs.rs/tray-icon/latest/tray_icon/
- **macOS NSStatusBar**: https://developer.apple.com/documentation/appkit/nsstatusbar
- **tray-icon examples**: https://github.com/tauri-apps/tray-icon/tree/main/examples
- **Tauri tray implementation**: https://github.com/tauri-apps/tauri (reference)

---

## Alternative: Custom NSStatusBar (Option B)

If `tray-icon` doesn't meet requirements:

```rust
// Use cocoa crate directly
use cocoa::appkit::{NSStatusBar, NSStatusItem, NSMenu, NSMenuItem};
use cocoa::base::{id, nil};
use objc::runtime::{Object, Sel};

// This requires more Objective-C FFI code
// Only pursue if tray-icon has critical limitations
```

---

## Known Limitations

1. **Menu updates**: tray-icon may require menu recreation for state changes
2. **Icon quality**: Programmatic icons are basic; consider professional icons for production
3. **Retina displays**: Ensure @2x icons for high-DPI screens
4. **Dark mode**: Icons should work in both light and dark macOS themes

These are acceptable for MVP.

---

## Future Enhancements

- T-038: Add transcription history in tray menu
- T-039: Add quick settings in tray menu
- T-040: Support tray icon animations (recording pulse)
- T-041: Professional icon design

Priority: Low (MVP is functional tray)
