# T-045: Fix macOS Hotkey Main Thread Requirement (Complete Implementation)

**Priority:** Critical
**Effort:** High (4-6 hours)
**Type:** Bug Fix - Architecture
**Status:** Available
**Created:** 2025-11-19
**Supersedes:** T-038 (was marked complete but implementation incomplete)

---

## Problem Statement

The `hush listen` command hangs on macOS because `GlobalHotKeyManager` from the `global-hotkey` crate **must** be created on the actual OS main thread, but is currently created from within a tokio worker thread.

### Current Behavior

```bash
$ ./target/debug/hush listen
🎤 Hush Intelligent Listening Mode
═══════════════════════════════════════════════════════
Press Ctrl+Alt+V to start recording
Release to transcribe and insert text
Press Ctrl+C to quit
═══════════════════════════════════════════════════════

[HANGS INDEFINITELY - no hotkey registration, no overlay, no response]
```

### Root Cause Analysis

1. **main.rs (line 51)**: `runtime.block_on(async { ... })` enters tokio runtime
2. **dispatcher.dispatch()**: Called inside async context (tokio worker thread)
3. **handle_listen() (listen.rs:155)**: Creates `HotkeyManager::new("Ctrl+Alt+V")`
4. **HotkeyManager::new() (hotkey/global.rs:48)**: Calls `GlobalHotKeyManager::new()`
5. **GlobalHotKeyManager::new()**: **REQUIRES OS main thread on macOS, but we're on tokio worker**
6. **Result**: Hangs/blocks waiting for main thread that will never come

### Why T-038 Didn't Fix It

T-038 added `ensure_main_thread()` check, BUT:
- The check is **disabled** (src/adapters/hotkey/macos.rs:16-21)
- It just logs a warning and returns `Ok(()` without actually checking
- Comment says "relying on global_hotkey internal handling" - **this is incorrect**
- `global-hotkey` does NOT handle threading internally - YOU must ensure main thread

```rust
// src/adapters/hotkey/macos.rs:16-21
pub fn ensure_main_thread() -> Result<()> {
    // For now, trust that global_hotkey handles main thread requirements
    info!("Hotkey manager initialization (macOS)");
    warn!("Main thread check disabled - relying on global_hotkey internal handling");
    Ok(())  // <- DOES NOTHING!
}
```

---

## Goals

1. ✅ Create `HotkeyManager` on OS main thread BEFORE tokio runtime starts
2. ✅ Pass pre-created hotkey manager through command dispatch
3. ✅ Modify `handle_listen()` to accept optional pre-created hotkey manager
4. ✅ Keep main thread architecture for hotkey event handling
5. ✅ Maintain Linux compatibility (no changes needed)
6. ✅ Enable proper main thread checking in `ensure_main_thread()`

---

## Detailed Implementation Plan

### Phase 1: Enable Real Main Thread Detection (30 min)

**File:** `src/adapters/hotkey/macos.rs`

**Current (disabled):**
```rust
pub fn ensure_main_thread() -> Result<()> {
    info!("Hotkey manager initialization (macOS)");
    warn!("Main thread check disabled - relying on global_hotkey internal handling");
    Ok(())
}
```

**Fixed (actually checks):**
```rust
#[cfg(target_os = "macos")]
pub fn ensure_main_thread() -> Result<()> {
    if !is_main_thread() {
        return Err(anyhow::anyhow!(
            "Hotkey initialization must happen on main thread on macOS.\n\
            \n\
            This is a platform requirement due to AppKit/Cocoa event handling.\n\
            \n\
            Current thread: {:?}\n\
            \n\
            Possible solutions:\n\
            1. Create HotkeyManager before tokio runtime starts\n\
            2. Use a different threading model\n\
            3. Consider using an alternative hotkey library\n\
            \n\
            See docs/macos/THREADING.md for details.",
            std::thread::current().id()
        ));
    }

    info!("✅ Hotkey manager initialized on main thread");
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn is_main_thread() -> bool {
    // Use pthread API to check if we're on main thread
    unsafe {
        // pthread_main_np() returns 1 if on main thread, 0 otherwise
        libc::pthread_main_np() != 0
    }
}
```

