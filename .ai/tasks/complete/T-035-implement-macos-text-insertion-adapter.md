# T-035: Implement macOS Text Insertion Adapter

**Priority:** Critical
**Effort:** High (1-2 days)
**Type:** Platform Support - Core Feature
**Status:** Available
**Created:** 2025-11-19

---

## Problem Statement

Text insertion is Linux-only, using UInput (kernel interface) and X11 (window system). macOS needs a native adapter using CoreGraphics CGEvent API and Accessibility framework.

### Current Implementation

```rust
// src/text/insertion.rs:1-100
pub struct TextInserter {
    uinput_keyboard: Option<UinputKeyboard>,  // Linux only
    enigo: Enigo,                              // Cross-platform fallback
    x11_conn: RustConnection,                  // Linux only
    // ...
}
```

This prevents text insertion from working on macOS.

---

## Goals

1. ✅ Create macOS TextOutput adapter using CGEvent API
2. ✅ Implement Accessibility permission checks
3. ✅ Add window detection using macOS Accessibility API
4. ✅ Provide fallback to enigo if CGEvent fails
5. ✅ Match feature parity with Linux adapter
6. ✅ Use factory pattern for platform selection

---

## Implementation Steps

### 1. Create macOS Adapter Structure

**New file: `src/adapters/text/macos_adapter.rs`**

```rust
use crate::core::traits::TextOutput;
use crate::core::error::HushError;
use crate::Result;
use async_trait::async_trait;
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, CGKeyCode};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use std::thread;
use std::time::Duration;
use tracing::{debug, info, warn};

pub struct MacOSTextAdapter {
    event_source: CGEventSource,
    enigo_fallback: enigo::Enigo,
    accessibility_enabled: bool,
    typing_delay_ms: u64,
}

impl MacOSTextAdapter {
    pub fn new() -> Result<Self> {
        info!("Initializing macOS text insertion adapter");

        // Check Accessibility permissions
        let accessibility_enabled = check_accessibility_permissions();
        if !accessibility_enabled {
            warn!("Accessibility permissions not granted");
            warn!("Text insertion will use fallback method");
            prompt_accessibility_permissions();
        }

        // Create CGEvent source (system-level events)
        let event_source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|e| HushError::Platform(format!("Failed to create CGEventSource: {:?}", e)))?;

        // Initialize enigo as fallback
        let enigo_fallback = enigo::Enigo::new(&enigo::Settings::default())
            .map_err(|e| HushError::Platform(format!("Failed to initialize fallback: {:?}", e)))?;

        Ok(Self {
            event_source,
            enigo_fallback,
            accessibility_enabled,
            typing_delay_ms: 10,
        })
    }

    fn insert_with_cgevent(&mut self, text: &str) -> Result<()> {
        debug!("Inserting text using CGEvent API");

        for ch in text.chars() {
            self.type_character(ch)?;
            thread::sleep(Duration::from_millis(self.typing_delay_ms));
        }

        Ok(())
    }

    fn type_character(&mut self, ch: char) -> Result<()> {
        // Convert character to keycode and modifiers
        let (keycode, needs_shift) = char_to_keycode(ch)?;

        // Press shift if needed
        if needs_shift {
            self.press_key(CGKeyCode(56), true)?; // 56 = Shift
        }

        // Press and release key
        self.press_key(keycode, true)?;
        self.press_key(keycode, false)?;

        // Release shift
        if needs_shift {
            self.press_key(CGKeyCode(56), false)?;
        }

        Ok(())
    }

    fn press_key(&self, keycode: CGKeyCode, key_down: bool) -> Result<()> {
        let event_type = if key_down {
            CGEventType::KeyDown
        } else {
            CGEventType::KeyUp
        };

        let event = CGEvent::new_keyboard_event(self.event_source.clone(), keycode.0, key_down)
            .map_err(|e| HushError::Platform(format!("Failed to create keyboard event: {:?}", e)))?;

        event.post(CGEventTapLocation::HID);
        Ok(())
    }

    fn get_focused_window(&self) -> Result<MacOSWindowInfo> {
        // Use Accessibility API to get focused window
        get_focused_window_info()
    }
}

#[async_trait]
impl TextOutput for MacOSTextAdapter {
    async fn insert_text(&mut self, text: &str) -> Result<()> {
        info!("Inserting text (macOS): {} chars", text.len());

        // Try CGEvent first if permissions are granted
        if self.accessibility_enabled {
            match self.insert_with_cgevent(text) {
                Ok(()) => {
                    debug!("Text inserted successfully via CGEvent");
                    return Ok(());
                }
                Err(e) => {
                    warn!("CGEvent insertion failed: {}, falling back to enigo", e);
                }
            }
        }

        // Fallback to enigo
        debug!("Using enigo fallback");
        self.insert_with_enigo(text)
    }

    async fn get_active_window(&self) -> Result<String> {
        let window = self.get_focused_window()?;
        Ok(format!("{} - {}", window.title, window.app_name))
    }
}
```

