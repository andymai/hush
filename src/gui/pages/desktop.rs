//! The panel icon, the sounds, and the file behind every page.

use crate::gui::theme::{row, section, toggle_row};
use crate::gui::Gui;

pub fn show(gui: &mut Gui, ui: &mut egui::Ui) {
    ui.add_space(6.0);
    toggle_row(
        ui,
        "Show the panel icon",
        "Click it to start or stop dictation. GNOME needs the AppIndicator extension.",
        &mut gui.config.tray.enabled,
    );
    toggle_row(
        ui,
        "Play a sound when recording starts and stops",
        "A short tone on start, stop, cancel, and the warning before a cap.",
        &mut gui.config.feedback.audio_enabled,
    );

    section(ui, "Configuration file");
    let path = crate::config::paths::config_path();
    let shown = path.display().to_string();
    row("Every page writes to this file")
        .describe(&shown)
        .show(ui, |ui| {
            if ui.button("Open the file").clicked() {
                if !path.exists() {
                    let _ = gui.config.save_to_file(&path);
                }
                if let Err(e) = std::process::Command::new("xdg-open").arg(&path).spawn() {
                    gui.say(format!("Could not open it: {}", e));
                }
            }
        });
    row("Reset everything")
        .describe("Puts the built-in defaults back on every page. Save makes it permanent.")
        .show(ui, |ui| {
            if ui.button("Reset").clicked() {
                match crate::config::Config::defaults() {
                    Ok(defaults) => {
                        gui.config = defaults;
                        gui.say("Reset. Save to keep it.");
                    },
                    Err(e) => gui.say(format!("Could not reset: {}", e)),
                }
            }
        });
}
