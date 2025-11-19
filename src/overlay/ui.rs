use super::state::{OverlayConfig, OverlayState, OverlayTheme};
use egui::{Color32, Context, Frame, RichText, Sense, Stroke};

// Overlay dimensions
const IDLE_WIDTH: f32 = 60.0;
const IDLE_HEIGHT: f32 = 4.0;
const RECORDING_WIDTH: f32 = 80.0;
const RECORDING_HEIGHT: f32 = 24.0;

// Theme colors - alpha values
const BG_ALPHA_DARK: u8 = 200; // ~78% opacity
const BG_ALPHA_LIGHT: u8 = 200;
const BORDER_ALPHA: u8 = 60; // ~23% opacity

// Waveform animation parameters
const WAVEFORM_NUM_BARS: usize = 12;
const WAVEFORM_BAR_SPACING: f32 = 1.0;
const WAVEFORM_BAR_WIDTH: f32 = 2.5;
const WAVEFORM_BASE_HEIGHT: f32 = 3.0;
const WAVEFORM_MAX_HEIGHT: f32 = 16.0;

/// Render the overlay UI based on current state
pub fn render_overlay(
    ctx: &Context,
    state: &OverlayState,
    config: &OverlayConfig,
) -> OverlayAction {
    let mut action = OverlayAction::None;

    // Configure the style based on theme
    apply_theme(ctx, config.theme);

    // Dynamic sizing based on state
    let (width, height) = match state {
        OverlayState::Idle => (IDLE_WIDTH, IDLE_HEIGHT), // Very short pill when inactive
        OverlayState::Recording { .. } => (RECORDING_WIDTH, RECORDING_HEIGHT), // Expand taller when recording
        OverlayState::Settings => (220.0, 140.0), // Larger panel for settings
        _ => (RECORDING_WIDTH, RECORDING_HEIGHT), // Other states use recording size
    };

    // Get screen dimensions and calculate default position
    let screen_rect = ctx.screen_rect();
    let (default_x, default_y) =
        config.get_window_position(screen_rect.width(), screen_rect.height());

    // Create a draggable overlay area
    // The area will remember its position between frames
    egui::Area::new(egui::Id::new("hush_overlay"))
        .default_pos([default_x, default_y])
        .movable(true)  // Enable dragging
        .interactable(true)
        .show(ctx, |ui| {
            // Create a frame with rounded corners and shadow
            let frame = create_frame(config, height);

            frame.show(ui, |ui| {
                ui.set_width(width);
                ui.set_height(height);

                // Render based on state
                match state {
                    OverlayState::Idle => {
                        if config.show_button_when_idle {
                            action = render_idle_state(ui, config);
                        }
                    }
                    OverlayState::Recording { start_time, amplitude } => {
                        render_recording_state(ui, start_time.elapsed().as_secs_f32(), *amplitude, config);
                    }
                    OverlayState::Processing { message } => {
                        render_processing_state(ui, message, config);
                    }
                    OverlayState::Success { text, .. } => {
                        render_success_state(ui, text, config);
                    }
                    OverlayState::Error { message, .. } => {
                        render_error_state(ui, message, config);
                    }
                    OverlayState::Settings => {
                        action = render_settings_state(ui, config);
                    }
                }
            });
        }).inner;

    action
}

/// Actions that can be triggered by UI interactions
#[derive(Debug, Clone, PartialEq)]
pub enum OverlayAction {
    None,
    StartRecording,
    Settings,
    CloseSettings,
    ToggleTheme,
}

fn apply_theme(ctx: &Context, theme: OverlayTheme) {
    let mut style = (*ctx.style()).clone();

    match theme {
        OverlayTheme::Dark => {
            style.visuals.window_fill = Color32::from_rgba_unmultiplied(20, 20, 20, 240);
            style.visuals.panel_fill = Color32::from_rgba_unmultiplied(20, 20, 20, 240);
        },
        OverlayTheme::Light => {
            style.visuals.window_fill = Color32::from_rgba_unmultiplied(240, 240, 240, 240);
            style.visuals.panel_fill = Color32::from_rgba_unmultiplied(240, 240, 240, 240);
        },
    }

    ctx.set_style(style);
}

fn create_frame(config: &OverlayConfig, height: f32) -> Frame {
    let (bg_color, border_color) = match config.theme {
        OverlayTheme::Dark => (
            Color32::from_black_alpha(BG_ALPHA_DARK),
            Color32::from_white_alpha(BORDER_ALPHA),
        ),
        OverlayTheme::Light => (
            Color32::from_white_alpha(BG_ALPHA_LIGHT),
            Color32::from_black_alpha(BORDER_ALPHA),
        ),
    };

    Frame::none()
        .fill(bg_color)
        .stroke(Stroke::new(1.0, border_color))  // 1px subtle border
        .rounding(height / 2.0)  // Perfect pill shape (height/2)
        .inner_margin(0.0)  // No padding - keep it tight
        .shadow(egui::epaint::Shadow {
            extrusion: 4.0,
            color: Color32::from_black_alpha(100),
        })
}

fn render_idle_state(ui: &mut egui::Ui, _config: &OverlayConfig) -> OverlayAction {
    let mut action = OverlayAction::None;

    // Empty idle state - just a tiny black pill (10x20)
    // Make the entire area clickable
    let response = ui.allocate_response(ui.available_size(), Sense::click());

    if response.clicked() {
        action = OverlayAction::StartRecording;
    }

    action
}