### 2. Implement Accessibility Helpers

```rust
// src/adapters/text/macos_accessibility.rs

use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;

pub fn check_accessibility_permissions() -> bool {
    // Check if app has Accessibility permissions
    // This uses private API via core-foundation
    unsafe {
        let options = CFDictionary::from_CFType_pairs(&[]);
        // AXIsProcessTrusted is the official way to check
        // Returns true if permissions granted
        ax_is_process_trusted_with_options(options.as_concrete_TypeRef())
    }
}

pub fn prompt_accessibility_permissions() {
    use std::process::Command;

    eprintln!("\n⚠️  Accessibility Permissions Required\n");
    eprintln!("Hush needs Accessibility permissions to insert text.");
    eprintln!("\nTo grant permissions:");
    eprintln!("  1. Open System Preferences");
    eprintln!("  2. Go to Privacy & Security → Accessibility");
    eprintln!("  3. Add and enable 'hush' or your terminal app\n");
    eprintln!("Opening System Preferences now...\n");

    // Open System Preferences to Accessibility pane
    let _ = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
}

#[derive(Debug, Clone)]
pub struct MacOSWindowInfo {
    pub title: String,
    pub app_name: String,
    pub bundle_id: String,
    pub is_focused: bool,
}

pub fn get_focused_window_info() -> Result<MacOSWindowInfo> {
    // Use Accessibility API to get frontmost window
    // This is complex - see full implementation in reference code
    // For now, return basic info
    Ok(MacOSWindowInfo {
        title: "Active Window".to_string(),
        app_name: "Unknown App".to_string(),
        bundle_id: "unknown".to_string(),
        is_focused: true,
    })
}

// Helper: Convert character to macOS keycode
pub fn char_to_keycode(ch: char) -> Result<(CGKeyCode, bool)> {
    // Map characters to macOS virtual key codes
    // Reference: https://eastmanreference.com/complete-list-of-applescript-key-codes
    let (code, shift) = match ch {
        'a'..='z' => ((ch as u16 - 'a' as u16), false),
        'A'..='Z' => ((ch as u16 - 'A' as u16), true),
        '0'..='9' => {
            let offset = match ch {
                '0' => 29,
                '1'..='9' => (ch as u16 - '1' as u16) + 18,
                _ => unreachable!(),
            };
            (offset, false)
        }
        ' ' => (49, false),   // Space
        '\n' => (36, false),  // Return
        '\t' => (48, false),  // Tab
        '.' => (47, false),
        ',' => (43, false),
        // Add more special characters as needed
        _ => {
            warn!("Unsupported character: '{}', using space", ch);
            (49, false)
        }
    };

    Ok((CGKeyCode(code), shift))
}

// FFI for Accessibility API
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> bool;
}

unsafe fn ax_is_process_trusted_with_options(options: *const std::ffi::c_void) -> bool {
    AXIsProcessTrustedWithOptions(options)
}
```

### 3. Refactor Text Module for Platform Selection

**Update: `src/adapters/text/mod.rs`**

