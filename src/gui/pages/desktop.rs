//! The panel icon, the sounds, the media players, the update check, the
//! window's colours, and the file behind every page.

use crate::config::settings::Appearance;
use crate::gui::icons;
use crate::gui::widgets::{row, section, segmented, switch_row, Button};
use crate::gui::Gui;
use std::path::Path;

pub fn show(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "General");
    switch_row(
        ui,
        "desktop.tray",
        "Show the panel icon",
        "Click it to start or stop dictation. GNOME needs the AppIndicator extension.",
        &mut gui.config.tray.enabled,
    );
    switch_row(
        ui,
        "desktop.sounds",
        "Play a sound when recording starts and stops",
        "A short tone on start, stop, cancel, and the warning before a cap.",
        &mut gui.config.feedback.audio_enabled,
    );
    switch_row(
        ui,
        "desktop.pause_media",
        "Pause music and video while you record",
        "Pauses the players that play, then resumes them when the recording stops. The players must support MPRIS, as most do.",
        &mut gui.config.audio.pause_media,
    );
    let title = "Check for new releases";
    if crate::updates::InstallKind::current().checks() {
        switch_row(
            ui,
            "desktop.updates",
            title,
            "Once a day, ask GitHub for the newest release and show a notification when it is newer. The request sends only the Hush version.",
            &mut gui.config.updates.check,
        );
    } else {
        ui.add_enabled_ui(false, |ui| {
            let mut off = false;
            switch_row(
                ui,
                "desktop.updates",
                title,
                "Your package manager handles updates.",
                &mut off,
            );
        });
    }

    section(ui, "Appearance");
    row("desktop.appearance", "Theme")
        .describe("System follows the light or dark preference of your desktop.")
        .reserve(190.0)
        .show(ui, |ui| {
            let options = Appearance::ALL.map(|appearance| (appearance, appearance.name()));
            segmented(ui, &mut gui.config.gui.appearance, &options);
        });

    section(ui, "Configuration file");
    let path = crate::config::paths::config_path();
    let shown = home_relative(&path, dirs::home_dir().as_deref());
    row("desktop.config_file", "Every page writes to this file")
        .value(&shown)
        .reserve(150.0)
        .show(ui, |ui| {
            if ui
                .add(Button::new("Open the file").icon(icons::FILE_TEXT))
                .clicked()
            {
                if !path.exists() {
                    let _ = gui.config.save_to_file(&path);
                }
                if let Err(e) = std::process::Command::new("xdg-open").arg(&path).spawn() {
                    gui.say(format!("Could not open it: {}", e));
                }
            }
        });
    row("desktop.reset", "Reset everything")
        .describe("Puts the built-in defaults back on every page. Save makes it permanent.")
        .reserve(100.0)
        .show(ui, |ui| {
            if ui
                .add(Button::new("Reset").icon(icons::ROTATE_CCW))
                .clicked()
            {
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

/// The path with the home folder written as `~`.
fn home_relative(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_in_the_home_folder_starts_with_a_tilde() {
        let home = Path::new("/home/me");
        assert_eq!(
            home_relative(Path::new("/home/me/.config/hush/config.toml"), Some(home)),
            "~/.config/hush/config.toml"
        );
        assert_eq!(
            home_relative(Path::new("/etc/hush.toml"), Some(home)),
            "/etc/hush.toml"
        );
        assert_eq!(
            home_relative(Path::new("/home/me2/config.toml"), Some(home)),
            "/home/me2/config.toml",
            "a folder whose name only starts the same"
        );
        assert_eq!(home_relative(Path::new("/x"), None), "/x");
    }
}
