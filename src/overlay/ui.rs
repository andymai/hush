use egui::{Context, Frame, Stroke, Color32, RichText, Sense};
use super::state::{OverlayState, OverlayConfig, OverlayTheme};

/// Render the overlay UI based on current state
pub fn render_overlay(ctx: &Context, state: &OverlayState, config: &OverlayConfig) -> OverlayAction {
    let mut action = OverlayAction::None;

    // Configure the style based on theme
    apply_theme(ctx, config.theme);

    // Get screen dimensions and calculate position
    let screen_rect = ctx.screen_rect();
    let (x, y) = config.get_window_position(screen_rect.width(), screen_rect.height());

    // Create a window that covers the overlay area
    egui::Area::new(egui::Id::new("hush_overlay"))
        .fixed_pos([x, y])
        .show(ctx, |ui| {
            // Create a frame with rounded corners and shadow
            let frame = create_frame(config);

            frame.show(ui, |ui| {
                ui.set_width(config.width);
                ui.set_height(config.height);

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
        });

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
        .stroke(Stroke::none())  // No border
        .rounding(35.0)  // Very rounded for pill shape (height/2)
        .inner_margin(12.0)
        .shadow(egui::epaint::Shadow {
            extrusion: 4.0,
            color: Color32::from_black_alpha(100),
        })
}

fn render_idle_state(ui: &mut egui::Ui, _config: &OverlayConfig) -> OverlayAction {
    let mut action = OverlayAction::None;

    // Minimal horizontal line indicating idle state
    ui.vertical_centered(|ui| {
        ui.add_space(23.0);

        // Draw a horizontal line
        ui.horizontal(|ui| {
            ui.add_space(30.0);

            let line_width = 220.0;  // Wider for pill shape
            let line_height = 3.0;
            let line_color = Color32::WHITE;  // White on black background

            // Make the line clickable
            let (rect, response) = ui.allocate_exact_size(
                egui::vec2(line_width, line_height),
                Sense::click()
            );

            // Draw rounded horizontal line
            ui.painter().rect_filled(
                rect,
                1.5, // rounded corners
                line_color
            );

            if response.clicked() {
                action = OverlayAction::StartRecording;
            }

            ui.add_space(30.0);
        });

        ui.add_space(23.0);
    });

    action
}

fn render_recording_state(ui: &mut egui::Ui, duration: f32, amplitude: f32, _config: &OverlayConfig) {
    // Wispr Flow-style waveform animation responding to voice
    ui.vertical_centered(|ui| {
        ui.add_space(10.0);

        // Draw animated waveform bars that respond to audio amplitude
        ui.horizontal(|ui| {
            ui.add_space(20.0);

            let num_bars = 18; // More bars for wider pill shape
            let bar_spacing = 3.0;

            // Create bars that respond to amplitude with slight variation
            for i in 0..num_bars {
                // Each bar has a slight phase offset for visual variety
                let phase = i as f32 * 0.15;
                let time_factor = ((duration * 10.0 + phase).sin() + 1.0) / 2.0;

                // Base height on amplitude, with time factor for smooth animation
                let base_height = 6.0; // Minimum height
                let max_height = 45.0; // Maximum height
                let height = base_height + (amplitude * time_factor * max_height);

                let bar_color = Color32::WHITE;  // White bars on black background
                let bar_width = 3.0;

                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(bar_width, height),
                    Sense::hover()
                );

                ui.painter().rect_filled(
                    rect,
                    1.5, // rounded corners
                    bar_color
                );

                ui.add_space(bar_spacing);
            }

            ui.add_space(20.0);
        });

        ui.add_space(10.0);
    });
}

fn render_processing_state(ui: &mut egui::Ui, _message: &str, _config: &OverlayConfig) {
    // Minimal processing indicator
    ui.vertical_centered(|ui| {
        ui.add_space(15.0);

        // Simple spinner
        let spinner = RichText::new("⟳")
            .size(28.0)
            .color(Color32::WHITE);
        ui.label(spinner);

        ui.add_space(15.0);
    });
}

fn render_editing_state(ui: &mut egui::Ui, _message: &str, _config: &OverlayConfig) {
    // Minimal AI polishing indicator
    ui.vertical_centered(|ui| {
        ui.add_space(15.0);

        // Just sparkles icon
        let icon = RichText::new("✨")
            .size(28.0)
            .color(Color32::WHITE);  // White on black
        ui.label(icon);

        ui.add_space(15.0);
    });
}

fn render_success_state(ui: &mut egui::Ui, _text: &str, _config: &OverlayConfig) {
    // Minimal success indicator
    ui.vertical_centered(|ui| {
        ui.add_space(15.0);

        // Just a checkmark
        let checkmark = RichText::new("✓")
            .size(32.0)
            .color(Color32::WHITE)  // White on black
            .strong();
        ui.label(checkmark);

        ui.add_space(15.0);
    });
}

fn render_error_state(ui: &mut egui::Ui, _message: &str, _config: &OverlayConfig) {
    // Minimal error indicator
    ui.vertical_centered(|ui| {
        ui.add_space(15.0);

        // Just an X or warning icon
        let error_icon = RichText::new("✗")
            .size(32.0)
            .color(Color32::WHITE)  // White on black
            .strong();
        ui.label(error_icon);

        ui.add_space(15.0);
    });
}

