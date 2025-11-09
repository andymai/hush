/// Minimal example showing just the overlay without Hush dependencies
///
/// Run with: cargo run --example simple_overlay --features notifications
///
/// This example only uses egui_overlay and doesn't depend on audio/whisper libs

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    println!("🎤 Hush Overlay Test");
    println!("Creating overlay window...");

    // We'll use egui_overlay directly for this simple test
    use egui_overlay::{EguiOverlay, egui_window_glfw_passthrough::glfw};

    let mut overlay = EguiOverlay::new();

    // Configure window
    overlay.window_mut().glfw.window_hint(glfw::WindowHint::Decorated(false));
    overlay.window_mut().glfw.window_hint(glfw::WindowHint::TransparentFramebuffer(true));
    overlay.window_mut().glfw.window_hint(glfw::WindowHint::Floating(true));
    overlay.window_mut().glfw.window_hint(glfw::WindowHint::Resizable(false));

    // Set size and position (bottom-right corner)
    overlay.window_mut().set_size(300, 100);

    // Get screen size
    let (screen_width, screen_height) = overlay.window_mut().glfw
        .with_primary_monitor(|_, monitor| {
            monitor.map(|m| {
                let (_, _, w, h) = m.get_workarea();
                (w, h)
            })
        })
        .flatten()
        .unwrap_or((1920, 1080));

    // Position at bottom-right
    let pos_x = screen_width - 300 - 20;
    let pos_y = screen_height - 100 - 20;
    overlay.window_mut().set_pos(pos_x, pos_y);

    println!("Window positioned at ({}, {})", pos_x, pos_y);
    println!("Screen size: {}x{}", screen_width, screen_height);

    // State for cycling through UI states
    let start_time = Instant::now();

    overlay.run(move |ctx| {
        let elapsed = start_time.elapsed().as_secs();

        egui::Area::new(egui::Id::new("hush_test"))
            .fixed_pos([0.0, 0.0])
            .show(ctx, |ui| {
                let frame = egui::Frame::none()
                    .fill(egui::Color32::from_rgba_unmultiplied(20, 20, 20, 240))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(60)))
                    .rounding(10.0)
                    .inner_margin(12.0)
                    .shadow(egui::epaint::Shadow {
                        offset: egui::Vec2::new(0.0, 4.0),
                        blur: 8.0,
                        spread: 0.0,
                        color: egui::Color32::from_black_alpha(100),
                    });

                frame.show(ui, |ui| {
                    ui.set_width(276.0);
                    ui.set_height(76.0);

                    ui.vertical_centered(|ui| {
                        // Cycle through different states every 3 seconds
                        let state = (elapsed / 3) % 4;

                        match state {
                            0 => {
                                // Idle
                                ui.add_space(5.0);
                                ui.label(egui::RichText::new("🎤 Hush").size(18.0).strong());
                                ui.add_space(8.0);
                                ui.label(egui::RichText::new("Click or use hotkey").size(12.0));
                                ui.add_space(5.0);
                                ui.label(egui::RichText::new("Ctrl+Win+V").size(11.0).color(egui::Color32::from_gray(150)));
                            }
                            1 => {
                                // Recording
                                ui.add_space(8.0);
                                ui.label(egui::RichText::new("🔴 Recording").size(16.0).color(egui::Color32::from_rgb(220, 50, 50)).strong());
                                ui.add_space(10.0);
                                ui.add(egui::ProgressBar::new(0.6).show_percentage());
                                ui.add_space(8.0);
                                ui.label(egui::RichText::new(format!("{:.1}s", elapsed % 3)).size(14.0));
                            }
                            2 => {
                                // Processing
                                ui.add_space(15.0);
                                ui.label(egui::RichText::new("⟳").size(24.0).color(egui::Color32::from_rgb(100, 150, 255)));
                                ui.add_space(10.0);
                                ui.label(egui::RichText::new("Transcribing...").size(14.0));
                            }
                            3 => {
                                // Success
                                ui.add_space(10.0);
                                ui.label(egui::RichText::new("✓").size(28.0).color(egui::Color32::from_rgb(50, 200, 50)).strong());
                                ui.add_space(8.0);
                                ui.label(egui::RichText::new("Text Inserted").size(14.0));
                                ui.add_space(5.0);
                                ui.label(egui::RichText::new("\"Hello world\"").size(11.0).italics());
                            }
                            _ => {}
                        }

                        ui.add_space(5.0);
                    });
                });
            });

        // Request repaint for animations
        ctx.request_repaint_after(Duration::from_millis(100));
    }).expect("Failed to run overlay");
}
