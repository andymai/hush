/// System tray integration module
/// 
/// Provides trait-based system tray functionality following DESIGN_PATTERNS.md
/// architecture with full dependency injection support and 100% testability.

pub mod adapters;
pub mod mocks;

// Re-export core traits from the main traits module
pub use crate::core::traits::{
    SystemTray, HistoryStore, NotificationProvider,
    TrayIconState, TrayEvent, TrayMenu, TrayMenuItem,
    TranscriptionEntry, MenuTranscriptionItem, NotificationLevel
};

// Re-export adapter implementations
#[cfg(all(feature = "system-tray", target_os = "linux"))]
pub use adapters::ksni_adapter::KsniSystemTray;

#[cfg(all(feature = "system-tray", target_os = "macos"))]
pub use adapters::macos_tray_adapter::MacOSTrayAdapter;

pub use adapters::json_history::JsonHistoryStore;

#[cfg(feature = "notifications")]
pub use adapters::libnotify_adapter::LibnotifyProvider;

// Re-export factory function
#[cfg(feature = "system-tray")]
pub use adapters::create_system_tray;

// Re-export mocks for testing
pub use mocks::{
    MockSystemTray, MockHistoryStore, MockNotificationProvider
};