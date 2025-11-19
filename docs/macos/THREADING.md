# macOS Threading Requirements for Hotkeys

## Overview

On macOS, the `global-hotkey` library requires that `GlobalHotKeyManager` be created and used on the **main thread**. This is a platform requirement due to AppKit/Cocoa event handling architecture.

## The Problem

Using `#[tokio::main]` attribute can cause issues:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // This may NOT be on the main thread!
    let hotkey = HotkeyTriggerAdapter::new("Cmd+Alt+V")?; // ❌ May fail on macOS
    // ...
}
```

The `#[tokio::main]` macro creates a tokio runtime that may execute the async main function on a worker thread, not the actual OS main thread.

## The Solution

### Option 1: Create HotkeyManager Before Tokio Runtime (Recommended)

```rust
fn main() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        // Create hotkey manager on main thread (macOS requirement)
        let hotkey = HotkeyTriggerAdapter::new("Cmd+Alt+V")?;

        // Then run tokio runtime
        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(async {
            run_app_with_hotkey(hotkey).await
        })
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Linux can use tokio::main as normal
        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(async {
            let hotkey = HotkeyTriggerAdapter::new("Ctrl+Alt+V")?;
            run_app_with_hotkey(hotkey).await
        })
    }
}
```

### Option 2: Use Mock Components for Testing

When testing or when hotkeys aren't critical:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let hotkey = if cfg!(target_os = "macos") {
        // Use mock on macOS to avoid threading issues during development
        Box::new(MockInputTrigger::new()) as Box<dyn InputTrigger>
    } else {
        Box::new(HotkeyTriggerAdapter::new("Ctrl+Alt+V")?)
    };

    run_app(hotkey).await
}
```

### Option 3: Pin Main Thread to Event Loop (Advanced)

For full control, keep the OS main thread for event handling:

```rust
fn main() -> Result<()> {
    // Create hotkey manager on main thread
    let hotkey_manager = GlobalHotKeyManager::new()?;
    let hotkey = hotkey_manager.register(/* ... */)?;

    // Spawn tokio runtime on background thread
    std::thread::spawn(|| {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            run_app().await
        })
    });

    // Keep main thread alive for event loop
    loop {
        if let Ok(event) = global_hotkey::GlobalHotKeyEvent::receiver().recv() {
            // Handle hotkey events
        }
    }
}
```

## Error Messages

If you see this error:

```
Hotkey initialization must happen on main thread on macOS.

This is a platform requirement due to AppKit/Cocoa event handling.

Possible solutions:
1. Create HotkeyManager before tokio runtime starts
2. Use a different threading model
3. Consider using an alternative hotkey library
```

This means your code is trying to create the hotkey manager from within the tokio runtime. Use one of the solutions above.

## Platform Differences

| Platform | Main Thread Required | Notes |
|----------|---------------------|-------|
| **macOS** | ✅ Yes | AppKit/Cocoa requirement |
| **Linux** | ❌ No | Can create from any thread |
| **Windows** | ❓ Unknown | Not yet tested |

## Testing

The hotkey module includes tests that verify main thread detection:

```bash
# On macOS:
cargo test --lib hotkey::macos::tests

# Expected: Tests should pass, demonstrating:
# - Main thread detection works
# - Non-main thread creation fails with error
```

## Best Practices

1. **Always test on actual macOS hardware** - Behavior may differ from Linux
2. **Check thread in logs** - Use `hotkey::macos::log_thread_info()` for debugging
3. **Handle errors gracefully** - Provide fallback when hotkeys can't be registered
4. **Document the requirement** - Make it clear in your app's setup guide

## References

- [AppKit Thread Safety](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/ThreadSafetySummary/ThreadSafetySummary.html)
- [NSThread Documentation](https://developer.apple.com/documentation/foundation/nsthread)
- [global-hotkey crate](https://docs.rs/global-hotkey/latest/global_hotkey/)

## Future Improvements

- Consider creating a cross-platform abstraction that handles threading automatically
- Explore alternative hotkey libraries with different threading models
- Add runtime thread migration (if possible with AppKit)
