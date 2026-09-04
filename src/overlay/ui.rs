use super::state::{OverlayConfig, OverlayState, OverlayTheme};
use egui::{Color32, Context, Frame, RichText, Sense, Stroke};

// Overlay dimensions
const IDLE_WIDTH: f32 = 60.0;
const IDLE_HEIGHT: f32 = 4.0;
const RECORDING_WIDTH: f32 = 60.0; // Same width as idle for clean expansion
const RECORDING_HEIGHT: f32 = 30.0; // Compact height

// Transition animation
const TRANSITION_SPEED: f32 = 16.0; // Higher = faster transition (snappy response)

// Theme colors - alpha values
const BG_ALPHA_DARK: u8 = 200; // ~78% opacity
const BG_ALPHA_LIGHT: u8 = 200;
const BORDER_ALPHA: u8 = 60; // ~23% opacity

// Waveform animation parameters - refined for smooth, organic look
const WAVEFORM_NUM_BARS: usize = 7;
const WAVEFORM_BAR_SPACING: f32 = 2.5;
const WAVEFORM_BAR_WIDTH: f32 = 3.0;
const WAVEFORM_BASE_HEIGHT: f32 = 3.0;
const WAVEFORM_MAX_HEIGHT: f32 = 24.0; // Compact waveform

// Pre-computed random phase offsets for each bar (breaks uniformity)
const BAR_PHASES: [f32; 7] = [0.0, 0.8, 0.3, 1.1, 0.5, 0.9, 0.2];

/// Render the overlay UI based on current state
pub fn render_overlay(
    ctx: &Context,
    state: &OverlayState,
    config: &OverlayConfig,
) -> OverlayAction {
    let mut action = OverlayAction::None;

    // Configure the style based on theme
    apply_theme(ctx, config.theme);

    // Target dimensions based on state
    let (target_width, target_height) = match state {
        OverlayState::Idle => (IDLE_WIDTH, IDLE_HEIGHT), // Very short pill when inactive
        OverlayState::Recording {
            warning: Some(_), ..
        } => (RECORDING_WIDTH + 44.0, RECORDING_HEIGHT),
        OverlayState::Recording { .. } => (RECORDING_WIDTH, RECORDING_HEIGHT), // Expand taller when recording
        OverlayState::Settings => (220.0, 140.0), // Larger panel for settings
        _ => (RECORDING_WIDTH, RECORDING_HEIGHT), // Other states use recording size
    };

    // Animate dimensions for smooth transitions
    let width = ctx.animate_value_with_time(
        egui::Id::new("overlay_width"),
        target_width,
        1.0 / TRANSITION_SPEED,
    );
    let height = ctx.animate_value_with_time(
        egui::Id::new("overlay_height"),
        target_height,
        1.0 / TRANSITION_SPEED,
    );

    // Calculate transition progress (0.0 = idle, 1.0 = fully expanded)
    let transition_progress = if target_height > IDLE_HEIGHT {
        ((height - IDLE_HEIGHT) / (target_height - IDLE_HEIGHT)).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // Get screen dimensions and calculate base position (for idle state)
    let screen_rect = ctx.screen_rect();
    let (base_x, base_y) = config.get_window_position(screen_rect.width(), screen_rect.height());

    // Adjust position so overlay expands from center point
    // As height increases, move y up by half the difference to keep center stable
    let height_diff = height - IDLE_HEIGHT;
    let adjusted_y = base_y - (height_diff / 2.0);

    // Also center horizontally if width changes
    let width_diff = width - IDLE_WIDTH;
    let adjusted_x = base_x - (width_diff / 2.0);

    // Create a draggable overlay area
    // Use fixed_pos to override saved position and enable center expansion
    egui::Area::new(egui::Id::new("hush_overlay"))
        .fixed_pos([adjusted_x, adjusted_y])
        .movable(false)  // Disable dragging to ensure center expansion works
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
                    OverlayState::Recording { start_time, amplitude, smoothed_amplitude, locked, warning } => {
                        render_recording_state(ui, start_time.elapsed().as_secs_f32(), *amplitude, *smoothed_amplitude, config, transition_progress);
                        render_recording_badges(ui, *locked, warning.as_deref());
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
        });

    action
}

