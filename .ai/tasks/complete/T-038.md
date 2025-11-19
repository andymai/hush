# T-038: Fix Hotkey Threading Model for macOS

**Priority:** High
**Effort:** Low-Medium (3-4 hours)
**Type:** Platform Support - Architecture
**Status:** Available
**Created:** 2025-11-19

---

## Problem Statement

The `global-hotkey` crate requires the hotkey manager and event loop to run on the **main thread** on macOS. This is a platform requirement due to macOS's event handling architecture.

Current implementation may not respect this constraint, potentially causing:
- Hotkeys not registering
- Event loop panics
- Application hangs

### macOS Constraint

From `global-hotkey` documentation:
> **macOS**: The event loop must be created and run on the main thread. The GlobalHotKeyManager must also be created on the main thread.

---

## Goals

1. ✅ Ensure GlobalHotKeyManager is created on main thread
2. ✅ Run event loop on main thread (not tokio thread pool)
3. ✅ Maintain Linux compatibility (no main thread requirement)
4. ✅ Preserve async architecture for other components
5. ✅ Test hotkey registration and triggering on macOS

---

## Current Architecture Assessment

Need to verify:
1. Where is GlobalHotKeyManager created?
2. Which thread runs the event loop?
3. Is tokio runtime blocking main thread access?

### Discovery Steps

```bash
# Find GlobalHotKeyManager usage
rg "GlobalHotKeyManager" --type rust -A 5

# Find hotkey initialization
rg "register_hotkey|HotkeyAdapter" --type rust -A 10

# Check if main thread is blocked
rg "tokio::main|tokio::runtime" --type rust

# Check event loop
rg "event_loop|run_event_loop" --type rust
```

---

## Implementation Strategies

### Strategy A: Pin Main Thread to Event Loop (RECOMMENDED)

Run event loop on main thread, spawn tokio runtime on separate thread:

```rust
// src/main.rs

#[cfg(target_os = "macos")]
fn main() {
    // macOS: Main thread must handle UI events
    macos_main();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    // Linux: Can use tokio::main directly
    linux_main();
}

#[cfg(target_os = "macos")]
fn macos_main() {
    use std::sync::mpsc;

    // Create hotkey manager on main thread
    let hotkey_manager = global_hotkey::GlobalHotKeyManager::new()
        .expect("Failed to create hotkey manager");

    // Register hotkeys
    let hotkey_id = register_hotkeys(&hotkey_manager)
        .expect("Failed to register hotkeys");

    // Create event loop on main thread
    let event_loop = winit::event_loop::EventLoop::new();

    // Channel to communicate with tokio runtime
    let (tx, rx) = mpsc::channel();

    // Spawn tokio runtime on background thread
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            run_application(rx).await.unwrap();
        });
    });

    // Run event loop on main thread
    event_loop.run(move |event, _, control_flow| {
        use winit::event::{Event, WindowEvent};
        use winit::event_loop::ControlFlow;

        *control_flow = ControlFlow::Wait;

        match event {
            Event::GlobalHotKeyEvent(hotkey_event) => {
                if hotkey_event.id == hotkey_id {
                    // Send hotkey event to tokio runtime
                    tx.send(AppEvent::HotkeyTriggered).unwrap();
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                tx.send(AppEvent::Shutdown).unwrap();
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}

#[cfg(not(target_os = "macos"))]
#[tokio::main]
async fn linux_main() {
    // Linux can use tokio::main directly
    run_application_linux().await.unwrap();
}
```

### Strategy B: Conditional Event Loop

Keep current architecture, add platform-specific event handling:

```rust
// src/hotkey/mod.rs

pub struct HotkeyManager {
    #[cfg(target_os = "macos")]
    event_loop: winit::event_loop::EventLoop<()>,

    manager: global_hotkey::GlobalHotKeyManager,
    // ...
}

impl HotkeyManager {
    #[cfg(target_os = "macos")]
    pub fn new() -> Result<Self> {
        // Must be called on main thread
        assert!(is_main_thread(), "HotkeyManager must be created on main thread on macOS");

        let event_loop = winit::event_loop::EventLoop::new();
        let manager = global_hotkey::GlobalHotKeyManager::new()?;

        Ok(Self { event_loop, manager })
    }

    #[cfg(not(target_os = "macos"))]
    pub fn new() -> Result<Self> {
        // Linux: No main thread requirement
        let manager = global_hotkey::GlobalHotKeyManager::new()?;
        Ok(Self { manager })
    }
}

#[cfg(target_os = "macos")]
fn is_main_thread() -> bool {
    use cocoa::appkit::NSThread;
    unsafe { NSThread::isMainThread() }
}
```

