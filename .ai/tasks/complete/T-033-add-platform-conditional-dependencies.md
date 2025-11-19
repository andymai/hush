# T-033: Add Platform-Conditional Dependencies for macOS Support

**Priority:** High
**Effort:** Low (2-3 hours)
**Type:** Platform Support - Foundation
**Status:** Available
**Created:** 2025-11-19

---

## Problem Statement

Currently, `Cargo.toml` declares Linux-specific dependencies globally, making it impossible to build on macOS. Dependencies like `x11rb`, `input-linux`, and `ksni` are Linux-only and cause compilation failures on macOS.

### Current Issues

```toml
# Lines 50-54: Linux-only dependencies declared globally
x11rb = "0.13"
input-linux = "0.7"
ksni = { version = "0.2", optional = true }
```

These dependencies must be platform-conditional to support multi-platform builds.

---

## Goals

1. ✅ Move Linux-specific dependencies to `[target.'cfg(target_os = "linux")'.dependencies]`
2. ✅ Add macOS-specific dependencies under `[target.'cfg(target_os = "macos")'.dependencies]`
3. ✅ Keep cross-platform dependencies in main `[dependencies]` section
4. ✅ Ensure existing Linux builds are unaffected
5. ✅ Enable successful `cargo check` on macOS (without CUDA)

---

## Implementation Steps

### 1. Refactor Existing Dependencies

**Move to Linux-only section:**
```toml
[target.'cfg(target_os = "linux")'.dependencies]
x11rb = "0.13"
input-linux = "0.7"
ksni = { version = "0.2", optional = true }
```

### 2. Add macOS Dependencies

```toml
[target.'cfg(target_os = "macos")'.dependencies]
# macOS system APIs
cocoa = "0.25"              # AppKit/Cocoa bindings
core-graphics = "0.23"      # CGEvent API for keyboard simulation
core-foundation = "0.9"     # macOS system frameworks
objc = "0.2"                # Objective-C runtime

# Cross-platform tray alternative (supports macOS)
tray-icon = { version = "0.14", optional = true }
```

### 3. Verify Cross-Platform Dependencies

Keep these in main `[dependencies]` (already cross-platform):
- ✅ `cpal = "0.15"` - Supports CoreAudio on macOS
- ✅ `global-hotkey = "0.6"` - Supports macOS
- ✅ `enigo = "0.2"` - Supports macOS
- ✅ `arboard = "3.0"` - Supports macOS
- ✅ `egui_overlay = "0.5"` - Supports wgpu/Metal
- ✅ `candle-*` - Supports Metal backend
- ✅ `notify-rust = "4.10"` - Has basic macOS support

### 4. Update Feature Flags

```toml
[features]
default = ["notifications", "system-tray"]

# Platform-specific defaults
notifications = ["dep:notify-rust"]

# System tray: use platform-specific implementation
system-tray = []

[target.'cfg(target_os = "linux")'.dependencies]
ksni = { version = "0.2", optional = true }

[target.'cfg(target_os = "macos")'.dependencies]
tray-icon = { version = "0.14", optional = true }
```

---

## Success Criteria

- [ ] `cargo check` succeeds on Linux (existing functionality preserved)
- [ ] `cargo check` succeeds on macOS (no Linux dependency errors)
- [ ] `cargo build --no-default-features` works on both platforms
- [ ] Platform-specific dependencies only pulled on their respective platforms
- [ ] All existing Linux features continue to work

---

## Verification Steps

```bash
# 1. On Linux: Verify existing build still works
cargo clean
cargo check
cargo build

# 2. On macOS: Verify dependencies resolve (may fail in later stages, but deps should resolve)
cargo clean
cargo check --lib

# 3. Verify platform-specific dependency resolution
cargo tree | grep x11rb      # Should only appear on Linux
cargo tree | grep cocoa      # Should only appear on macOS

# 4. Check feature flag resolution
cargo check --no-default-features
cargo check --features notifications
cargo check --features system-tray
```

---

## Files to Modify

- `Cargo.toml` (Lines 23-103)
  - Reorganize dependencies into platform-specific sections
  - Update feature flags for platform awareness
  - Add macOS-specific dependencies

---

## Dependencies

**Blocks:**
- T-034 (build.rs updates need platform-conditional deps first)
- T-035 (macOS text adapter needs core-graphics dependency)
- T-037 (macOS tray adapter needs tray-icon or cocoa)

**Blocked by:** None (foundation task)

---

## References

- **Rust Platform-Specific Dependencies**: https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#platform-specific-dependencies
- **Existing macOS code**: `src/overlay/window.rs:167-218` (already has macOS conditionals)
- **Cross-platform crates**:
  - `cpal` CoreAudio docs: https://docs.rs/cpal/latest/cpal/
  - `global-hotkey` macOS notes: https://docs.rs/global-hotkey/latest/global_hotkey/
  - `tray-icon`: https://docs.rs/tray-icon/latest/tray_icon/

---

## Notes

- This is a **non-breaking change** for Linux builds
- macOS builds will still fail in later compilation stages (text insertion, tray) until adapters are implemented
- This task enables incremental development of macOS features
- The `metal` feature flag for GPU acceleration will be added in T-036
