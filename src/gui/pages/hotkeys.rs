//! The keys that start, stop, cancel, and command a dictation. Each one is a
//! button that captures the next key pressed, with a text box behind it for
//! keys the capture cannot see.

use crate::config::Config;
use crate::gui::capture::Capture;
use crate::gui::theme::{
    field, hint, quiet_button, row, section, slider, toggle_row, ACCENT, CONTROL_HEIGHT,
    FIELD_WIDTH, MUTED,
};
use crate::gui::Gui;
use crate::hotkey::{HotkeyBackend, HotkeyMode, KeyCombination};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyField {
    Dictate,
    Cancel,
    Command,
    PasteLast,
    Learn,
}

impl KeyField {
    pub fn value(self, config: &mut Config) -> &mut String {
        match self {
            KeyField::Dictate => &mut config.hotkey.combination,
            KeyField::Cancel => &mut config.hotkey.cancel,
            KeyField::Command => &mut config.hotkey.command,
            KeyField::PasteLast => &mut config.hotkey.paste_last,
            KeyField::Learn => &mut config.hotkey.learn,
        }
    }

    /// Dictation cannot be unbound; every other key can.
    fn required(self) -> bool {
        self == KeyField::Dictate
    }
}

/// Width of the key button plus its Type and Clear actions.
const KEY_COLUMN: f32 = 320.0;

/// Red text under a hotkey field that does not parse.
pub fn combination_error(text: &str, required: bool) -> Option<String> {
    if text.trim().is_empty() {
        return required.then(|| "Hush needs a key to dictate with.".to_string());
    }
    KeyCombination::parse(text).err().map(|e| e.to_string())
}

pub fn show(gui: &mut Gui, ui: &mut egui::Ui) {
    hint(
        ui,
        "Click a key, then press the one you want. A modifier on its own, such as \
         RightAlt, never reaches other applications.",
    );
    ui.add_space(4.0);

    key_row(
        gui,
        ui,
        KeyField::Dictate,
        "Dictate",
        match gui.config.hotkey.mode {
            HotkeyMode::Hold => {
                "Hold to talk. Double-tap to keep recording hands-free until you tap again."
            },
            HotkeyMode::Toggle => "Tap to start, tap again to stop.",
        },
    );
    row("Mode")
        .describe("Hold to talk, or tap to start and tap to stop.")
        .show(ui, |ui| {
            egui::ComboBox::from_id_salt("hotkey_mode")
                .width(FIELD_WIDTH)
                .selected_text(mode_name(gui.config.hotkey.mode))
                .show_ui(ui, |ui| {
                    for mode in [HotkeyMode::Hold, HotkeyMode::Toggle] {
                        ui.selectable_value(&mut gui.config.hotkey.mode, mode, mode_name(mode));
                    }
                });
        });
    key_row(
        gui,
        ui,
        KeyField::Cancel,
        "Cancel",
        "Discards the recording in progress. Clear it for none.",
    );
    key_row(
        gui,
        ui,
        KeyField::Command,
        "Command",
        "Hold, say what to change, release. Rewrites the text you selected, or the \
         last thing Hush typed.",
    );

    section(ui, "More keys");
    key_row(
        gui,
        ui,
        KeyField::PasteLast,
        "Type last again",
        "Types the last transcript once more.",
    );
    key_row(
        gui,
        ui,
        KeyField::Learn,
        "Learn selection",
        "Adds the selected word to the vocabulary with its exact spelling.",
    );
    row("Read keys through")
        .describe("The input devices work on Wayland and X11 alike. X11 is the fallback.")
        .show(ui, |ui| {
            egui::ComboBox::from_id_salt("hotkey_backend")
                .width(FIELD_WIDTH)
                .selected_text(backend_name(gui.config.hotkey.backend))
                .show_ui(ui, |ui| {
                    for backend in [
                        HotkeyBackend::Auto,
                        HotkeyBackend::Evdev,
                        HotkeyBackend::X11,
                    ] {
                        ui.selectable_value(
                            &mut gui.config.hotkey.backend,
                            backend,
                            backend_name(backend),
                        );
                    }
                });
        });
    row("Tap is shorter than")
        .describe("A shorter press is a tap: taps lock and unlock, holds record.")
        .show(ui, |ui| {
            slider(
                ui,
                egui::Slider::new(&mut gui.config.hotkey.tap_ms, 100..=1000).suffix(" ms"),
            );
        });
    toggle_row(
        ui,
        "Keep the key from other applications",
        "Grabs the device the key belongs to. Needed for mouse buttons, which \
         browsers otherwise treat as Back.",
        &mut gui.config.hotkey.exclusive,
    );
}

