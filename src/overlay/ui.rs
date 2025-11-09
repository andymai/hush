use egui::{Context, Frame, Stroke, Color32, RichText, Sense};
use super::state::{OverlayState, OverlayConfig, OverlayTheme};

/// Render the overlay UI based on current state
pub fn render_overlay(ctx: &Context, state: &OverlayState, config: &OverlayConfig) -> OverlayAction {
    let mut action = OverlayAction::None;

    // Configure the style based on theme
    apply_theme(ctx, config.theme);

    // Dynamic sizing based on state
    let (width, height) = match state {
        OverlayState::Idle => (10.0, 20.0),  // Tiny idle state
        OverlayState::Recording { .. } => (80.0, 20.0),  // Expand when recording
        _ => (80.0, 20.0),  // Other states use recording size
    };

    // Get screen dimensions and calculate default position
    let screen_rect = ctx.screen_rect();
    let (default_x, default_y) = config.get_window_position(screen_rect.width(), screen_rect.height());

    // Create a draggable overlay area
    // The area will remember its position between frames
    egui::Area::new(egui::Id::new("hush_overlay"))
        .default_pos([default_x, default_y])
        .movable(true)  // Enable dragging
        .interactable(true)
        .show(ctx, |ui| {
            // Create a frame with rounded corners and shadow
            let frame = create_frame(config);

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
                    OverlayState::Editing { message } => {
                        render_editing_state(ui, message, config);
                    }
                    OverlayState::Success { text, .. } => {
                        render_success_state(ui, text, config);
                    }
                    OverlayState::Error { message, .. } => {
                        render_error_state(ui, message, config);
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
    #[allow(dead_code)]
    Settings,
}

fn apply_theme(ctx: &Context, theme: OverlayTheme) {
    let mut style = (*ctx.style()).clone();

    match theme {
        OverlayTheme::Dark => {
            style.visuals.window_fill = Color32::from_rgba_unmultiplied(20, 20, 20, 240);
            style.visuals.panel_fill = Color32::from_rgba_unmultiplied(20, 20, 20, 240);
        }
        OverlayTheme::Light => {
            style.visuals.window_fill = Color32::from_rgba_unmultiplied(240, 240, 240, 240);
            style.visuals.panel_fill = Color32::from_rgba_unmultiplied(240, 240, 240, 240);
        }
    }

    ctx.set_style(style);
}

fn create_frame(config: &OverlayConfig) -> Frame {
    let bg_color = match config.theme {
        OverlayTheme::Dark => Color32::BLACK,  // Pure black background
        OverlayTheme::Light => Color32::WHITE,
    };

    Frame::none()
        .fill(bg_color)
        .stroke(Stroke::NONE)  // No border
        .rounding(10.0)  // Perfect pill shape (height/2 = 20/2)
        .inner_margin(4.0)  // Minimal padding for tiny size
        .shadow(egui::epaint::Shadow {
            extrusion: 4.0,
            color: Color32::from_black_alpha(100),
        })
}

fn render_idle_state(ui: &mut egui::Ui, _config: &OverlayConfig) -> OverlayAction {
    let mut action = OverlayAction::None;

    // Empty idle state - just a tiny black pill (10x20)
    // Make the entire area clickable
    let response = ui.allocate_response(
        ui.available_size(),
        Sense::click()
    );

    if response.clicked() {
        action = OverlayAction::StartRecording;
    }

    action
}

fn render_recording_state(ui: &mut egui::Ui, duration: f32, amplitude: f32, _config: &OverlayConfig) {
    // Tiny waveform animation (80x20)
    ui.vertical_centered(|ui| {
        ui.add_space(2.0);

        // Draw animated waveform bars that respond to audio amplitude
        ui.horizontal(|ui| {
            ui.add_space(6.0);

            let num_bars = 10; // Fewer bars for tiny size
            let bar_spacing = 1.5;

            // Create bars that respond to amplitude with slight variation
            for i in 0..num_bars {
                // Each bar has a slight phase offset for visual variety
                let phase = i as f32 * 0.15;
                let time_factor = ((duration * 10.0 + phase).sin() + 1.0) / 2.0;

                // Base height on amplitude, with time factor for smooth animation
                let base_height = 2.0; // Minimum height
                let max_height = 12.0; // Maximum height (scaled for 20px height)
                let height = base_height + (amplitude * time_factor * max_height);

                let bar_color = Color32::WHITE;  // White bars on black background
                let bar_width = 2.0;  // Thin bars

                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(bar_width, height),
                    Sense::hover()
                );

                ui.painter().rect_filled(
                    rect,
                    1.0, // rounded corners
                    bar_color
                );

                ui.add_space(bar_spacing);
            }

            ui.add_space(6.0);
        });

        ui.add_space(2.0);
    });
}

fn render_processing_state(ui: &mut egui::Ui, _message: &str, _config: &OverlayConfig) {
    // Minimal processing indicator for tiny size
    ui.vertical_centered(|ui| {
        ui.add_space(1.0);

        // Simple spinner - small for 20px height
        let spinner = RichText::new("⟳")
            .size(14.0)
            .color(Color32::WHITE);
        ui.label(spinner);

        ui.add_space(1.0);
    });
}

fn render_editing_state(ui: &mut egui::Ui, _message: &str, _config: &OverlayConfig) {
    // Minimal AI polishing indicator for tiny size
    ui.vertical_centered(|ui| {
        ui.add_space(1.0);

        // Just sparkles icon - small
        let icon = RichText::new("✨")
            .size(14.0)
            .color(Color32::WHITE);  // White on black
        ui.label(icon);

        ui.add_space(1.0);
    });
}

fn render_success_state(ui: &mut egui::Ui, _text: &str, _config: &OverlayConfig) {
    // Minimal success indicator for tiny size
    ui.vertical_centered(|ui| {
        ui.add_space(1.0);

        // Just a checkmark - small
        let checkmark = RichText::new("✓")
            .size(14.0)
            .color(Color32::WHITE)  // White on black
            .strong();
        ui.label(checkmark);

        ui.add_space(1.0);
    });
}

fn render_error_state(ui: &mut egui::Ui, _message: &str, _config: &OverlayConfig) {
    // Minimal error indicator for tiny size
    ui.vertical_centered(|ui| {
        ui.add_space(1.0);

        // Just an X - small
        let error_icon = RichText::new("✗")
            .size(14.0)
            .color(Color32::WHITE)  // White on black
            .strong();
        ui.label(error_icon);

        ui.add_space(1.0);
    });
}