**Add to Cargo.toml dependencies:**
```toml
[target.'cfg(target_os = "macos")'.dependencies]
libc = "0.2"
```

### Phase 2: Create Hotkey Manager on Main Thread (1 hour)

**File:** `src/main.rs` (macOS version)

**Strategy:** Detect if command needs hotkeys, create manager BEFORE entering async.

```rust
#[cfg(target_os = "macos")]
fn main() -> Result<()> {
    use hush::cli_main::Commands;
    use hush::hotkey::HotkeyManager;
    use std::sync::Arc;

    let cli = Cli::parse_args();
    let logging_result = initialize_logging(&cli);
    // ... (existing logging setup) ...

    info!("🚀 Hush application starting (macOS main thread mode)");

    // Check if command needs hotkeys
    let needs_hotkeys = matches!(
        cli.command,
        Commands::Listen { .. } | Commands::Start { .. }
    );

    if needs_hotkeys {
        info!("macOS: Creating hotkey manager on main thread (BEFORE tokio)");

        // Create hotkey manager on main thread
        let (hotkey_manager, hotkey_rx) = HotkeyManager::new("Ctrl+Alt+V")
            .map_err(|e| {
                error!("Failed to create hotkey manager: {}", e);
                error!("This usually means we're not on the main thread");
                e
            })?;

        hotkey_manager.start_listening()?;
        info!("✅ Hotkey manager created and listening");

        // Wrap in Arc for sharing across threads
        let hotkey_manager = Arc::new(hotkey_manager);
        let hotkey_rx = Arc::new(parking_lot::Mutex::new(hotkey_rx));

        // NOW create tokio runtime
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime");

        // Run async code with pre-created hotkeys
        let result = runtime.block_on(async {
            let dispatcher = CommandDispatcher::new(
                cli.config_file.clone(),
                !cli.no_notifications
            );

            // Pass pre-created hotkeys to dispatcher
            dispatcher.dispatch_with_hotkeys(
                cli.command,
                Some((hotkey_manager, hotkey_rx))
            ).await
        });

        // Handle result...
        result
    } else {
        // Commands without hotkeys use normal flow
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime");

        let result = runtime.block_on(async {
            let dispatcher = CommandDispatcher::new(
                cli.config_file.clone(),
                !cli.no_notifications
            );
            dispatcher.dispatch(cli.command).await
        });

        result
    }
}
```

### Phase 3: Add Hotkey Passthrough to Dispatcher (1 hour)

**File:** `src/cli/dispatcher.rs`

Add new method that accepts pre-created hotkeys:

```rust
use std::sync::Arc;
use std::sync::mpsc;
use parking_lot::Mutex as ParkingMutex;

pub type PreCreatedHotkeys = (
    Arc<HotkeyManager>,
    Arc<ParkingMutex<mpsc::Receiver<HotkeyEvent>>>
);

impl CommandDispatcher {
    /// Dispatch command with pre-created hotkey manager (macOS main thread requirement)
    pub async fn dispatch_with_hotkeys(
        &self,
        command: Commands,
        hotkeys: Option<PreCreatedHotkeys>,
    ) -> Result<()> {
        match command {
            Commands::Listen {
                editing_mode,
                no_processing,
                no_button,
            } => {
                // Pass hotkeys to listen command
                commands::listen::handle_listen_with_hotkeys(
                    editing_mode,
                    no_processing,
                    no_button,
                    hotkeys,
                ).await
            },
            Commands::Start { .. } => {
                // Similar handling for start command
                unimplemented!("Start command with pre-created hotkeys")
            },
            _ => {
                // Other commands don't need hotkeys, use normal dispatch
                self.dispatch(command).await
            }
        }
    }
}
```

### Phase 4: Modify handle_listen to Accept Hotkeys (2 hours)