/// Actions that can be triggered by UI interactions
#[derive(Debug, Clone, PartialEq)]
pub enum OverlayAction {
    None,
    StartRecording,
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
        .stroke(Stroke::new(1.0_f32, border_color))  // 1px subtle border
        .rounding(height / 2.0)  // Perfect pill shape (height/2)
        .inner_margin(0.0)  // No padding - keep it tight
        .shadow(egui::epaint::Shadow {
            offset: egui::Vec2::ZERO,
            blur: 8.0,
            spread: 2.0,
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
    _amplitude: f32,
    smoothed_amplitude: f32,
    _config: &OverlayConfig,
    transition_progress: f32,
) {
    // Waveform animation - refined organic movement with multi-harmonic waves
    // Uses smoothed amplitude for natural decay and gaussian bar distribution

    // Pre-calculated constants for performance
    const TOTAL_WIDTH: f32 = (WAVEFORM_NUM_BARS as f32 * WAVEFORM_BAR_WIDTH)
        + ((WAVEFORM_NUM_BARS - 1) as f32 * WAVEFORM_BAR_SPACING);
    const ROUNDING: f32 = WAVEFORM_BAR_WIDTH / 2.0;
    const CENTER_BAR: f32 = (WAVEFORM_NUM_BARS as f32 - 1.0) / 2.0;

    // Get the rect we can draw in
    let available_rect = ui.available_rect_before_wrap();
    let available_width = available_rect.width();

    // Calculate starting position (centered)
    let start_x = available_rect.left() + (available_width - TOTAL_WIDTH) / 2.0;
    let center_y = available_rect.center().y;

    let painter = ui.painter();

    // Use smoothed amplitude for natural animation
    let amp = smoothed_amplitude;

    // Ease-out cubic for smooth deceleration: 1 - (1 - t)^3
    let eased_progress = 1.0 - (1.0 - transition_progress).powi(3);

    // Bar opacity fades in with transition
    let bar_alpha = (eased_progress * 255.0) as u8;

    for (i, &phase) in BAR_PHASES.iter().enumerate().take(WAVEFORM_NUM_BARS) {
        // Stagger bar appearance: center bars appear first, edges last
        let dist_from_center = (i as f32 - CENTER_BAR).abs() / CENTER_BAR;
        let bar_delay = dist_from_center * 0.3; // 0-30% delay based on distance from center
        let bar_progress = ((eased_progress - bar_delay) / (1.0 - bar_delay)).clamp(0.0, 1.0);

        // Multi-harmonic wave animation for organic movement
        // Primary wave (slower, dominant movement)
        let wave1 = (duration * 8.0 + phase).sin();
        // Secondary harmonic (faster, adds complexity)
        let wave2 = (duration * 13.0 + phase * 1.5).sin() * 0.3;
        // Tertiary harmonic (slowest, subtle sway)
        let wave3 = (duration * 3.0 + phase * 0.5).sin() * 0.2;
        // Combine and normalize to 0.3-1.0 range (never fully collapse)
        let time_factor = 0.3 + (wave1 + wave2 + wave3 + 1.5) / 4.3;

        // Gaussian-like bar distribution: center bars are more responsive
        // Edge bars respond at ~70% of center bars
        let bar_weight = 1.0 - 0.3 * dist_from_center;

        // Calculate bar height - boost amplitude effect and ensure bars are tall
        // Use sqrt to make low amplitudes more visible
        // Scale height by bar_progress for grow-in effect
        let boosted_amp = (amp * bar_weight).sqrt().min(1.0);
        let target_height =
            WAVEFORM_BASE_HEIGHT + (boosted_amp * time_factor * WAVEFORM_MAX_HEIGHT);
        let height = WAVEFORM_BASE_HEIGHT + (target_height - WAVEFORM_BASE_HEIGHT) * bar_progress;

        // Calculate bar position (horizontally spaced, vertically centered)
        let x = start_x + (i as f32 * (WAVEFORM_BAR_WIDTH + WAVEFORM_BAR_SPACING));
        let y = center_y - height * 0.5;

        let bar_rect =
            egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(WAVEFORM_BAR_WIDTH, height));

        // Subtle glow effect when amplitude is high (only when fully transitioned)
        if amp > 0.3 && bar_progress > 0.8 {
            let glow_alpha = ((amp - 0.3) * 0.4 * 255.0 * bar_progress) as u8;
            let glow_expand = 1.5;
            let glow_rect = egui::Rect::from_min_size(
                egui::pos2(x - glow_expand / 2.0, y - glow_expand / 2.0),
                egui::vec2(WAVEFORM_BAR_WIDTH + glow_expand, height + glow_expand),
            );
            painter.rect_filled(
                glow_rect,
                ROUNDING + 1.0,
                Color32::from_white_alpha(glow_alpha),
            );
        }

        // Draw the main bar with rounded ends, fading in with transition
        let bar_color = Color32::from_white_alpha(bar_alpha);
        painter.rect_filled(bar_rect, ROUNDING, bar_color);
    }

    // Allocate the space we used
    ui.allocate_rect(available_rect, Sense::hover());
}

/// A dot on the left while locked hands-free, and the time left on the right
/// once the cap is near.
fn render_recording_badges(ui: &mut egui::Ui, locked: bool, warning: Option<&str>) {
    let rect = ui.max_rect();
    let painter = ui.painter();
    if locked {
        painter.circle_filled(
            egui::pos2(rect.left() + 7.0, rect.center().y),
            2.5,
            Color32::from_white_alpha(220),
        );
    }
    if let Some(text) = warning {
        painter.text(
            egui::pos2(rect.right() - 6.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            text,
            egui::FontId::proportional(10.0),
            Color32::from_white_alpha(230),
        );
    }
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

            let hotkey_text = RichText::new(&config.hotkey)
                .size(10.0)
                .color(Color32::WHITE);
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
