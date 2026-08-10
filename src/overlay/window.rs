use anyhow::Result;

use egui_overlay::{
    egui_render_three_d::ThreeDBackend, egui_window_glfw_passthrough::GlfwBackend, EguiOverlay,
};

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

use super::state::{OverlayConfig, OverlayState, OverlayTheme};
use super::ui::{render_overlay, OverlayAction};

// Cache primary monitor info for faster overlay startup
static PRIMARY_MONITOR_INFO: Lazy<([u32; 2], [i32; 2])> = Lazy::new(|| {
    get_primary_monitor_info().unwrap_or_else(|| {
        warn!("Failed to detect primary monitor, using fallback 1920x1080 at (0, 0)");
        ([1920, 1080], [0, 0])
    })
});

/// Internal struct that implements the EguiOverlay trait
struct OverlayApp {
    state: Arc<Mutex<OverlayState>>,
    config: Arc<Mutex<OverlayConfig>>,
    /// Shared storage for egui context - populated on first gui_run call
    egui_context_storage: Arc<Mutex<Option<egui::Context>>>,
}

impl EguiOverlay for OverlayApp {
    fn gui_run(
        &mut self,
        egui_context: &egui::Context,
        _default_gfx_backend: &mut ThreeDBackend,
        glfw_backend: &mut GlfwBackend,
    ) {
        // Store egui context for external repaint requests (first call only)
        {
            let mut ctx_storage = self.egui_context_storage.lock();
            if ctx_storage.is_none() {
                debug!("Storing egui context for external repaint requests");
                *ctx_storage = Some(egui_context.clone());
            }
        }

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

        // Get config for visibility check
        let current_config = self.config.lock().clone();

        // Hide/show window based on state
        let should_be_visible = match &current_state {
            OverlayState::Idle => current_config.show_button_when_idle,
            _ => true,
        };

        if should_be_visible {
            glfw_backend.window.show();
        } else {
            glfw_backend.window.hide();
        }

        // Render the UI
        let action = render_overlay(egui_context, &current_state, &current_config);

        // Toggle passthrough based on whether mouse is over UI
        let is_pointer_over_area = egui_context.is_pointer_over_area();
        glfw_backend
            .window
            .set_mouse_passthrough(!is_pointer_over_area);

        // Handle UI actions
        match action {
            OverlayAction::StartRecording => {
                info!("User clicked to start recording");
                *self.state.lock() = OverlayState::start_recording();
            },
            OverlayAction::CloseSettings => {
                info!("User closed settings");
                *self.state.lock() = OverlayState::Idle;
            },
            OverlayAction::ToggleTheme => {
                info!("User toggled theme");
                let mut config = self.config.lock();
                config.theme = match config.theme {
                    OverlayTheme::Dark => OverlayTheme::Light,
                    OverlayTheme::Light => OverlayTheme::Dark,
                };
            },
            OverlayAction::None => {},
        }

        // Adaptive repaint rate based on state
        let repaint_interval = match &current_state {
            OverlayState::Recording { .. } => Duration::from_millis(50),
            OverlayState::Settings => Duration::from_millis(100),
            OverlayState::Idle if current_config.show_button_when_idle => Duration::from_secs(1),
            OverlayState::Idle => Duration::from_secs(5),
            _ => Duration::from_millis(500),
        };
        egui_context.request_repaint_after(repaint_interval);
    }
}

/// The main overlay window manager
pub struct OverlayWindow {
    state: Arc<Mutex<OverlayState>>,
    config: Arc<Mutex<OverlayConfig>>,
    /// Shared storage for egui context - allows external repaint requests
    egui_context: Arc<Mutex<Option<egui::Context>>>,
}

impl OverlayWindow {
    /// Create a new overlay window
    pub fn new(config: OverlayConfig) -> Self {
        info!("Creating overlay window");
        Self {
            state: Arc::new(Mutex::new(OverlayState::default())),
            config: Arc::new(Mutex::new(config)),
            egui_context: Arc::new(Mutex::new(None)),
        }
    }

    /// Get a clone of the state Arc for external access
    pub fn state(&self) -> Arc<Mutex<OverlayState>> {
        self.state.clone()
    }

    /// Get a clone of the egui context Arc for external repaint requests
    pub fn egui_context(&self) -> Arc<Mutex<Option<egui::Context>>> {
        self.egui_context.clone()
    }

    /// Request an immediate repaint of the overlay
    /// Call this after updating state to avoid waiting for the next scheduled repaint
    pub fn request_repaint(&self) {
        if let Some(ctx) = self.egui_context.lock().as_ref() {
            ctx.request_repaint();
        }
    }

    /// Update the overlay state
    pub fn set_state(&self, new_state: OverlayState) {
        debug!("Overlay state transition: {:?}", new_state);
        *self.state.lock() = new_state;
    }

    /// Run the overlay window (blocking)
    pub fn run(self) -> Result<()> {
        info!("Starting overlay window event loop");

        let app = OverlayApp {
            state: self.state,
            config: self.config,
            egui_context_storage: self.egui_context,
        };

        start_fullscreen_overlay(app);

        info!("Overlay window closed");
        Ok(())
    }
}

