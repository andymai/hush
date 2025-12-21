/// Global hotkey management for system-wide keyboard shortcuts
///
/// This module provides global hotkey registration, enabling Hush to respond
/// to keyboard shortcuts even when not in focus.
///
/// # Components
///
/// - [`HotkeyManager`] - Registers and manages global keyboard shortcuts
/// - [`HotkeyEvent`] - Events triggered by hotkey activation
///
/// # Default Hotkeys
///
/// - **Ctrl+Shift+Space** - Push-to-talk recording in listen mode
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
/// // Register Ctrl+Shift+Space
/// manager.register("Ctrl+Shift+Space").expect("Failed to register hotkey");
///
/// // Handle events
/// while let Some(event) = manager.poll_event() {
///     match event {
///         HotkeyEvent::Pressed => println!("Recording started"),
///         HotkeyEvent::Released => println!("Recording stopped"),
///     }
/// }
/// ```
pub mod global;

pub use global::{HotkeyEvent, HotkeyManager};