```rust
#[cfg(target_os = "linux")]
mod x11_adapter;
#[cfg(target_os = "linux")]
pub use x11_adapter::X11TextAdapter;

#[cfg(target_os = "macos")]
mod macos_adapter;
#[cfg(target_os = "macos")]
mod macos_accessibility;
#[cfg(target_os = "macos")]
pub use macos_adapter::MacOSTextAdapter;

use crate::core::traits::TextOutput;
use crate::Result;

/// Factory function to create platform-appropriate text adapter
pub fn create_text_adapter() -> Result<Box<dyn TextOutput>> {
    #[cfg(target_os = "linux")]
    {
        Ok(Box::new(X11TextAdapter::new()?))
    }

    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(MacOSTextAdapter::new()?))
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Err(crate::core::error::HushError::Platform(
            "Unsupported platform for text insertion".to_string()
        ))
    }
}
```

### 4. Update Application Code

**Update: `src/application/mod.rs` or wherever TextInserter is created**

```rust
// Replace direct instantiation with factory
// OLD:
// let text_inserter = TextInserter::new()?;

// NEW:
use crate::adapters::text::create_text_adapter;
let text_output = create_text_adapter()?;
```

---

## Success Criteria

- [ ] MacOSTextAdapter compiles on macOS
- [ ] Accessibility permission check works
- [ ] Text insertion via CGEvent functions correctly
- [ ] Fallback to enigo works when CGEvent fails
- [ ] Window detection returns focused window info
- [ ] Factory pattern selects correct adapter per platform
- [ ] Linux builds unaffected (X11 adapter still works)
- [ ] Integration tests pass on macOS

---

## Verification Steps

```bash
# On macOS:
# 1. Build with new adapter
cargo build

# 2. Test text insertion (requires Accessibility permissions)
./hush listen
# Trigger hotkey, speak, verify text appears

# 3. Test permission check
./hush check-permissions

# 4. Test without permissions (deny in System Preferences)
# Should fall back to enigo and show warning

# On Linux:
# Verify no regression
cargo build && cargo test
./hush listen  # Should work as before
```

---

## Files to Create

- `src/adapters/text/macos_adapter.rs` (~300 lines)
- `src/adapters/text/macos_accessibility.rs` (~200 lines)

## Files to Modify

- `src/adapters/text/mod.rs` - Add platform selection
- `src/adapters/text/x11_adapter.rs` - Ensure X11TextAdapter implements TextOutput trait
- `src/application/mod.rs` - Use factory instead of direct instantiation
- `src/core/traits.rs` - Verify TextOutput trait is platform-agnostic

---

## Dependencies

**Blocks:**
- T-039 (macOS integration tests need text insertion working)
- T-040 (macOS docs need Accessibility permission instructions)

**Blocked by:**
- T-033 (needs core-graphics dependency)
- T-034 (needs framework checks in build.rs)

---

## References

- **CGEvent API**: https://developer.apple.com/documentation/coregraphics/cgevent
- **Accessibility API**: https://developer.apple.com/documentation/applicationservices/accessibility
- **macOS Virtual Key Codes**: https://eastmanreference.com/complete-list-of-applescript-key-codes
- **Rust core-graphics**: https://docs.rs/core-graphics/latest/core_graphics/
- **enigo macOS support**: https://docs.rs/enigo/0.2/enigo/

---

## Testing Notes

**Accessibility Permissions:**
- First run will prompt for permissions
- User must manually enable in System Preferences
- App needs to be restarted after granting permissions
- Test both "granted" and "denied" scenarios

**Character Mapping:**
- Start with basic ASCII (a-z, 0-9, space, punctuation)
- Add extended characters (Unicode) in follow-up task if needed
- Special keys (arrows, function keys) may need separate handling

**Performance:**
- CGEvent should be faster than enigo
- Typical latency: 1-2ms per character
- Compare with Linux UInput performance

---

## Known Limitations

1. **Sandboxed Apps**: Some apps block external keyboard events (e.g., password fields)
2. **Secure Input**: Apps in "secure input mode" may reject CGEvent (e.g., sudo prompts)
3. **Unicode**: Complex Unicode characters may need special handling
4. **IME**: Input Method Editors (Chinese, Japanese) not yet supported

These are acceptable for MVP; can be addressed in future tasks.