**File:** `src/cli/commands/listen.rs`

**Current signature:**
```rust
pub async fn handle_listen(
    editing_mode_str: String,
    no_processing: bool,
    no_button: bool,
) -> Result<()>
```

**New signature:**
```rust
use std::sync::mpsc;
use std::sync::Arc;
use parking_lot::Mutex as ParkingMutex;

pub type PreCreatedHotkeys = (
    Arc<HotkeyManager>,
    Arc<ParkingMutex<mpsc::Receiver<HotkeyEvent>>>
);

pub async fn handle_listen_with_hotkeys(
    editing_mode_str: String,
    no_processing: bool,
    no_button: bool,
    pre_created_hotkeys: Option<PreCreatedHotkeys>,
) -> Result<()> {
    info!("🎧 Starting intelligent listening mode");
    // ... existing setup ...

    // Use pre-created hotkeys if provided (macOS), otherwise create new (Linux)
    let (hotkey_manager, hotkey_rx) = if let Some((manager, rx)) = pre_created_hotkeys {
        info!("Using pre-created hotkey manager (macOS main thread)");

        // Extract from Arc/Mutex
        let manager = manager;  // Already Arc
        let rx = {
            let mut locked_rx = rx.lock();
            // Move receiver out - need to restructure this
            // Actually, we need to keep it in Arc<Mutex> for sharing
        };

        (manager, rx)
    } else {
        info!("Creating new hotkey manager (Linux)");

        // Current Linux path
        let (manager, rx) = HotkeyManager::new("Ctrl+Alt+V")?;
        manager.start_listening()?;
        info!("✅ Hotkey 'Ctrl+Alt+V' registered");

        (Arc::new(manager), Arc::new(ParkingMutex::new(rx)))
    };

    // Rest of function uses hotkey_manager and hotkey_rx as before
    // ... existing code ...
}

// Keep old function for backward compatibility
pub async fn handle_listen(
    editing_mode_str: String,
    no_processing: bool,
    no_button: bool,
) -> Result<()> {
    handle_listen_with_hotkeys(editing_mode_str, no_processing, no_button, None).await
}
```

**Key Changes in handle_listen:**
1. Accept `Option<PreCreatedHotkeys>` parameter
2. If `Some`, use pre-created manager (macOS path)
3. If `None`, create new manager (Linux path)
4. Adjust code to work with `Arc<HotkeyManager>` instead of owned
5. Handle `Arc<Mutex<Receiver>>` for hotkey events

### Phase 5: Handle Hotkey Event Loop (1 hour)

**Challenge:** The hotkey event loop needs to receive events from the pre-created receiver.

**File:** `src/cli/commands/listen.rs` (continued)

```rust
// In the hotkey thread section (around line 230):

let hotkey_thread = thread::spawn(move || {
    info!("Hotkey listener thread started");

    // Get receiver - either from Arc<Mutex> or local
    let hotkey_rx_clone = hotkey_rx.clone();

    loop {
        // Receive from mutex-wrapped receiver
        let event = {
            let mut rx = hotkey_rx_clone.lock();
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(event) => Some(event),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    warn!("Hotkey receiver disconnected");
                    break;
                },
            }
        };

        if let Some(event) = event {
            match event {
                HotkeyEvent::Pressed => {
                    info!("Hotkey pressed");
                    if let Err(e) = audio_cmd_tx.send(AudioCommand::StartRecording) {
                        error!("Failed to send start recording command: {}", e);
                        break;
                    }
                    // ... rest of handling ...
                },
                HotkeyEvent::Released => {
                    // ... existing code ...
                },
            }
        }

        // Check for shutdown, etc.
    }

    info!("Hotkey listener thread terminated");
});
```

---

## Success Criteria