### Strategy C: Use global-hotkey's Built-in Event Loop

Simplest approach if current implementation allows:

```rust
// src/adapters/hotkey/hotkey_adapter.rs

use global_hotkey::{GlobalHotKeyManager, GlobalHotKeyEvent};
use global_hotkey::hotkey::{HotKey, Code, Modifiers};

pub struct HotkeyAdapter {
    manager: GlobalHotKeyManager,
}

impl HotkeyAdapter {
    pub fn new() -> Result<Self> {
        #[cfg(target_os = "macos")]
        {
            // Verify we're on main thread
            if !is_main_thread() {
                return Err(HushError::Platform(
                    "HotkeyAdapter must be created on main thread on macOS".to_string()
                ));
            }
        }

        let manager = GlobalHotKeyManager::new()
            .map_err(|e| HushError::Platform(format!("Failed to create hotkey manager: {:?}", e)))?;

        Ok(Self { manager })
    }

    pub fn register(&self, key_combo: &str) -> Result<u32> {
        // Parse key combo (e.g., "Ctrl+Alt+V")
        let hotkey = parse_hotkey(key_combo)?;

        self.manager.register(hotkey)
            .map_err(|e| HushError::Hotkey(format!("Failed to register hotkey: {:?}", e)))?;

        Ok(hotkey.id())
    }

    pub async fn wait_for_hotkey(&self) -> Result<u32> {
        // Use GlobalHotKeyEvent receiver
        let receiver = GlobalHotKeyEvent::receiver();

        // On macOS, this must run on main thread
        #[cfg(target_os = "macos")]
        {
            // Block on receiver (already on main thread)
            let event = receiver.recv()
                .map_err(|e| HushError::Hotkey(format!("Hotkey event error: {:?}", e)))?;
            Ok(event.id)
        }

        #[cfg(not(target_os = "macos"))]
        {
            // Linux: Can use async
            tokio::task::spawn_blocking(move || {
                receiver.recv()
                    .map(|event| event.id)
                    .map_err(|e| HushError::Hotkey(format!("Hotkey event error: {:?}", e)))
            }).await?
        }
    }
}
```

---

## Recommended Approach

**Strategy C + Conditional Main Thread Check**

1. Keep HotkeyAdapter creation on main thread (add assertion)
2. Use global-hotkey's built-in event receiver
3. On macOS: Initialize before tokio runtime
4. On Linux: Current behavior unchanged

**Minimal code changes, maximum compatibility.**

---

## Implementation Steps

### 1. Add Main Thread Check

```rust
// src/adapters/hotkey/mod.rs

#[cfg(target_os = "macos")]
fn ensure_main_thread() -> Result<()> {
    use cocoa::appkit::NSThread;

    unsafe {
        if !NSThread::isMainThread() {
            return Err(HushError::Platform(
                "Hotkey initialization must happen on main thread on macOS".to_string()
            ));
        }
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn ensure_main_thread() -> Result<()> {
    Ok(())
}
```

### 2. Update HotkeyAdapter::new()

```rust
impl HotkeyAdapter {
    pub fn new() -> Result<Self> {
        // Ensure we're on main thread (macOS requirement)
        ensure_main_thread()?;

        let manager = GlobalHotKeyManager::new()
            .map_err(|e| HushError::Platform(format!("Failed to create hotkey manager: {:?}", e)))?;

        info!("Hotkey manager initialized successfully");

        #[cfg(target_os = "macos")]
        info!("macOS: Hotkey manager created on main thread");

        Ok(Self { manager })
    }
}
```

### 3. Update main.rs Initialization Order

