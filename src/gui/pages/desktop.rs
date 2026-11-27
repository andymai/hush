//! The panel icon, the sounds, the media players, the update check, the
//! recording pill, the window's colours, and the file behind every page.

use crate::config::settings::{Appearance, OverlayTheme};
use crate::config::Config;
use crate::gui::autosave::Pending;
use crate::gui::icons;
use crate::gui::widgets::{dropdown, option, row, section, segmented, switch_row, Button, Variant};
use crate::gui::Gui;
use std::path::Path;

pub fn show(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "General");
    switch_row(
        ui,
        "desktop.tray",
        "Show the panel icon",
        "Click it to start or stop dictating. On GNOME, you'll need the AppIndicator \
         extension.",
        &mut gui.config.tray.enabled,
    );
    switch_row(
        ui,
        "desktop.sounds",
        "Play a sound when recording starts and stops",
        "A short tone when you start, stop, or cancel, and before a long recording times \
         out.",
        &mut gui.config.feedback.audio_enabled,
    );
    switch_row(
        ui,
        "desktop.pause_media",
        "Pause music and video while you record",
        "Pauses whatever's playing and picks it back up when you're done. Works with most \
         players.",
        &mut gui.config.audio.pause_media,
    );
    let title = "Check for new releases";
    if crate::updates::InstallKind::current().checks() {
        switch_row(
            ui,
            "desktop.updates",
            title,
            "Checks GitHub once a day and lets you know when there's a new version. It \
             only sends which version you have.",
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

    section(ui, "Recording pill");
    switch_row(
        ui,
        "desktop.overlay",
        "Show the recording pill",
        "A small pill at the bottom of the screen shows that you're recording. Click it \
         to stop, or right-click to throw the recording away.",
        &mut gui.config.overlay.enabled,
    );
    let theme = gui.config.overlay.theme;
    row("desktop.overlay_theme", "Pill colors")
        .describe(overlay_theme_note(theme))
        .reserve(190.0)
        .show(ui, |ui| {
            let current = &mut gui.config.overlay.theme;
            dropdown(ui, "overlay_theme", 190.0, theme.name(), |ui| {
                for choice in OverlayTheme::ALL {
                    option(ui, current, choice, choice.name());
                }
            });
        });

    section(ui, "Appearance");
    row("desktop.appearance", "Window theme")
        .describe("For this window. System matches your desktop's light or dark setting.")
        .reserve(190.0)
        .show(ui, |ui| {
            let options = Appearance::ALL.map(|appearance| (appearance, appearance.name()));
            segmented(ui, &mut gui.config.gui.appearance, &options);
        });

    section(ui, "Configuration file");
    let path = crate::config::paths::config_path();
    let shown = home_relative(&path, dirs::home_dir().as_deref());
    row("desktop.config_file", "All your settings live in this file")
        .value(&shown)
        .reserve(150.0)
        .show(ui, |ui| {
            if ui
                .add(Button::new("Open the file").icon(icons::FILE_TEXT))
                .clicked()
            {
                open_config_file(gui);
            }
        });
    let confirming = gui.confirm_reset;
    row("desktop.reset", "Reset everything")
        .describe("Puts every setting back to its default. Your history and writing styles stay.")
        .caution(confirming.then_some(
            "This resets your hotkeys and every speech, text, output, and desktop \
             setting. Your history settings, writing styles, vocabulary, transcripts, and \
             saved audio stay as they are.",
        ))
        .reserve(if confirming { 230.0 } else { 100.0 })
        .show(ui, |ui| {
            if !confirming {
                if ui
                    .add(Button::new("Reset").icon(icons::ROTATE_CCW))
                    .clicked()
                {
                    gui.confirm_reset = true;
                }
                return;
            }
            let button = Button::new("Reset everything")
                .icon(icons::ROTATE_CCW)
                .variant(Variant::Danger);
            if ui.add(button).clicked() {
                gui.confirm_reset = false;
                match reset_settings(&gui.config) {
                    Ok(defaults) => {
                        gui.config = defaults;
                        gui.pending = Pending::default();
                        gui.say("Your settings are back to their defaults.");
                    },
                    Err(e) => gui.say(format!("Couldn't reset: {}", e)),
                }
            }
            if ui.add(Button::new("Keep them").focus_on_jump()).clicked() {
                gui.confirm_reset = false;
            }
        });
}

/// Open the configuration file in the desktop's editor, and write it first
/// when it does not exist.
pub fn open_config_file(gui: &mut Gui) {
    let path = crate::config::paths::config_path();
    if !path.exists() {
        let _ = gui.config.save_to_file(&path);
    }
    if let Err(e) = std::process::Command::new("xdg-open").arg(&path).spawn() {
        gui.say(format!("Couldn't open it: {}", e));
    }
}

/// The built-in defaults, with the history settings and the writing styles
/// of `current`, so a reset deletes no kept audio and no transcripts.
fn overlay_theme_note(theme: OverlayTheme) -> &'static str {
    match theme {
        OverlayTheme::System => {
            "System matches your desktop's light or dark setting, accent color, and \
             contrast."
        },
        OverlayTheme::File => {
            "Uses overlay-theme.toml next to your config file, and picks up your changes \
             as you save."
        },
        _ => "If you've turned on reduced motion, the level shows as one still bar.",
    }
}

fn reset_settings(current: &Config) -> anyhow::Result<Config> {
    let mut defaults = Config::defaults()?;
    defaults.history = current.history.clone();
    defaults.styles = current.styles.clone();
    defaults.validate()?;
    Ok(defaults)
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
    fn a_reset_keeps_the_history_settings_and_the_styles() {
        let mut current = Config::defaults().unwrap();
        current.history.keep_audio_days = 30;
        current.history.max_entries = 2000;
        current.styles.insert(
            "casual".into(),
            crate::text_processing::StyleConfig {
                prompt: "Brief.".into(),
                polish: true,
            },
        );
        current.tray.enabled = false;
        current.hotkey.combination = "F13".into();
        let reset = reset_settings(&current).unwrap();
        assert_eq!(reset.history, current.history, "no audio or transcripts go");
        assert_eq!(reset.styles, current.styles);
        assert!(reset.tray.enabled);
        assert_eq!(reset.hotkey.combination, "RightAlt");
    }

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
