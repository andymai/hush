use egui::{Context, Frame, Stroke, Color32, RichText, Sense};
use super::state::{OverlayState, OverlayConfig, OverlayTheme};

/// Render the overlay UI based on current state
pub fn render_overlay(ctx: &Context, state: &OverlayState, config: &OverlayConfig) -> OverlayAction {
    let mut action = OverlayAction::None;

    // Configure the style based on theme
    apply_theme(ctx, config.theme);

    // Create a window that covers the overlay area
    egui::Area::new(egui::Id::new("hush_overlay"))
        .fixed_pos([0.0, 0.0])
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
                    OverlayState::Recording { start_time } => {
                        render_recording_state(ui, start_time.elapsed().as_secs_f32(), config);
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
        OverlayTheme::Dark => Color32::from_rgba_unmultiplied(20, 20, 20, (config.opacity * 255.0) as u8),
        OverlayTheme::Light => Color32::from_rgba_unmultiplied(240, 240, 240, (config.opacity * 255.0) as u8),
    };

    Frame::none()
        .fill(bg_color)
        .stroke(Stroke::new(1.0, Color32::from_gray(60)))
        .rounding(10.0)
        .inner_margin(12.0)
        .shadow(egui::epaint::Shadow {
            extrusion: 4.0,
            color: Color32::from_black_alpha(100),
        })
}

fn render_idle_state(ui: &mut egui::Ui, config: &OverlayConfig) -> OverlayAction {
    let mut action = OverlayAction::None;

    ui.vertical_centered(|ui| {
        ui.add_space(5.0);

        // Microphone icon and title
        let title = RichText::new("🎤 Hush")
            .size(18.0)
            .strong();
        ui.label(title);

        ui.add_space(8.0);

        // Instruction text
        let instruction = RichText::new("Click or use hotkey")
            .size(12.0)
            .color(get_text_color(config.theme, 0.7));
        ui.label(instruction);

        ui.add_space(5.0);

        // Clickable button area
        let button_response = ui.add(
            egui::Label::new(
                RichText::new("Ctrl+Win+V")
                    .size(11.0)
                    .color(get_text_color(config.theme, 0.5))
            )
            .sense(Sense::click())
        );

        if button_response.clicked() {
            action = OverlayAction::StartRecording;
        }

        ui.add_space(5.0);
    });

    action
}

fn render_recording_state(ui: &mut egui::Ui, duration: f32, config: &OverlayConfig) {
    ui.vertical_centered(|ui| {
        ui.add_space(8.0);

        // Recording indicator
        let recording_text = RichText::new("🔴 Recording")
            .size(16.0)
            .color(Color32::from_rgb(220, 50, 50))
            .strong();
        ui.label(recording_text);

        ui.add_space(10.0);

        // Simple progress bar representation (we'll enhance this later)
        let progress = (duration * 2.0).sin().abs(); // Pulsing effect
        ui.add(egui::ProgressBar::new(progress).show_percentage());

        ui.add_space(8.0);

        // Duration
        let duration_text = RichText::new(format!("{:.1}s", duration))
            .size(14.0)
            .color(get_text_color(config.theme, 0.8));
        ui.label(duration_text);

        ui.add_space(5.0);

        // Instruction
        let instruction = RichText::new("Release to transcribe")
            .size(11.0)
            .color(get_text_color(config.theme, 0.6));
        ui.label(instruction);

        ui.add_space(5.0);
    });
}

fn render_processing_state(ui: &mut egui::Ui, message: &str, config: &OverlayConfig) {
    ui.vertical_centered(|ui| {
        ui.add_space(15.0);

        // Spinner icon (we'll use a simple rotating character)
        let spinner = RichText::new("⟳")
            .size(24.0)
            .color(get_accent_color(config.theme));
        ui.label(spinner);

        ui.add_space(10.0);

        // Processing message
        let msg = RichText::new(message)
            .size(14.0)
            .color(get_text_color(config.theme, 0.9));
        ui.label(msg);

        ui.add_space(15.0);
    });
}

fn render_editing_state(ui: &mut egui::Ui, message: &str, config: &OverlayConfig) {
    ui.vertical_centered(|ui| {
        ui.add_space(15.0);

        // Editing icon (sparkles for AI polishing)
        let icon = RichText::new("✨")
            .size(24.0)
            .color(Color32::from_rgb(255, 200, 100));
        ui.label(icon);

        ui.add_space(10.0);

        // Editing message
        let msg = RichText::new(message)
            .size(14.0)
            .color(get_text_color(config.theme, 0.9));
        ui.label(msg);

        ui.add_space(5.0);

        // Subtitle
        let subtitle = RichText::new("AI Polishing...")
            .size(11.0)
            .color(get_text_color(config.theme, 0.6))
            .italics();
        ui.label(subtitle);

        ui.add_space(15.0);
    });
}

fn render_success_state(ui: &mut egui::Ui, text: &str, config: &OverlayConfig) {
    ui.vertical_centered(|ui| {
        ui.add_space(10.0);

        // Success checkmark
        let checkmark = RichText::new("✓")
            .size(28.0)
            .color(Color32::from_rgb(50, 200, 50))
            .strong();
        ui.label(checkmark);

        ui.add_space(8.0);

        // Success message
        let success_text = RichText::new("Text Inserted")
            .size(14.0)
            .color(get_text_color(config.theme, 0.9));
        ui.label(success_text);

        ui.add_space(5.0);

        // Show preview of inserted text (truncated)
        let preview = if text.len() > 40 {
            format!("\"{}...\"", &text[..40])
        } else {
            format!("\"{}\"", text)
        };

        let preview_text = RichText::new(preview)
            .size(11.0)
            .color(get_text_color(config.theme, 0.6))
            .italics();
        ui.label(preview_text);

        ui.add_space(10.0);
    });
}

fn render_error_state(ui: &mut egui::Ui, message: &str, config: &OverlayConfig) {
    ui.vertical_centered(|ui| {
        ui.add_space(10.0);

        // Error icon
        let error_icon = RichText::new("⚠")
            .size(28.0)
            .color(Color32::from_rgb(220, 50, 50))
            .strong();
        ui.label(error_icon);

        ui.add_space(8.0);

        // Error title
        let error_title = RichText::new("Error")
            .size(14.0)
            .color(Color32::from_rgb(220, 50, 50))
            .strong();
        ui.label(error_title);

        ui.add_space(5.0);

        // Error message
        let error_text = RichText::new(message)
            .size(11.0)
            .color(get_text_color(config.theme, 0.8));
        ui.label(error_text);

        ui.add_space(10.0);
    });
}

fn get_text_color(theme: OverlayTheme, alpha: f32) -> Color32 {
    match theme {
        OverlayTheme::Dark => Color32::from_rgba_unmultiplied(
            255,
            255,
            255,
            (alpha * 255.0) as u8,
        ),
        OverlayTheme::Light => Color32::from_rgba_unmultiplied(
            20,
            20,
            20,
            (alpha * 255.0) as u8,
        ),
    }
}

fn get_accent_color(theme: OverlayTheme) -> Color32 {
    match theme {
        OverlayTheme::Dark => Color32::from_rgb(100, 150, 255),
        OverlayTheme::Light => Color32::from_rgb(50, 100, 200),
    }
}
