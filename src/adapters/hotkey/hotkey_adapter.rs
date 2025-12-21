use crate::core::traits::{InputTrigger, TriggerEvent};
/// Adapter for HotkeyManager to implement InputTrigger trait
use crate::hotkey::{HotkeyEvent, HotkeyManager};
use crate::Result;
use async_trait::async_trait;
use std::sync::mpsc;
use tokio::sync::mpsc as tokio_mpsc;

/// Adapter that wraps HotkeyManager to implement InputTrigger trait
/// Uses tokio channels for proper async event handling without busy-waiting
pub struct HotkeyTriggerAdapter {
    manager: HotkeyManager,
    async_receiver: tokio_mpsc::UnboundedReceiver<HotkeyEvent>,
    combination: String,
}

impl HotkeyTriggerAdapter {
    /// Create new adapter wrapping a HotkeyManager
    pub fn new(combination: &str) -> Result<Self> {
        let (manager, sync_receiver) = HotkeyManager::new(combination)?;

        // Create async channel for proper async/await support
        let (async_tx, async_rx) = tokio_mpsc::unbounded_channel();

        // Bridge sync channel to async channel in background thread
        std::thread::spawn(move || {
            Self::bridge_sync_to_async(sync_receiver, async_tx);
        });

        Ok(Self {
            manager,
            async_receiver: async_rx,
            combination: combination.to_string(),
        })
    }

    /// Bridge events from sync mpsc to async tokio channel
    fn bridge_sync_to_async(
        sync_rx: mpsc::Receiver<HotkeyEvent>,
        async_tx: tokio_mpsc::UnboundedSender<HotkeyEvent>,
    ) {
        // Block on sync channel and forward to async channel
        // This thread sleeps when waiting for events (no busy-wait)
        while let Ok(event) = sync_rx.recv() {
            if async_tx.send(event).is_err() {
                // Receiver dropped, exit bridge
                break;
            }
        }
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
        // Properly async receive - no polling, no busy-wait
        match self.async_receiver.recv().await {
            Some(HotkeyEvent::Pressed) => Some(TriggerEvent::StartRecording),
            Some(HotkeyEvent::Released) => Some(TriggerEvent::StopRecording),
            None => None, // Channel closed
        }
    }

    fn description(&self) -> String {
        self.combination.clone()
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
