use anyhow::Result;
use egui_overlay::{EguiOverlay, egui_window_glfw_passthrough::GlfwBackend, egui_render_three_d::ThreeDBackend};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{debug, info};

use super::state::{OverlayState, OverlayConfig};
use super::ui::{render_overlay, OverlayAction};

/// Internal struct that implements the EguiOverlay trait
struct OverlayApp {
    state: Arc<Mutex<OverlayState>>,
    config: OverlayConfig,
}

impl EguiOverlay for OverlayApp {
    fn gui_run(
        &mut self,
        egui_context: &egui::Context,
        _default_gfx_backend: &mut ThreeDBackend,
        _glfw_backend: &mut GlfwBackend,
    ) {
        // Get current state
        let current_state = self.state.lock().unwrap().clone();

        // Check if we should auto-hide
        if current_state.should_hide() {
            debug!("Auto-hiding overlay");
            *self.state.lock().unwrap() = OverlayState::Idle;
        }

        // Render the UI
        let action = render_overlay(egui_context, &current_state, &self.config);

        // Handle UI actions
        match action {
            OverlayAction::StartRecording => {
                info!("User clicked to start recording");
                *self.state.lock().unwrap() = OverlayState::start_recording();
            }
            OverlayAction::Settings => {
                info!("User opened settings");
                // TODO: Implement settings UI
            }
            OverlayAction::None => {}
        }

        // Request repaint for animations
        egui_context.request_repaint_after(Duration::from_millis(100));
    }
}

/// The main overlay window manager
pub struct OverlayWindow {
    state: Arc<Mutex<OverlayState>>,
    config: OverlayConfig,
}

impl OverlayWindow {
    /// Create a new overlay window
    pub fn new(config: OverlayConfig) -> Self {
        info!("Creating overlay window");
        Self {
            state: Arc::new(Mutex::new(OverlayState::default())),
            config,
        }
    }

    /// Get a clone of the state Arc for external access
    pub fn state(&self) -> Arc<Mutex<OverlayState>> {
        self.state.clone()
    }

    /// Update the overlay state
    pub fn set_state(&self, new_state: OverlayState) {
        debug!("Overlay state transition: {:?}", new_state);
        if let Ok(mut state) = self.state.lock() {
            *state = new_state;
        }
    }

    /// Run the overlay window (blocking)
    pub fn run(self) -> Result<()> {
        info!("Starting overlay window event loop");

        // Create the app that implements EguiOverlay
        let app = OverlayApp {
            state: self.state,
            config: self.config,
        };

        // Start the overlay
        egui_overlay::start(app);

        info!("Overlay window closed");
        Ok(())
    }
}

/// Builder for configuring an overlay window
pub struct OverlayWindowBuilder {
    config: OverlayConfig,
}

impl OverlayWindowBuilder {
    /// Create a new builder with default config
    pub fn new() -> Self {
        Self {
            config: OverlayConfig::default(),
        }
    }

    /// Set the window width
    pub fn width(mut self, width: f32) -> Self {
        self.config.width = width;
        self
    }

    /// Set the window height
    pub fn height(mut self, height: f32) -> Self {
        self.config.height = height;
        self
    }

    /// Set the window position
    pub fn position(mut self, position: super::state::OverlayPosition) -> Self {
        self.config.position = position;
        self
    }

    /// Set the opacity
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.config.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Set whether to show button when idle
    pub fn show_button_when_idle(mut self, show: bool) -> Self {
        self.config.show_button_when_idle = show;
        self
    }

    /// Set the theme
    pub fn theme(mut self, theme: super::state::OverlayTheme) -> Self {
        self.config.theme = theme;
        self
    }

    /// Build the overlay window
    pub fn build(self) -> OverlayWindow {
        OverlayWindow::new(self.config)
    }
}

impl Default for OverlayWindowBuilder {
    fn default() -> Self {
        Self::new()
    }
}