- [ ] `ensure_main_thread()` actually checks and errors if not on main thread
- [ ] HotkeyManager created on main thread before tokio runtime (macOS)
- [ ] `./target/debug/hush listen` starts without hanging
- [ ] Overlay appears after starting listen mode
- [ ] Pressing Ctrl+Alt+V triggers recording
- [ ] Releasing Ctrl+Alt+V stops recording and transcribes
- [ ] Linux behavior unchanged (hotkeys still work)
- [ ] No compilation errors
- [ ] All existing tests pass

---

## Verification Steps

### On macOS:

```bash
# 1. Clean build
cargo clean
cargo build --release

# 2. Run listen mode
./target/release/hush listen

# Expected:
# - Overlay window appears immediately
# - Console shows: "✅ Hotkey manager created on main thread"
# - No hanging

# 3. Test hotkey
# Press Ctrl+Alt+V
# Expected: Recording starts, overlay shows "Recording..."

# Release Ctrl+Alt+V
# Expected: Recording stops, transcription happens, text inserted

# 4. Test error detection
# Run this test to verify main thread check works:
cargo test test_non_main_thread_fails --lib

# Expected: Test passes, showing background thread is detected
```

### On Linux:

```bash
# Verify no regression
cargo build && cargo test
./target/debug/hush listen

# Expected: Works exactly as before (no changes to Linux path)
```

---

## Alternative Approaches Considered

### Alt 1: Use Different Hotkey Library
**Pros:** Might not have main thread requirement
**Cons:** Need to find and integrate new library, unknown quality
**Decision:** Rejected - `global-hotkey` is solid, just needs proper threading

### Alt 2: Disable Hotkeys on macOS
**Pros:** Quick workaround, unblocks other work
**Cons:** Major feature loss, defeats purpose of app
**Decision:** Rejected - hotkeys are core functionality

### Alt 3: Use rdev Crate
**Pros:** Different threading model, might work
**Cons:** Less reliable, different API, more platform quirks
**Decision:** Rejected - adds complexity without solving core issue

### Alt 4: Spawn Tokio on Background Thread
**Pros:** Keep main thread for hotkeys
**Cons:** Complex, non-standard, might break other things
**Decision:** Rejected - current approach (create hotkeys first) is cleaner

---

## Implementation Challenges

### Challenge 1: Moving Receiver Out of Arc<Mutex>
**Problem:** `mpsc::Receiver` is not `Clone`, so can't share easily.
**Solution:** Keep it in `Arc<Mutex>` and lock when receiving events. Accept small locking overhead.

### Challenge 2: Lifetimes and Ownership
**Problem:** HotkeyManager needs to live for entire program, but created in main().
**Solution:** Wrap in `Arc` to share ownership across async boundaries.

### Challenge 3: Backward Compatibility
**Problem:** Linux path shouldn't change.
**Solution:** Make `pre_created_hotkeys` parameter `Option`. `None` = create new (Linux), `Some` = use provided (macOS).

### Challenge 4: Testing
**Problem:** Hard to test main thread requirement in CI.
**Solution:** Add unit tests that spawn threads and verify detection. Add manual testing checklist.

---

## Files to Modify

1. **`src/main.rs`** (60 lines changed)
   - Add hotkey detection logic
   - Create HotkeyManager before tokio (macOS only)
   - Call `dispatch_with_hotkeys()` instead of `dispatch()`

2. **`src/cli/dispatcher.rs`** (30 lines added)
   - Add `PreCreatedHotkeys` type alias
   - Add `dispatch_with_hotkeys()` method
   - Route to appropriate command handler

3. **`src/cli/commands/listen.rs`** (50 lines changed)
   - Add `handle_listen_with_hotkeys()` function
   - Accept `Option<PreCreatedHotkeys>` parameter
   - Handle Arc-wrapped hotkey manager
   - Keep backward-compatible `handle_listen()`

4. **`src/adapters/hotkey/macos.rs`** (20 lines changed)
   - Implement real `ensure_main_thread()` check
   - Use `libc::pthread_main_np()` for detection
   - Return proper error with helpful message