/// Custom start function that creates a fullscreen overlay
fn start_fullscreen_overlay<T: EguiOverlay + 'static>(user_data: T) {
    use egui_overlay::egui_window_glfw_passthrough::{glfw, GlfwBackend, GlfwConfig};

    let (monitor_size, monitor_pos) = *PRIMARY_MONITOR_INFO;
    info!(
        "Creating fullscreen overlay window: {}x{} at ({}, {})",
        monitor_size[0], monitor_size[1], monitor_pos[0], monitor_pos[1]
    );

    let mut glfw_backend = GlfwBackend::new(GlfwConfig {
        size: monitor_size,
        glfw_callback: Box::new(|gtx| {
            gtx.window_hint(glfw::WindowHint::ScaleToMonitor(true));
            gtx.window_hint(glfw::WindowHint::Floating(true));
            gtx.window_hint(glfw::WindowHint::Decorated(false));
            gtx.window_hint(glfw::WindowHint::FocusOnShow(false));
            gtx.window_hint(glfw::WindowHint::AutoIconify(false));
        }),
        opengl_window: Some(true),
        transparent_window: Some(true),
        window_title: "hush overlay".to_string(),
        ..Default::default()
    });

    glfw_backend.window.set_floating(true);
    glfw_backend.window.set_decorated(false);
    glfw_backend.window.set_pos(monitor_pos[0], monitor_pos[1]);

    // Hide from taskbar on X11
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        if let Ok(handle) = glfw_backend.window.window_handle() {
            if let RawWindowHandle::Xlib(xlib_handle) = handle.as_raw() {
                if let Err(e) = set_skip_taskbar_x11(xlib_handle.window as u32) {
                    warn!("Failed to set skip taskbar hint: {}", e);
                } else {
                    info!("Window hidden from taskbar");
                }
            }
        }
    }

    let latest_size = glfw_backend.window.get_framebuffer_size();
    let latest_size = [latest_size.0 as u32, latest_size.1 as u32];

    let default_gfx_backend = ThreeDBackend::new(
        egui_overlay::egui_render_three_d::ThreeDConfig {
            ..Default::default()
        },
        |s| glfw_backend.get_proc_address(s),
        latest_size,
    );

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

    let mut glfw_context = glfw::init(glfw::fail_on_errors).ok()?;

    glfw_context.with_connected_monitors(|_, monitors| {
        info!("Detected {} monitor(s)", monitors.len());
        for (i, monitor) in monitors.iter().enumerate() {
            if let Some(mode) = monitor.get_video_mode() {
                let pos = monitor.get_pos();
                info!(
                    "  Monitor {}: {}x{} at ({}, {})",
                    i, mode.width, mode.height, pos.0, pos.1
                );
            }
        }
    });

    glfw_context.with_primary_monitor(|_, m| {
        m.and_then(|mon| {
            let mode = mon.get_video_mode()?;
            let pos = mon.get_pos();
            info!(
                "Using primary monitor: {}x{} at ({}, {})",
                mode.width, mode.height, pos.0, pos.1
            );
            Some(([mode.width, mode.height], [pos.0, pos.1]))
        })
    })
}

/// Builder for configuring an overlay window
pub struct OverlayWindowBuilder {
    config: OverlayConfig,
}

impl OverlayWindowBuilder {
    pub fn new() -> Self {
        Self {
            config: OverlayConfig::default(),
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.config.width = width;
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.config.height = height;
        self
    }

    pub fn position(mut self, position: super::state::OverlayPosition) -> Self {
        self.config.position = position;
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.config.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn show_button_when_idle(mut self, show: bool) -> Self {
        self.config.show_button_when_idle = show;
        self
    }

    pub fn theme(mut self, theme: super::state::OverlayTheme) -> Self {
        self.config.theme = theme;
        self
    }

    pub fn hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.config.hotkey = hotkey.into();
        self
    }

    pub fn build(self) -> OverlayWindow {
        OverlayWindow::new(self.config)
    }
}

impl Default for OverlayWindowBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Set X11 window properties to hide from taskbar and prevent focus stealing
fn set_skip_taskbar_x11(window_id: u32) -> anyhow::Result<()> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ChangeWindowAttributesAux, ConnectionExt, PropMode};
    use x11rb::rust_connection::RustConnection;
    use x11rb::wrapper::ConnectionExt as WrapperConnectionExt;

    let (conn, _screen_num) = RustConnection::connect(None)?;

    // Set override_redirect to bypass window manager decorations
    let values = ChangeWindowAttributesAux::new().override_redirect(1);
    conn.change_window_attributes(window_id, &values)?;

    // Set window type to NOTIFICATION - this tells the WM to never focus this window
    let wm_window_type = conn
        .intern_atom(false, b"_NET_WM_WINDOW_TYPE")?
        .reply()?
        .atom;
    let type_notification = conn
        .intern_atom(false, b"_NET_WM_WINDOW_TYPE_NOTIFICATION")?
        .reply()?
        .atom;

    conn.change_property32(
        PropMode::REPLACE,
        window_id,
        wm_window_type,
        AtomEnum::ATOM,
        &[type_notification],
    )?;

    // Set _NET_WM_STATE_ABOVE for always-on-top without focus issues
    let wm_state = conn.intern_atom(false, b"_NET_WM_STATE")?.reply()?.atom;
    let state_above = conn
        .intern_atom(false, b"_NET_WM_STATE_ABOVE")?
        .reply()?
        .atom;

    conn.change_property32(
        PropMode::REPLACE,
        window_id,
        wm_state,
        AtomEnum::ATOM,
        &[state_above],
    )?;

    conn.flush()?;
    Ok(())
}