fn render_recording_state(
    ui: &mut egui::Ui,
    duration: f32,
    amplitude: f32,
    _config: &OverlayConfig,
) {
    // Waveform animation - draw bars directly using painter for performance
    // Creates an animated waveform visualization that responds to audio amplitude

    // Pre-calculated constants for performance
    const TOTAL_WIDTH: f32 = (WAVEFORM_NUM_BARS as f32 * WAVEFORM_BAR_WIDTH)
        + ((WAVEFORM_NUM_BARS - 1) as f32 * WAVEFORM_BAR_SPACING);
    const ROUNDING: f32 = WAVEFORM_BAR_WIDTH / 2.0;

    // Get the rect we can draw in
    let available_rect = ui.available_rect_before_wrap();
    let available_width = available_rect.width();

    // Calculate starting position (centered)
    let start_x = available_rect.left() + (available_width - TOTAL_WIDTH) / 2.0;
    let center_y = available_rect.center().y;

    // Draw each bar directly using the painter
    let painter = ui.painter();
    for i in 0..WAVEFORM_NUM_BARS {
        // Each bar has a phase offset for visual variety - creates a wave effect
        // The phase shifts the sine wave for each bar, making them animate at slightly different times
        let phase = i as f32 * 0.2;

        // time_factor creates the animation: sin wave normalized to 0.0-1.0 range
        // Multiplied by 10.0 to speed up the animation
        let time_factor = ((duration * 10.0 + phase).sin() + 1.0) / 2.0;

        // Calculate bar height: base height + (amplitude × animation × max height)
        // This makes bars grow with both audio amplitude AND time-based animation
        let height = WAVEFORM_BASE_HEIGHT + (amplitude * time_factor * WAVEFORM_MAX_HEIGHT);

        // Calculate bar position (horizontally spaced, vertically centered)
        let x = start_x + (i as f32 * (WAVEFORM_BAR_WIDTH + WAVEFORM_BAR_SPACING));
        let y = center_y - height * 0.5;

        // Draw the bar with rounded ends (pill-shaped) for visual consistency with overlay
        let bar_rect =
            egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(WAVEFORM_BAR_WIDTH, height));
        painter.rect_filled(bar_rect, ROUNDING, Color32::WHITE);
    }

    // Allocate the space we used
    ui.allocate_rect(available_rect, Sense::hover());
}

fn render_processing_state(ui: &mut egui::Ui, _message: &str, _config: &OverlayConfig) {
    // Minimal processing indicator
    ui.vertical_centered(|ui| {
        ui.add_space(4.0);

        let text = RichText::new("processing...")
            .size(10.0)
            .color(Color32::WHITE);
        ui.label(text);

        ui.add_space(4.0);
    });
}

fn render_success_state(ui: &mut egui::Ui, _text: &str, _config: &OverlayConfig) {
    // Minimal success indicator
    ui.vertical_centered(|ui| {
        ui.add_space(4.0);

        let text = RichText::new("done").size(10.0).color(Color32::WHITE);
        ui.label(text);

        ui.add_space(4.0);
    });
}

fn render_error_state(ui: &mut egui::Ui, _message: &str, _config: &OverlayConfig) {
    // Minimal error indicator
    ui.vertical_centered(|ui| {
        ui.add_space(4.0);

        let text = RichText::new("error").size(10.0).color(Color32::WHITE);
        ui.label(text);

        ui.add_space(4.0);
    });
}

fn render_settings_state(ui: &mut egui::Ui, config: &OverlayConfig) -> OverlayAction {
    let mut action = OverlayAction::None;

    ui.vertical(|ui| {
        ui.add_space(8.0);

        // Title
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let title = RichText::new("Hush Settings")
                .size(12.0)
                .strong()
                .color(Color32::WHITE);
            ui.label(title);
        });

        ui.add_space(8.0);

        // Theme toggle
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let label = RichText::new("Theme:").size(10.0).color(Color32::GRAY);
            ui.label(label);

            ui.add_space(4.0);

            let theme_text = match config.theme {
                OverlayTheme::Dark => "Dark",
                OverlayTheme::Light => "Light",
            };

            if ui
                .button(RichText::new(theme_text).size(10.0).color(Color32::WHITE))
                .clicked()
            {
                action = OverlayAction::ToggleTheme;
            }
        });

        ui.add_space(6.0);

        // Hotkey info
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let label = RichText::new("Hotkey:").size(10.0).color(Color32::GRAY);
            ui.label(label);

            ui.add_space(4.0);

            let hotkey_text = RichText::new("Ctrl+Alt+V").size(10.0).color(Color32::WHITE);
            ui.label(hotkey_text);
        });

        ui.add_space(6.0);

        // Editing mode info (read-only for now, would need config plumbing to change)
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let label = RichText::new("Text Processing:")
                .size(10.0)
                .color(Color32::GRAY);
            ui.label(label);

            ui.add_space(4.0);

            let mode_text = RichText::new("Medium").size(10.0).color(Color32::WHITE);
            ui.label(mode_text);
        });

        ui.add_space(8.0);

        // Close button
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            if ui
                .button(RichText::new("Close").size(10.0).color(Color32::WHITE))
                .clicked()
            {
                action = OverlayAction::CloseSettings;
            }
        });

        ui.add_space(8.0);
    });

    action
}
