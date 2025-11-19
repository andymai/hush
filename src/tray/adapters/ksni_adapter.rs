/// KSNI system tray adapter implementation
/// 
/// Implements SystemTray trait using the ksni crate for cross-platform
/// system tray support (StatusNotifierItem/AppIndicator).

use crate::Result;
use crate::core::traits::{
    SystemTray, TrayIconState, TrayEvent, TrayMenu, TrayMenuItem, MenuTranscriptionItem
};
use async_trait::async_trait;
use ksni::{Tray, TrayService, Handle, MenuItem};
use tokio::sync::mpsc;
use std::sync::{Arc, Mutex};
use tracing::warn;

/// Embedded icon data - using microphone icon as base64 SVG
const ICON_IDLE: &[u8] = include_bytes!("../../../assets/icons/microphone-idle.svg");
const ICON_RECORDING: &[u8] = include_bytes!("../../../assets/icons/microphone-recording.svg");
const ICON_PROCESSING: &[u8] = include_bytes!("../../../assets/icons/microphone-processing.svg");
const ICON_ERROR: &[u8] = include_bytes!("../../../assets/icons/microphone-error.svg");

/// KSNI-based system tray implementation
pub struct KsniSystemTray {
    handle: Option<Handle<HushTrayIcon>>,
    event_sender: mpsc::Sender<TrayEvent>,
    event_receiver: mpsc::Receiver<TrayEvent>,
    current_state: TrayIconState,
    tooltip: String,
}

impl KsniSystemTray {
    /// Create new KSNI system tray
    pub fn new() -> Result<Self> {
        // Bounded channel with capacity for UI events (100 events buffer)
        const TRAY_EVENT_CAPACITY: usize = 100;
        let (event_sender, event_receiver) = mpsc::channel(TRAY_EVENT_CAPACITY);
        
        Ok(Self {
            handle: None,
            event_sender,
            event_receiver,
            current_state: TrayIconState::Idle,
            tooltip: "Hush Voice-to-Text".to_string(),
        })
    }
    
    /// Initialize the tray service (separate from construction for error handling)
    pub async fn initialize(&mut self) -> Result<()> {
        let tray_icon = HushTrayIcon::new(self.event_sender.clone());
        let service = TrayService::new(tray_icon);
        let handle = service.handle();
        
        // Store handle for later use
        self.handle = Some(handle);
        
        // Start the service (this spawns background task)
        service.spawn().await;
        
        Ok(())
    }
    
    /// Get icon data for current state
    fn get_icon_data(&self) -> &'static [u8] {
        match self.current_state {
            TrayIconState::Idle => ICON_IDLE,
            TrayIconState::Recording => ICON_RECORDING,
            TrayIconState::Processing => ICON_PROCESSING,
            TrayIconState::Error => ICON_ERROR,
        }
    }
    
    /// Update tray icon and menu through handle
    fn update_tray(&self, menu: Option<TrayMenu>) -> Result<()> {
        if let Some(ref handle) = self.handle {
            // Update icon
            handle.set_icon_name(&format!("hush-{:?}", self.current_state).to_lowercase());
            
            // Update tooltip
            handle.set_title(&self.tooltip);
            
            // Update menu if provided
            if let Some(menu) = menu {
                let menu_items = self.build_menu_items(menu);
                handle.set_menu(menu_items);
            }
        }
        Ok(())
    }
    
    /// Build KSNI menu items from TrayMenu
    fn build_menu_items(&self, menu: TrayMenu) -> Vec<MenuItem> {
        let mut items = Vec::new();
        
        // Add recent transcriptions
        if !menu.recent_transcriptions.is_empty() {
            items.push(MenuItem::Label("Recent Transcriptions".to_string()));
            items.push(MenuItem::Separator);
            
            for (i, item) in menu.recent_transcriptions.iter().enumerate() {
                if i >= 3 { break; } // Limit to 3 items
                
                let preview = if item.preview.len() > 40 {
                    format!("{}...", &item.preview[..37])
                } else {
                    item.preview.clone()
                };
                
                items.push(MenuItem::Action {
                    label: preview,
                    activate: {
                        let sender = self.event_sender.clone();
                        let id = item.id.clone();
                        Box::new(move || {
                            if let Err(e) = sender.try_send(TrayEvent::HistoryItemSelected(id.clone())) {
                                warn!("Failed to send tray event: {:?}", e);
                            }
                        })
                    }
                });
            }
            items.push(MenuItem::Separator);
        }
        
        // Recording toggle
        let recording_label = if menu.recording_state {
            "🛑 Stop Recording"
        } else {
            "🎙️ Start Recording"
        };
        
        items.push(MenuItem::Action {
            label: recording_label.to_string(),
            activate: {
                let sender = self.event_sender.clone();
                let item = if menu.recording_state {
                    TrayMenuItem::StopRecording
                } else {
                    TrayMenuItem::StartRecording
                };
                Box::new(move || {
                    if let Err(e) = sender.try_send(TrayEvent::MenuClicked(item)) {
                        warn!("Failed to send tray event: {:?}", e);
                    }
                })
            }
        });
        
        items.push(MenuItem::Separator);
        
        // Settings
        items.push(MenuItem::Action {
            label: "⚙️ Settings".to_string(),
            activate: {
                let sender = self.event_sender.clone();
                Box::new(move || {
                    if let Err(e) = sender.try_send(TrayEvent::SettingsRequested) {
                        warn!("Failed to send tray event: {:?}", e);
                    }
                })
            }
        });

        // Clear history
        items.push(MenuItem::Action {
            label: "🗑️ Clear History".to_string(),
            activate: {
                let sender = self.event_sender.clone();
                Box::new(move || {
                    if let Err(e) = sender.try_send(TrayEvent::MenuClicked(TrayMenuItem::ClearHistory)) {
                        warn!("Failed to send tray event: {:?}", e);
                    }
                })
            }
        });

        items.push(MenuItem::Separator);

        // About
        items.push(MenuItem::Action {
            label: "ℹ️ About Hush".to_string(),
            activate: {
                let sender = self.event_sender.clone();
                Box::new(move || {
                    if let Err(e) = sender.try_send(TrayEvent::MenuClicked(TrayMenuItem::About)) {
                        warn!("Failed to send tray event: {:?}", e);
                    }
                })
            }
        });

        // Quit
        items.push(MenuItem::Action {
            label: "❌ Quit".to_string(),
            activate: {
                let sender = self.event_sender.clone();
                Box::new(move || {
                    if let Err(e) = sender.try_send(TrayEvent::QuitRequested) {
                        warn!("Failed to send tray event: {:?}", e);
                    }
                })
            }
        });
        
        items
    }
}