5. **`Cargo.toml`** (2 lines added)
   - Add `libc` dependency for macOS target

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn test_main_thread_detection() {
        // This test runs on main thread
        assert!(is_main_thread());
        assert!(ensure_main_thread().is_ok());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_background_thread_detection() {
        let handle = std::thread::spawn(|| {
            (is_main_thread(), ensure_main_thread())
        });

        let (is_main, result) = handle.join().unwrap();
        assert!(!is_main);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("main thread"));
    }
}
```

### Integration Test

```rust
// tests/macos_hotkey_main_thread.rs

#[cfg(target_os = "macos")]
#[test]
fn test_hotkey_manager_on_main_thread() {
    // This should succeed (test runs on main thread)
    let result = HotkeyManager::new("Ctrl+Alt+V");
    assert!(result.is_ok());
}

#[cfg(target_os = "macos")]
#[test]
fn test_hotkey_manager_in_tokio_fails() {
    // This should fail (tokio worker thread)
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let result = runtime.block_on(async {
        HotkeyManager::new("Ctrl+Alt+V")
    });

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("main thread"));
}
```

### Manual Testing Checklist

- [ ] Build succeeds on macOS
- [ ] Build succeeds on Linux
- [ ] `hush listen` starts without hanging (macOS)
- [ ] Overlay window appears (macOS)
- [ ] Hotkey triggers recording (macOS)
- [ ] Recording → transcription → insertion works (macOS)
- [ ] Linux hotkeys still work (no regression)
- [ ] Error message is helpful if thread detection fails

---

## Dependencies

**Blocks:**
- macOS users can't use `hush listen` command
- Overlay functionality untestable on macOS
- Voice-to-text workflow blocked

**Blocked by:**
- None (can be implemented immediately)

**Related:**
- T-038 (was marked complete but incomplete)
- T-039 (macOS integration tests - some may be failing due to this)

---

## Estimated Time Breakdown

- Phase 1: Enable main thread detection - **30 minutes**
- Phase 2: Create hotkeys on main thread - **1 hour**
- Phase 3: Add dispatcher passthrough - **1 hour**
- Phase 4: Modify handle_listen - **2 hours**
- Phase 5: Handle event loop - **1 hour**
- Testing and debugging - **1 hour**
- **Total: 6.5 hours**

---

## References

- **T-038**: Original task with detailed strategies (partially implemented)
- **docs/macos/THREADING.md**: Threading requirements documentation
- **global-hotkey docs**: https://docs.rs/global-hotkey/latest/global_hotkey/
- **pthread_main_np**: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/pthread_main_np.3.html
- **AppKit threading**: https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/ThreadSafetySummary/ThreadSafetySummary.html

---

## Notes

### Why This Wasn't Caught Earlier

1. T-038 was marked "complete" but `ensure_main_thread()` was left disabled
2. No integration testing of `hush listen` on macOS after T-038
3. The warning message was misleading ("relying on global_hotkey")
4. CI may not be running actual hotkey tests (requires GUI environment)

### Debug Tips

If still hangs after implementation:

```bash
# Enable all logging
RUST_LOG=trace ./target/debug/hush listen 2>&1 | tee hush.log

# Check for these messages:
# "✅ Hotkey manager created on main thread" - good!
# "macOS detected: Checking main thread requirement" - should see this
# "Main thread check disabled" - BAD, means ensure_main_thread() still broken

# Check thread IDs in logs
grep -i "thread" hush.log

# Use lldb to debug hang
lldb -- ./target/debug/hush listen
(lldb) run
# Wait for hang
(lldb) thread backtrace all
# Look for GlobalHotKeyManager::new in stack
```

---

## Post-Implementation TODO

- [ ] Update `docs/macos/THREADING.md` with actual solution
- [ ] Add to `docs/macos/TROUBLESHOOTING.md` if issues persist
- [ ] Update T-038 status to "Reopened - Completed Properly"
- [ ] Create follow-up task for `Start` command (similar fix needed)
- [ ] Add to CI: Manual testing checklist for macOS hotkeys
- [ ] Consider upstreaming fix to `global-hotkey` crate (better error messages)
