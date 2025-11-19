/// Global hotkey management for system-wide keyboard shortcuts
///
/// This module provides cross-platform global hotkey registration, enabling
/// Hush to respond to keyboard shortcuts even when not in focus.
///
/// # Components
///
/// - [`HotkeyManager`] - Registers and manages global keyboard shortcuts
/// - [`HotkeyEvent`] - Events triggered by hotkey activation
///
/// # Default Hotkeys
///
/// - **Ctrl+Alt+V** - Push-to-talk recording in listen mode
/// - **F11** - Alternative recording hotkey (configurable)
///
/// # Examples
///
/// ```no_run
/// use hush::hotkey::{HotkeyManager, HotkeyEvent};
///
/// // Create hotkey manager
/// let manager = HotkeyManager::new().expect("Failed to create manager");
///
/// // Register Ctrl+Alt+V
/// manager.register("Ctrl+Alt+V").expect("Failed to register hotkey");
///
/// // Handle events
/// while let Some(event) = manager.poll_event() {
///     match event {
///         HotkeyEvent::Pressed => println!("Recording started"),
///         HotkeyEvent::Released => println!("Recording stopped"),
///     }
/// }
/// ```
///
/// # Platform Support
///
/// - **Linux**: X11 and Wayland (via global-hotkey crate)
/// - **macOS**: Requires main thread initialization (AppKit requirement)
///   - HotkeyManager must be created on the main thread
///   - Will fail with helpful error if created on background thread
/// - Requires proper permissions for global keyboard access
///
/// # macOS Threading Note
///
/// On macOS, the GlobalHotKeyManager must be created on the main thread due to
/// AppKit/Cocoa event handling requirements. If using `#[tokio::main]`, you may
/// need to create the HotkeyManager before the tokio runtime starts.
pub mod global;

pub use global::{HotkeyEvent, HotkeyManager};
