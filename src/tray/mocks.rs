/// Mock implementations for system tray traits
/// 
/// Provides fully testable mock implementations following the design pattern
/// requirement of 100% mockability.

use crate::Result;
use crate::core::traits::{
    SystemTray, HistoryStore, NotificationProvider, 
    TrayIconState, TrayEvent, TrayMenu, TranscriptionEntry,
    TranscriptionResult, NotificationLevel
};
use async_trait::async_trait;
use tokio::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

// ============================================================================
// Mock History Operations for Testing
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryOperation {
    Add(String), // entry id
    GetRecent(usize), // limit
    Clear,
}

// ============================================================================
// Mock System Tray
// ============================================================================

/// Mock system tray for testing
pub struct MockSystemTray {
    pub events: Vec<TrayEvent>,
    pub icon_state: TrayIconState,
    pub tooltip: String,
    pub menu: Option<TrayMenu>,
    pub is_visible: bool,
    pub event_sender: mpsc::Sender<TrayEvent>,
    pub event_receiver: mpsc::Receiver<TrayEvent>,
}

impl MockSystemTray {
    pub fn new() -> Self {
        // Bounded channel with capacity for UI events (100 events buffer)
        const TRAY_EVENT_CAPACITY: usize = 100;
        let (event_sender, event_receiver) = mpsc::channel(TRAY_EVENT_CAPACITY);

        Self {
            events: Vec::new(),
            icon_state: TrayIconState::Idle,
            tooltip: "Mock Tray".to_string(),
            menu: None,
            is_visible: false,
            event_sender,
            event_receiver,
        }
    }

    /// Simulate clicking a menu item (for testing)
    pub fn simulate_menu_click(&self, item: crate::core::traits::TrayMenuItem) {
        let _ = self.event_sender.try_send(TrayEvent::MenuClicked(item));
    }

    /// Simulate selecting a history item (for testing)
    pub fn simulate_history_selection(&self, entry_id: String) {
        let _ = self.event_sender.try_send(TrayEvent::HistoryItemSelected(entry_id));
    }

    /// Simulate quit request (for testing)
    pub fn simulate_quit(&self) {
        let _ = self.event_sender.try_send(TrayEvent::QuitRequested);
    }
}

#[async_trait]
impl SystemTray for MockSystemTray {
    async fn show(&mut self) -> Result<()> {
        self.is_visible = true;
        Ok(())
    }
    
    async fn hide(&mut self) -> Result<()> {
        self.is_visible = false;
        Ok(())
    }
    
    fn set_icon(&mut self, state: TrayIconState) {
        self.icon_state = state;
    }
    
    fn set_tooltip(&mut self, text: &str) {
        self.tooltip = text.to_string();
    }
    
    fn update_menu(&mut self, menu: TrayMenu) {
        self.menu = Some(menu);
    }
    
    fn event_receiver(&self) -> &mpsc::Receiver<TrayEvent> {
        &self.event_receiver
    }
}

// ============================================================================
// Mock History Store
// ============================================================================

/// Mock history store for testing
pub struct MockHistoryStore {
    pub entries: VecDeque<TranscriptionEntry>,
    pub operations: Vec<HistoryOperation>,
    pub max_entries: usize,
}

impl MockHistoryStore {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            operations: Vec::new(),
            max_entries: 100,
        }
    }
    
    pub fn with_max_entries(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            operations: Vec::new(),
            max_entries,
        }
    }
    
    /// Add sample entries for testing
    pub fn add_sample_entries(&mut self, count: usize) {
        for i in 0..count {
            let entry = TranscriptionEntry {
                id: format!("test_{}", i),
                timestamp: std::time::SystemTime::now(),
                text: format!("Sample transcription {}", i + 1),
                confidence: 0.85 + (i as f32 * 0.02),
                duration: std::time::Duration::from_secs(i as u64 + 1),
            };
            self.entries.push_back(entry);
        }
    }
    
    /// Get the last operation for testing
    pub fn last_operation(&self) -> Option<&HistoryOperation> {
        self.operations.last()
    }
}

#[async_trait]
impl HistoryStore for MockHistoryStore {
    async fn add_entry(&mut self, entry: TranscriptionEntry) -> Result<()> {
        self.operations.push(HistoryOperation::Add(entry.id.clone()));
        
        // Simulate circular buffer
        self.entries.push_back(entry);
        while self.entries.len() > self.max_entries {
            self.entries.pop_front();
        }
        
        Ok(())
    }
    
