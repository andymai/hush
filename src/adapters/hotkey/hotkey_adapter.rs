/// Adapter for HotkeyManager to implement InputTrigger trait

use crate::hotkey::{HotkeyManager, HotkeyEvent};
use crate::core::traits::{InputTrigger, TriggerEvent};
use crate::Result;
use async_trait::async_trait;
use parking_lot::Mutex;
use std::sync::{mpsc, Arc};

/// Adapter that wraps HotkeyManager to implement InputTrigger trait
/// Uses Arc<Mutex<>> for thread safety since mpsc::Receiver is not Sync
pub struct HotkeyTriggerAdapter {
    manager: HotkeyManager,
    receiver: Arc<Mutex<mpsc::Receiver<HotkeyEvent>>>,
    combination: String,
}

impl HotkeyTriggerAdapter {
    /// Create new adapter wrapping a HotkeyManager
    pub fn new(combination: &str) -> Result<Self> {
        let (manager, receiver) = HotkeyManager::new(combination)?;
        Ok(Self {
            manager,
            receiver: Arc::new(Mutex::new(receiver)),
            combination: combination.to_string(),
        })
    }

    /// Get reference to inner HotkeyManager (for migration period)
    pub fn inner(&self) -> &HotkeyManager {
        &self.manager
    }
}

#[async_trait]
impl InputTrigger for HotkeyTriggerAdapter {
    async fn start_listening(&mut self) -> Result<()> {
        self.manager.start_listening()
    }

    async fn stop_listening(&mut self) -> Result<()> {
        self.manager.stop_listening()
    }

    async fn next_event(&mut self) -> Option<TriggerEvent> {
        // Convert HotkeyEvent to TriggerEvent
        // Note: This is a blocking receive, which isn't ideal for async
        // In a future version, we should make HotkeyManager use async channels
        
        // Don't hold the lock across await points
        let recv_result = self.receiver.lock().try_recv();
        match recv_result {
            Ok(HotkeyEvent::Pressed) => Some(TriggerEvent::StartRecording),
            Ok(HotkeyEvent::Released) => Some(TriggerEvent::StopRecording),
            Err(mpsc::TryRecvError::Empty) => {
                // No event ready, yield to allow other tasks to run
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                None
            }
            Err(mpsc::TryRecvError::Disconnected) => None,
        }
    }

    fn description(&self) -> String {
        self.combination.clone()
    }
}

impl Drop for HotkeyTriggerAdapter {
    fn drop(&mut self) {
        // HotkeyManager will clean up in its own Drop
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::InputTrigger;

    #[test]
    fn test_hotkey_adapter_creation() {
        // This might fail if hotkey registration fails (permissions, etc.)
        if let Ok(adapter) = HotkeyTriggerAdapter::new("Ctrl+Shift+Space") {
            assert_eq!(adapter.description(), "Ctrl+Shift+Space");
        }
    }

    #[test]
    fn test_adapter_implements_trait() {
        // Verify the adapter compiles as trait object
        let _can_box: Result<Box<dyn InputTrigger>> = HotkeyTriggerAdapter::new("F10")
            .map(|adapter| Box::new(adapter) as Box<dyn InputTrigger>);
    }
}
