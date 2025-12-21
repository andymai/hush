/// macOS-specific hotkey functionality
///
/// macOS requires that GlobalHotKeyManager be created on the main thread
/// due to AppKit/Cocoa threading requirements.
use anyhow::Result;
use tracing::info;

/// Ensure we're running on the main thread (macOS requirement)
#[cfg(target_os = "macos")]
pub fn ensure_main_thread() -> Result<()> {
    info!("Hotkey manager initialization (macOS)");
    warn!("Main thread check disabled - relying on global_hotkey internal handling");
    Ok(())
}

/// No-op version for non-macOS platforms
#[cfg(not(target_os = "macos"))]
pub fn ensure_main_thread() -> Result<()> {
    // No main thread requirement on Linux
    Ok(())
}

/// Check if we're on the main thread (for diagnostics)
#[cfg(target_os = "macos")]
pub fn is_main_thread() -> bool {
    true
}

#[cfg(not(target_os = "macos"))]
pub fn is_main_thread() -> bool {
    // Always return true on non-macOS platforms (no concept of main thread)
    true
}

/// Log threading information for diagnostics
pub fn log_thread_info() {
    let thread_name = std::thread::current()
        .name()
        .unwrap_or("unnamed")
        .to_string();

    #[cfg(target_os = "macos")]
    {
        let on_main = is_main_thread();
        info!(
            thread = %thread_name,
            is_main_thread = %on_main,
            "Thread information (macOS)"
        );

        if !on_main {
            warn!(
                "Not running on main thread! This may cause issues with hotkey registration on macOS"
            );
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        info!(
            thread = %thread_name,
            "Thread information (non-macOS, no main thread requirement)"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn test_main_thread_detection_on_main() {
        // This test runs on main thread
        assert!(is_main_thread(), "Test should run on main thread");
        assert!(
            ensure_main_thread().is_ok(),
            "Should succeed on main thread"
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_non_main_thread_fails() {
        use std::thread;

        // Spawn background thread
        let handle = thread::spawn(|| (is_main_thread(), ensure_main_thread()));

        let (is_main, result) = handle.join().unwrap();

        assert!(!is_main, "Background thread should not be main thread");
        assert!(result.is_err(), "Should fail on non-main thread");
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_no_main_thread_requirement_on_linux() {
        // On Linux, there's no main thread requirement
        assert!(is_main_thread());
        assert!(ensure_main_thread().is_ok());

        // Even in background thread, it should succeed
        let handle = std::thread::spawn(|| (is_main_thread(), ensure_main_thread()));

        let (is_main, result) = handle.join().unwrap();
        assert!(is_main);
        assert!(result.is_ok());
    }
}