    async fn get_recent(&self, limit: usize) -> Result<Vec<TranscriptionEntry>> {
        // Note: We don't add to operations here in the mock for simplicity,
        // but we could if we wanted to track read operations
        
        Ok(self.entries
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect())
    }
    
    async fn clear(&mut self) -> Result<()> {
        self.operations.push(HistoryOperation::Clear);
        self.entries.clear();
        Ok(())
    }
    
    fn len(&self) -> usize {
        self.entries.len()
    }
}

// ============================================================================
// Mock Notification Provider
// ============================================================================

#[derive(Debug, Clone)]
pub struct MockNotification {
    pub message: String,
    pub level: NotificationLevel,
    pub is_transcription: bool,
    pub timestamp: std::time::Instant,
}

/// Mock notification provider for testing
pub struct MockNotificationProvider {
    pub notifications: Vec<MockNotification>,
    pub enabled: bool,
}

impl MockNotificationProvider {
    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
            enabled: true,
        }
    }
    
    /// Get the last notification for testing
    pub fn last_notification(&self) -> Option<&MockNotification> {
        self.notifications.last()
    }
    
    /// Get notifications by level for testing
    pub fn notifications_by_level(&self, level: NotificationLevel) -> Vec<&MockNotification> {
        self.notifications
            .iter()
            .filter(|n| n.level == level)
            .collect()
    }
    
    /// Get transcription notifications for testing
    pub fn transcription_notifications(&self) -> Vec<&MockNotification> {
        self.notifications
            .iter()
            .filter(|n| n.is_transcription)
            .collect()
    }
    
    /// Clear notifications for testing
    pub fn clear_notifications(&mut self) {
        self.notifications.clear();
    }
}

#[async_trait]
impl NotificationProvider for MockNotificationProvider {
    async fn show_transcription(&self, result: &TranscriptionResult) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        
        let notification = MockNotification {
            message: result.text.clone(),
            level: NotificationLevel::Info,
            is_transcription: true,
            timestamp: std::time::Instant::now(),
        };
        
        // Note: We can't mutate self in an async trait method that takes &self,
        // so in a real test you'd use Arc<Mutex<MockNotificationProvider>>
        // For now, this is a simplified version for demonstration
        
        Ok(())
    }
    
    async fn show_status(&self, message: &str, level: NotificationLevel) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        
        let notification = MockNotification {
            message: message.to_string(),
            level,
            is_transcription: false,
            timestamp: std::time::Instant::now(),
        };
        
        // Same note as above about mutation
        
        Ok(())
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

// ============================================================================
// Thread-Safe Mock Variants
// ============================================================================

/// Thread-safe wrapper for MockNotificationProvider
pub type ThreadSafeMockNotificationProvider = Arc<Mutex<MockNotificationProvider>>;

impl ThreadSafeMockNotificationProvider {
    pub fn new() -> Self {
        Arc::new(Mutex::new(MockNotificationProvider::new()))
    }
}

#[async_trait]
impl NotificationProvider for ThreadSafeMockNotificationProvider {
    async fn show_transcription(&self, result: &TranscriptionResult) -> Result<()> {
        let mut provider = self.lock().unwrap();
        if !provider.enabled {
            return Ok(());
        }
        
        let notification = MockNotification {
            message: result.text.clone(),
            level: NotificationLevel::Info,
            is_transcription: true,
            timestamp: std::time::Instant::now(),
        };
        
        provider.notifications.push(notification);
        Ok(())
    }
    
    async fn show_status(&self, message: &str, level: NotificationLevel) -> Result<()> {
        let mut provider = self.lock().unwrap();
        if !provider.enabled {
            return Ok(());
        }
        
        let notification = MockNotification {
            message: message.to_string(),
            level,
            is_transcription: false,
            timestamp: std::time::Instant::now(),
        };
        
        provider.notifications.push(notification);
        Ok(())
    }
    
    fn is_enabled(&self) -> bool {
        self.lock().unwrap().enabled
    }
    
    fn set_enabled(&mut self, enabled: bool) {
        self.lock().unwrap().enabled = enabled;
    }
}