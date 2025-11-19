/// Adapter implementations for system tray traits
///
/// Following the Adapter pattern from DESIGN_PATTERNS.md, these modules
/// contain concrete implementations of the system tray traits.

#[cfg(feature = "system-tray")]
pub mod ksni_adapter;
pub mod json_history;
#[cfg(feature = "notifications")]
pub mod libnotify_adapter;

#[cfg(feature = "system-tray")]
pub use ksni_adapter::KsniSystemTray;
pub use json_history::JsonHistoryStore;
#[cfg(feature = "notifications")]
pub use libnotify_adapter::LibnotifyProvider;