```rust
// src/main.rs

fn main() -> Result<()> {
    // Initialize logging first
    init_logging()?;

    #[cfg(target_os = "macos")]
    {
        info!("macOS detected: Initializing hotkeys on main thread");
        // Create hotkey manager BEFORE tokio runtime
        let hotkey_adapter = HotkeyAdapter::new()?;

        // Pass to tokio runtime
        return run_with_hotkeys(hotkey_adapter);
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Linux: Use tokio::main as normal
        tokio::runtime::Runtime::new()?.block_on(async {
            run_application().await
        })
    }
}

#[cfg(target_os = "macos")]
fn run_with_hotkeys(hotkey_adapter: HotkeyAdapter) -> Result<()> {
    use std::sync::Arc;

    let hotkey_adapter = Arc::new(hotkey_adapter);

    // Create tokio runtime
    let runtime = tokio::runtime::Runtime::new()?;

    // Clone for event loop
    let hotkey_for_events = hotkey_adapter.clone();

    // Spawn event listener on main thread
    std::thread::spawn(move || {
        runtime.block_on(async {
            run_application_with_hotkeys(hotkey_for_events).await
        })
    });

    // Keep main thread alive
    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
```

### 4. Update Application to Accept Hotkey Adapter

```rust
async fn run_application_with_hotkeys(hotkey_adapter: Arc<HotkeyAdapter>) -> Result<()> {
    // Use provided hotkey adapter instead of creating new one
    let app = Application::new_with_hotkeys(hotkey_adapter)?;
    app.run().await
}
```

---

## Success Criteria

- [ ] HotkeyAdapter creation succeeds on macOS
- [ ] Main thread assertion passes on macOS
- [ ] Hotkey registration works (Cmd+Alt+V or equivalent)
- [ ] Hotkey triggering works (events received)
- [ ] No panics or hangs on macOS
- [ ] Linux builds unaffected (hotkeys still work)
- [ ] Documentation updated with threading model notes

---

## Verification Steps

```bash
# On macOS:
# 1. Build application
cargo build

# 2. Run and check logs
RUST_LOG=debug ./target/debug/hush listen 2>&1 | grep -i "hotkey\|main thread"
# Expected: "macOS: Hotkey manager created on main thread"

# 3. Test hotkey registration
./target/debug/hush listen
# Press configured hotkey (Cmd+Alt+V)
# Expected: Recording starts

# 4. Test in wrong thread (should fail)
# Create test that tries to init hotkey in tokio thread
# Expected: Error about main thread requirement

# On Linux:
# Verify no regression
cargo build && cargo test
./target/debug/hush listen
# Press hotkey (Ctrl+Alt+V)
# Expected: Recording starts (behavior unchanged)
```

---

## Files to Modify

- `src/main.rs` - Conditional main thread initialization
- `src/adapters/hotkey/hotkey_adapter.rs` - Add main thread check
- `src/adapters/hotkey/mod.rs` - Platform-specific helpers
- `src/application/mod.rs` - Accept hotkey adapter in constructor

## Files to Create

- `src/adapters/hotkey/macos.rs` - macOS-specific threading helpers (optional)

---

## Dependencies

**Blocks:**
- T-039 (integration tests need hotkeys working)

**Blocked by:**
- T-033 (needs global-hotkey dependency verified)

---

## References

- **global-hotkey docs**: https://docs.rs/global-hotkey/latest/global_hotkey/
- **macOS threading**: https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/RunLoopManagement/RunLoopManagement.html
- **winit event loop**: https://docs.rs/winit/latest/winit/event_loop/
- **cocoa NSThread**: https://docs.rs/cocoa/latest/cocoa/appkit/struct.NSThread.html

---

## Testing Notes

**Main Thread Detection:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn test_main_thread_detection() {
        // This test runs on main thread
        assert!(ensure_main_thread().is_ok());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_non_main_thread_fails() {
        use std::thread;

        // Spawn background thread
        let handle = thread::spawn(|| {
            ensure_main_thread()
        });

        let result = handle.join().unwrap();
        assert!(result.is_err());
    }
}
```

---

## Known Issues

1. **Tokio blocking**: If tokio runtime blocks main thread, hotkeys won't work
   - Solution: Spawn runtime on background thread (Strategy A)

2. **Event loop conflicts**: If overlay or other components need main thread
   - Solution: Share single event loop across components

3. **Testing**: Hard to test main thread requirement in CI
   - Solution: Add manual testing checklist for macOS

---

## Fallback Plan

If threading proves too complex:
- Use `rdev` crate instead of `global-hotkey`
- `rdev` has different threading model
- Trade-off: Less reliable on some systems

Only consider if main thread approach fails after 2-3 attempts.
