/// macOS system tray adapter implementation
///
/// Implements SystemTray trait using the tray-icon crate for native macOS
/// NSStatusBar integration in the menu bar.

use crate::Result;
use crate::core::traits::{
    SystemTray, TrayIconState, TrayEvent, TrayMenu, TrayMenuItem, MenuTranscriptionItem
};
use async_trait::async_trait;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
};
use tokio::sync::mpsc;
use std::sync::{Arc, Mutex};
use tracing::{debug, info, warn, error};

/// Embedded icon data - using microphone icon SVG
const ICON_IDLE: &[u8] = include_bytes!("../../../assets/icons/microphone-idle.svg");
const ICON_RECORDING: &[u8] = include_bytes!("../../../assets/icons/microphone-recording.svg");
const ICON_PROCESSING: &[u8] = include_bytes!("../../../assets/icons/microphone-processing.svg");
const ICON_ERROR: &[u8] = include_bytes!("../../../assets/icons/microphone-error.svg");

/// macOS tray adapter using tray-icon crate
pub struct MacOSTrayAdapter {
    tray_icon: Option<TrayIcon>,
    menu: Menu,
    event_sender: mpsc::Sender<TrayEvent>,
    event_receiver: mpsc::Receiver<TrayEvent>,
    current_state: TrayIconState,
    tooltip: String,
    menu_items: MenuItems,
}

/// Menu item IDs for tracking clicks
#[derive(Debug)]
struct MenuItems {
    start_recording: MenuItem,
    stop_recording: MenuItem,
    history_submenu: Option<Submenu>,
    settings: MenuItem,
    about: MenuItem,
    quit: PredefinedMenuItem,
}

impl MacOSTrayAdapter {
    /// Create new macOS system tray adapter
    pub fn new() -> Result<Self> {
        info!("Initializing macOS system tray adapter");

        // Create event channel
        const TRAY_EVENT_CAPACITY: usize = 100;
        let (event_sender, event_receiver) = mpsc::channel(TRAY_EVENT_CAPACITY);

        // Create menu
        let menu = Menu::new();

        // Create menu items
        let start_recording = MenuItem::new("Start Recording", true, None);
        let stop_recording = MenuItem::new("Stop Recording", false, None);
        let settings = MenuItem::new("Settings...", true, None);
        let about = MenuItem::new("About Hush", true, None);
        let quit = PredefinedMenuItem::quit(Some("Quit Hush"));

        let menu_items = MenuItems {
            start_recording,
            stop_recording,
            history_submenu: None,
            settings,
            about,
            quit,
        };

        Ok(Self {
            tray_icon: None,
            menu,
            event_sender,
            event_receiver,
            current_state: TrayIconState::Idle,
            tooltip: "Hush Voice-to-Text".to_string(),
            menu_items,
        })
    }

    /// Load icon from embedded SVG data
    fn load_icon_for_state(state: TrayIconState) -> Result<Icon> {
        let icon_data = match state {
            TrayIconState::Idle => ICON_IDLE,
            TrayIconState::Recording => ICON_RECORDING,
            TrayIconState::Processing => ICON_PROCESSING,
            TrayIconState::Error => ICON_ERROR,
        };

        // tray-icon doesn't support SVG directly, so we need to convert
        // For now, we'll use a simple approach with image crate
        Icon::from_rgba(Self::svg_to_rgba(icon_data)?, 32, 32)
            .map_err(|e| anyhow::anyhow!("Failed to create icon: {:?}", e))
    }

