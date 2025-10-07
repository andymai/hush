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
pub use adapters::{
    ksni_adapter::KsniSystemTray,
    json_history::JsonHistoryStore, 
    libnotify_adapter::LibnotifyProvider,
};

// Re-export mocks for testing
pub use mocks::{
    MockSystemTray, MockHistoryStore, MockNotificationProvider
};