fn mode_name(mode: HotkeyMode) -> &'static str {
    match mode {
        HotkeyMode::Hold => "Hold to talk",
        HotkeyMode::Toggle => "Tap to start and stop",
    }
}

fn backend_name(backend: HotkeyBackend) -> &'static str {
    match backend {
        HotkeyBackend::Auto => "Whatever works",
        HotkeyBackend::Evdev => "The input devices",
        HotkeyBackend::X11 => "X11",
    }
}

fn key_row(gui: &mut Gui, ui: &mut egui::Ui, field_id: KeyField, label: &str, hint_text: &str) {
    let capturing = gui.capture.as_ref().is_some_and(|(f, _)| *f == field_id);
    let editing = gui.editing_key == Some(field_id);
    let value = field_id.value(&mut gui.config).clone();
    let error = combination_error(&value, field_id.required());
    row(label)
        .describe(hint_text)
        .problem(error.as_deref())
        .reserve(KEY_COLUMN)
        .show(ui, |ui| {
            if editing {
                let response = field(ui, field_id.value(&mut gui.config), "Ctrl+Shift+Space");
                if gui.focus_editor {
                    response.request_focus();
                    gui.focus_editor = false;
                } else if response.lost_focus() {
                    gui.editing_key = None;
                }
                return;
            }

            let text = if capturing {
                egui::RichText::new("Press a key or button…").color(ACCENT)
            } else if value.is_empty() {
                egui::RichText::new("Not set").color(MUTED)
            } else {
                egui::RichText::new(&value).strong()
            };
            let mut button =
                egui::Button::new(text).min_size(egui::vec2(FIELD_WIDTH, CONTROL_HEIGHT));
            if capturing {
                button = button.stroke(egui::Stroke::new(1.5_f32, ACCENT));
            }
            if ui
                .add(button)
                .on_hover_text("Click, then press the key you want")
                .clicked()
            {
                if capturing {
                    gui.capture = None;
                } else {
                    start_capture(gui, field_id);
                }
            }
            if !field_id.required() && !value.is_empty() && quiet_button(ui, "Clear").clicked() {
                gui.capture = None;
                field_id.value(&mut gui.config).clear();
            }
            if quiet_button(ui, "Type")
                .on_hover_text("Type the key's name instead")
                .clicked()
            {
                gui.capture = None;
                gui.editing_key = Some(field_id);
                gui.focus_editor = true;
            }
        });
}

fn start_capture(gui: &mut Gui, field: KeyField) {
    if gui.machine.permissions.readable_event_nodes == 0 {
        gui.say("Grant keyboard access on Overview, or type the key's name.");
        gui.editing_key = Some(field);
        gui.focus_editor = true;
        return;
    }
    match Capture::start() {
        Ok(capture) => {
            gui.editing_key = None;
            gui.capture = Some((field, capture));
        },
        Err(e) => {
            gui.say(format!("Could not read the keyboard: {}", e));
            gui.editing_key = Some(field);
            gui.focus_editor = true;
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_unparseable_or_missing_required_keys_are_flagged() {
        assert!(combination_error("RightAlt", true).is_none());
        assert!(
            combination_error("", false).is_none(),
            "empty means unbound"
        );
        assert!(combination_error("  ", false).is_none());
        assert!(
            combination_error("", true).is_some(),
            "dictation needs a key"
        );
        assert!(combination_error("Ctrl+Nope", false).is_some());
    }

    #[test]
    fn every_field_maps_to_its_setting() {
        let mut config = Config::defaults().unwrap();
        *KeyField::Learn.value(&mut config) = "Super+RightAlt".to_string();
        assert_eq!(config.hotkey.learn, "Super+RightAlt");
        assert_eq!(KeyField::Dictate.value(&mut config), "RightAlt");
        assert!(KeyField::Dictate.required() && !KeyField::Cancel.required());
    }
}