#[async_trait]
impl SystemTray for KsniSystemTray {
    async fn show(&mut self) -> Result<()> {
        if self.handle.is_none() {
            self.initialize().await?;
        }
        Ok(())
    }
    
    async fn hide(&mut self) -> Result<()> {
        // KSNI doesn't support hiding, but we can stop the service
        // For now, just do nothing since hiding isn't critical
        Ok(())
    }
    
    fn set_icon(&mut self, state: TrayIconState) {
        self.current_state = state;
        let _ = self.update_tray(None);
    }
    
    fn set_tooltip(&mut self, text: &str) {
        self.tooltip = text.to_string();
        let _ = self.update_tray(None);
    }
    
    fn update_menu(&mut self, menu: TrayMenu) {
        let _ = self.update_tray(Some(menu));
    }
    
    fn event_receiver(&self) -> &mpsc::Receiver<TrayEvent> {
        &self.event_receiver
    }
}

/// KSNI Tray implementation
struct HushTrayIcon {
    event_sender: mpsc::Sender<TrayEvent>,
}

impl HushTrayIcon {
    fn new(event_sender: mpsc::Sender<TrayEvent>) -> Self {
        Self { event_sender }
    }
}

impl Tray for HushTrayIcon {
    fn id(&self) -> String {
        "hush-voice-to-text".to_string()
    }
    
    fn title(&self) -> String {
        "Hush Voice-to-Text".to_string()
    }
    
    fn tooltip(&self) -> String {
        "Hush Voice-to-Text - Press hotkey to record".to_string()
    }
    
    fn icon_name(&self) -> String {
        "microphone-sensitivity-high".to_string()
    }
    
    fn menu(&self) -> Vec<MenuItem> {
        // Default menu - will be updated through handle
        vec![
            MenuItem::Action {
                label: "🎙️ Start Recording".to_string(),
                activate: {
                    let sender = self.event_sender.clone();
                    Box::new(move || {
                        if let Err(e) = sender.try_send(TrayEvent::MenuClicked(TrayMenuItem::StartRecording)) {
                            warn!("Failed to send tray event: {:?}", e);
                        }
                    })
                }
            },
            MenuItem::Separator,
            MenuItem::Action {
                label: "❌ Quit".to_string(),
                activate: {
                    let sender = self.event_sender.clone();
                    Box::new(move || {
                        if let Err(e) = sender.try_send(TrayEvent::QuitRequested) {
                            warn!("Failed to send tray event: {:?}", e);
                        }
                    })
                }
            }
        ]
    }
}