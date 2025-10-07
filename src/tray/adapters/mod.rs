/// Adapter implementations for system tray traits
/// 
/// Following the Adapter pattern from DESIGN_PATTERNS.md, these modules
/// contain concrete implementations of the system tray traits.

pub mod ksni_adapter;
pub mod json_history;
pub mod libnotify_adapter;

pub use ksni_adapter::KsniSystemTray;
pub use json_history::JsonHistoryStore;
pub use libnotify_adapter::LibnotifyProvider;