    /// Convert SVG to RGBA bytes (32x32)
    ///
    /// This is a simple implementation that renders the SVG to a raster image.
    /// For production, consider pre-rendering icons or using a more efficient method.
    fn svg_to_rgba(svg_data: &[u8]) -> Result<Vec<u8>> {
        // For MVP, we'll create a simple colored square as a placeholder
        // In production, you'd want to use resvg or similar to render the SVG

        // Create a 32x32 RGBA image (1024 * 4 = 4096 bytes)
        let size = 32;
        let mut rgba = vec![0u8; (size * size * 4) as usize];

        // Determine color based on SVG content (simple heuristic)
        let is_recording = svg_data.windows(9).any(|w| w == b"recording");
        let is_processing = svg_data.windows(10).any(|w| w == b"processing");
        let is_error = svg_data.windows(5).any(|w| w == b"error");

        let (r, g, b) = if is_recording {
            (255, 0, 0) // Red for recording
        } else if is_processing {
            (255, 165, 0) // Orange for processing
        } else if is_error {
            (220, 20, 60) // Crimson for error
        } else {
            (128, 128, 128) // Gray for idle
        };

        // Draw a simple microphone icon shape
        for y in 0..size {
            for x in 0..size {
                let idx = ((y * size + x) * 4) as usize;

                // Simple microphone shape: circle + stem
                let cx = 16.0;
                let cy = 12.0;
                let radius = 6.0;

                let dx = (x as f32 - cx).abs();
                let dy = (y as f32 - cy).abs();
                let dist = (dx * dx + dy * dy).sqrt();

                // Circle (microphone head)
                if dist <= radius {
                    rgba[idx] = r;
                    rgba[idx + 1] = g;
                    rgba[idx + 2] = b;
                    rgba[idx + 3] = 255; // Alpha
                }
                // Stem
                else if x >= 14 && x <= 18 && y >= 18 && y <= 28 {
                    rgba[idx] = r;
                    rgba[idx + 1] = g;
                    rgba[idx + 2] = b;
                    rgba[idx + 3] = 255;
                }
            }
        }

        Ok(rgba)
    }

    /// Build menu from current state and menu data
    fn build_menu(&mut self, menu_data: Option<TrayMenu>) -> Result<()> {
        debug!("Building tray menu");

        // Clear existing menu items
        let new_menu = Menu::new();

        // Add status item (non-clickable label)
        let status_text = format!("Status: {:?}", self.current_state);
        let status_item = MenuItem::new(&status_text, false, None);
        new_menu.append(&status_item)
            .map_err(|e| anyhow::anyhow!("Failed to add menu item: {:?}", e))?;

        // Separator
        new_menu.append(&PredefinedMenuItem::separator())
            .map_err(|e| anyhow::anyhow!("Failed to add separator: {:?}", e))?;

        // Recording controls based on state
        match self.current_state {
            TrayIconState::Recording => {
                new_menu.append(&self.menu_items.stop_recording)
                    .map_err(|e| anyhow::anyhow!("Failed to add menu item: {:?}", e))?;
            }
            _ => {
                new_menu.append(&self.menu_items.start_recording)
                    .map_err(|e| anyhow::anyhow!("Failed to add menu item: {:?}", e))?;
            }
        }

        // Add recent transcriptions if available
        if let Some(ref menu) = menu_data {
            if !menu.recent_transcriptions.is_empty() {
                new_menu.append(&PredefinedMenuItem::separator())
                    .map_err(|e| anyhow::anyhow!("Failed to add separator: {:?}", e))?;

                let history_menu = Submenu::new("Recent Transcriptions", true);

                for (i, item) in menu.recent_transcriptions.iter().enumerate() {
                    if i >= 5 { break; } // Limit to 5 recent items

                    let preview = if item.preview.len() > 40 {
                        format!("{}...", &item.preview[..40])
                    } else {
                        item.preview.clone()
                    };

                    let history_item = MenuItem::new(&preview, true, None);
                    history_menu.append(&history_item)
                        .map_err(|e| anyhow::anyhow!("Failed to add history item: {:?}", e))?;
                }

                new_menu.append(&history_menu)
                    .map_err(|e| anyhow::anyhow!("Failed to add submenu: {:?}", e))?;
            }
        }

        // Separator before settings
        new_menu.append(&PredefinedMenuItem::separator())
            .map_err(|e| anyhow::anyhow!("Failed to add separator: {:?}", e))?;

        // Settings
        new_menu.append(&self.menu_items.settings)
            .map_err(|e| anyhow::anyhow!("Failed to add menu item: {:?}", e))?;

        // About
        new_menu.append(&self.menu_items.about)
            .map_err(|e| anyhow::anyhow!("Failed to add menu item: {:?}", e))?;

        // Separator before quit
        new_menu.append(&PredefinedMenuItem::separator())
            .map_err(|e| anyhow::anyhow!("Failed to add separator: {:?}", e))?;

        // Quit
        new_menu.append(&self.menu_items.quit)
            .map_err(|e| anyhow::anyhow!("Failed to add menu item: {:?}", e))?;

        // Update menu reference
        self.menu = new_menu;

        // Update tray icon with new menu
        if let Some(ref tray) = self.tray_icon {
            tray.set_menu(Some(self.menu.clone()));
        }

        Ok(())
    }

