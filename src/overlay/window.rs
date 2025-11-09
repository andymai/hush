use anyhow::Result;
use egui_overlay::{EguiOverlay, egui_window_glfw_passthrough::GlfwBackend, egui_render_three_d::ThreeDBackend};
use std::sync::Arc;
use parking_lot::Mutex;
use std::time::Duration;
use tracing::{debug, info, warn};
use once_cell::sync::Lazy;

use super::state::{OverlayState, OverlayConfig};
use super::ui::{render_overlay, OverlayAction};

// Cache primary monitor info for faster overlay startup
// Monitors rarely change during application runtime
static PRIMARY_MONITOR_INFO: Lazy<([u32; 2], [i32; 2])> = Lazy::new(|| {
    get_primary_monitor_info().unwrap_or_else(|| {
        warn!("Failed to detect primary monitor, using fallback 1920x1080 at (0, 0)");
        ([1920, 1080], [0, 0])
    })
});

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
        glfw_backend: &mut GlfwBackend,
    ) {
        // Single lock for reading state and checking auto-hide condition
        let (current_state, should_transition_to_idle) = {
            let state = self.state.lock();
            let should_hide = state.should_hide();
            (state.clone(), should_hide)
        };

        // Update state if needed (separate lock to minimize contention)
        if should_transition_to_idle {
            debug!("Auto-hiding overlay");
            *self.state.lock() = OverlayState::Idle;
        }

        // Render the UI
        let action = render_overlay(egui_context, &current_state, &self.config);

        // Toggle passthrough based on whether mouse is over UI
        // If egui is using the pointer (hovering over widgets), disable passthrough
        // so we can interact with the overlay. Otherwise, enable passthrough.
        let is_pointer_over_area = egui_context.is_pointer_over_area();
        glfw_backend.window.set_mouse_passthrough(!is_pointer_over_area);

        // Handle UI actions
        match action {
            OverlayAction::StartRecording => {
                info!("User clicked to start recording");
                *self.state.lock() = OverlayState::start_recording();
            }
            OverlayAction::Settings => {
                info!("User opened settings");
                // TODO: Implement settings UI
            }
            OverlayAction::None => {}
        }

        // Adaptive repaint rate based on state for better performance
        let repaint_interval = match &current_state {
            OverlayState::Recording { .. } => Duration::from_millis(50), // 20 FPS for smooth waveform animation
            OverlayState::Idle if self.config.show_button_when_idle => Duration::from_secs(1), // 1 FPS when showing idle button
            OverlayState::Idle => Duration::from_secs(5), // Very slow when completely hidden
            _ => Duration::from_millis(500), // 2 FPS for static states (processing/editing/success/error)
        };
        egui_context.request_repaint_after(repaint_interval);
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
        *self.state.lock() = new_state;
    }

    /// Run the overlay window (blocking)
    pub fn run(self) -> Result<()> {
        info!("Starting overlay window event loop");

        // Create the app that implements EguiOverlay
        let app = OverlayApp {
            state: self.state,
            config: self.config,
        };

        // Start the overlay with custom fullscreen configuration
        start_fullscreen_overlay(app);

        info!("Overlay window closed");
        Ok(())
    }
}

/// Custom start function that creates a fullscreen overlay
/// This is based on egui_overlay::start but with monitor-sized window
fn start_fullscreen_overlay<T: EguiOverlay + 'static>(user_data: T) {
    use egui_overlay::egui_window_glfw_passthrough::{GlfwBackend, GlfwConfig, glfw};

    // Get cached primary monitor size and position
    let (monitor_size, monitor_pos) = *PRIMARY_MONITOR_INFO;
    info!("Creating fullscreen overlay window: {}x{} at ({}, {})",
        monitor_size[0], monitor_size[1], monitor_pos[0], monitor_pos[1]);

    let mut glfw_backend = GlfwBackend::new(GlfwConfig {
        size: monitor_size,
        glfw_callback: Box::new(|gtx| {
            // Scale window based on monitor scale
            gtx.window_hint(glfw::WindowHint::ScaleToMonitor(true));
        }),
        #[cfg(not(target_os = "macos"))]
        opengl_window: Some(true), // OpenGL for non-macOS
        #[cfg(target_os = "macos")]
        opengl_window: Some(false), // macOS doesn't support OpenGL
        transparent_window: Some(true),
        window_title: "hush overlay".to_string(),
        ..Default::default()
    });

    // Always on top
    glfw_backend.window.set_floating(true);
    // Disable borders/titlebar
    glfw_backend.window.set_decorated(false);

    // Position at primary monitor's offset to cover that screen
    glfw_backend.window.set_pos(monitor_pos[0] as i32, monitor_pos[1] as i32);

    // Note: Mouse passthrough is toggled dynamically in gui_run()
    // based on whether the mouse is over the overlay widget

    let latest_size = glfw_backend.window.get_framebuffer_size();
    let latest_size = [latest_size.0 as u32, latest_size.1 as u32];

    // Create graphics backend (ThreeDBackend for OpenGL on non-macOS)
    #[cfg(not(target_os = "macos"))]
    let default_gfx_backend = {
        use raw_window_handle::HasRawWindowHandle;
        let handle = glfw_backend.window.raw_window_handle();
        ThreeDBackend::new(
            egui_overlay::egui_render_three_d::ThreeDConfig {
                ..Default::default()
            },
            |s| glfw_backend.get_proc_address(s),
            handle,
            latest_size,
        )
    };

    // macOS uses wgpu/metal
    #[cfg(target_os = "macos")]
    let default_gfx_backend = {
        use egui_overlay::egui_render_wgpu::WgpuBackend;
        WgpuBackend::new(
            egui_overlay::egui_render_wgpu::WgpuConfig {
                ..Default::default()
            },
            Some(&glfw_backend.window),
            latest_size,
        )
    };

    // Create the overlay app and run event loop
    let overlay_app = egui_overlay::OverlayApp {
        user_data,
        egui_context: Default::default(),
        default_gfx_backend,
        glfw_backend,
    };

    overlay_app.enter_event_loop();
}

/// Get the primary monitor's resolution and position
fn get_primary_monitor_info() -> Option<([u32; 2], [i32; 2])> {
    use egui_overlay::egui_window_glfw_passthrough::glfw;

    let mut glfw_context = glfw::init(glfw::FAIL_ON_ERRORS).ok()?;

    // Get all monitors and log them
    glfw_context.with_connected_monitors(|_, monitors| {
        info!("Detected {} monitor(s)", monitors.len());
        for (i, monitor) in monitors.iter().enumerate() {
            if let Some(mode) = monitor.get_video_mode() {
                let pos = monitor.get_pos();
                info!("  Monitor {}: {}x{} at ({}, {})", i, mode.width, mode.height, pos.0, pos.1);
            }
        }
    });

    // Get primary monitor
    glfw_context.with_primary_monitor(|_, m| {
        m.and_then(|mon| {
            let mode = mon.get_video_mode()?;
            let pos = mon.get_pos();
            info!("Using primary monitor: {}x{} at ({}, {})", mode.width, mode.height, pos.0, pos.1);
            Some(([mode.width, mode.height], [pos.0, pos.1]))
        })
    })
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