    /// Start menu event listener
    fn start_event_listener(&self) {
        let event_sender = self.event_sender.clone();

        std::thread::spawn(move || {
            debug!("Starting tray menu event listener");
            let menu_channel = MenuEvent::receiver();

            loop {
                match menu_channel.recv() {
                    Ok(event) => {
                        debug!("Tray menu event received: {:?}", event.id);

                        // Map menu event to TrayEvent
                        // Note: tray-icon doesn't provide item metadata, so we'd need to
                        // track menu item IDs. For MVP, we'll handle this in a basic way.

                        // In a real implementation, you'd want to associate menu item IDs
                        // with specific actions. For now, we'll just send a generic event.
                        let tray_event = TrayEvent::MenuClicked(TrayMenuItem::Show);

                        if let Err(e) = event_sender.blocking_send(tray_event) {
                            error!("Failed to send tray event: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Error receiving menu event: {}", e);
                        break;
                    }
                }
            }
        });
    }
}

#[async_trait]
impl SystemTray for MacOSTrayAdapter {
    async fn show(&mut self) -> Result<()> {
        info!("Showing macOS system tray icon");

        // Create icon
        let icon = Self::load_icon_for_state(self.current_state)?;

        // Build initial menu
        self.build_menu(None)?;

        // Create tray icon
        let tray_icon = TrayIconBuilder::new()
            .with_tooltip(&self.tooltip)
            .with_icon(icon)
            .with_menu(Box::new(self.menu.clone()))
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create tray icon: {:?}", e))?;

        self.tray_icon = Some(tray_icon);

        // Start event listener
        self.start_event_listener();

        info!("macOS system tray icon created successfully");
        Ok(())
    }

    async fn hide(&mut self) -> Result<()> {
        info!("Hiding macOS system tray icon");

        // Drop the tray icon to remove it from the menu bar
        self.tray_icon = None;

        Ok(())
    }

    fn set_icon(&mut self, state: TrayIconState) {
        debug!("Setting tray icon state to: {:?}", state);
        self.current_state = state;

        // Update icon if tray is visible
        if let Some(ref tray) = self.tray_icon {
            match Self::load_icon_for_state(state) {
                Ok(icon) => {
                    if let Err(e) = tray.set_icon(Some(icon)) {
                        error!("Failed to update tray icon: {:?}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to load icon for state {:?}: {}", state, e);
                }
            }
        }
    }

    fn set_tooltip(&mut self, text: &str) {
        debug!("Setting tray tooltip to: {}", text);
        self.tooltip = text.to_string();

        if let Some(ref tray) = self.tray_icon {
            if let Err(e) = tray.set_tooltip(Some(text)) {
                error!("Failed to set tooltip: {:?}", e);
            }
        }
    }

    fn update_menu(&mut self, menu: TrayMenu) {
        debug!("Updating tray menu with {} recent items", menu.recent_transcriptions.len());

        if let Err(e) = self.build_menu(Some(menu)) {
            error!("Failed to update menu: {}", e);
        }
    }

    fn event_receiver(&self) -> &mpsc::Receiver<TrayEvent> {
        &self.event_receiver
    }
}
