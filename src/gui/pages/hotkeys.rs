//! The keys that start, stop, cancel, and command a dictation. Each one is a
//! button that captures the next key pressed, with a text box behind it for
//! keys the capture cannot see.

use crate::config::settings::HotkeyConfig;
use crate::gui::capture::Capture;
use crate::gui::theme::{
    caution, field, hint, quiet_button, row, section, slider, toggle_row, ACCENT, CONTROL_HEIGHT,
    FIELD_WIDTH, MUTED,
};
use crate::gui::Gui;
use crate::hotkey::warnings;
use crate::hotkey::{HotkeyBackend, HotkeyMode, KeyCombination, Role};

/// Dictation cannot be unbound; every other key can.
fn required(role: Role) -> bool {
    role == Role::Dictate
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

/// The other actions that already use `chord`, as a sentence.
pub fn taken(hotkey: &HotkeyConfig, role: Role, chord: &str) -> Option<String> {
    let holders = warnings::holders(hotkey.chords(), role, chord);
    if holders.is_empty() {
        return None;
    }
    let shown = KeyCombination::parse(chord)
        .map(|chord| chord.to_string())
        .unwrap_or_else(|_| chord.trim().to_string());
    let verb = if holders.len() == 1 { "uses" } else { "use" };
    Some(format!(
        "{} already {} {}.",
        warnings::labels(&holders),
        verb,
        shown
    ))
}

/// Why `chord` cannot go to `role`, when another action uses it.
pub fn refusal(hotkey: &HotkeyConfig, role: Role, chord: &str) -> Option<String> {
    taken(hotkey, role, chord).map(|message| format!("{} Pick another key.", message))
}

/// Amber text under a hotkey field whose key types a character that the
/// focused app also gets.
fn printable_caution(hotkey: &HotkeyConfig, role: Role) -> Option<String> {
    warnings::printable(hotkey.chords(), hotkey.hides_keys())
        .into_iter()
        .find(|(found, _)| *found == role)
        .map(|(_, chord)| {
            format!(
                "{} also reaches the focused app. To hide it, turn on Keep the key from \
                 other applications.",
                chord
            )
        })
}

pub fn show(gui: &mut Gui, ui: &mut egui::Ui) {
    hint(
        ui,
        "Click a key, then press the one you want. A modifier on its own, such as \
         RightAlt, never reaches other applications.",
    );
    if let Some(altgr) = warnings::altgr(gui.config.hotkey.chords(), gui.layout.as_ref()) {
        ui.add_space(4.0);
        caution(
            ui,
            &format!("{}. Use RightCtrl in place of RightAlt.", altgr),
        );
    }
    ui.add_space(4.0);

    key_row(
        gui,
        ui,
        Role::Dictate,
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
        Role::Cancel,
        "Discards the recording in progress. Clear it for none.",
    );
    key_row(
        gui,
        ui,
        Role::Command,
        "Hold, say what to change, release. Rewrites the text you selected, or the \
         last thing Hush typed.",
    );

    section(ui, "More keys");
    key_row(
        gui,
        ui,
        Role::PasteLast,
        "Types the last transcript once more.",
    );
    key_row(
        gui,
        ui,
        Role::Learn,
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

fn key_row(gui: &mut Gui, ui: &mut egui::Ui, role: Role, hint_text: &str) {
    let capturing = gui.capture.as_ref().is_some_and(|(r, _)| *r == role);
    let editing = gui.editing_key.as_ref().is_some_and(|(r, _)| *r == role);
    let value = gui.config.hotkey.chord(role).to_string();
    let error = combination_error(&value, required(role))
        .or_else(|| taken(&gui.config.hotkey, role, &value));
    let notice = printable_caution(&gui.config.hotkey, role);
    row(role.label())
        .describe(hint_text)
        .problem(error.as_deref())
        .caution(notice.as_deref())
        .reserve(KEY_COLUMN)
        .show(ui, |ui| {
            if editing {
                let response = field(ui, gui.config.hotkey.chord_mut(role), "Ctrl+Shift+Space");
                if gui.focus_editor {
                    response.request_focus();
                    gui.focus_editor = false;
                } else if response.lost_focus() {
                    finish_typing(gui);
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
                    start_capture(gui, role);
                }
            }
            if !required(role) && !value.is_empty() && quiet_button(ui, "Clear").clicked() {
                gui.capture = None;
                gui.config.hotkey.chord_mut(role).clear();
            }
            if quiet_button(ui, "Type")
                .on_hover_text("Type the key's name instead")
                .clicked()
            {
                gui.capture = None;
                start_typing(gui, role);
            }
        });
}

fn start_typing(gui: &mut Gui, role: Role) {
    finish_typing(gui);
    gui.editing_key = Some((role, gui.config.hotkey.chord(role).to_string()));
    gui.focus_editor = true;
}

/// Keep the typed chord, unless another action uses it: then put back the
/// chord from before the edit.
fn finish_typing(gui: &mut Gui) {
    let Some((role, before)) = gui.editing_key.take() else {
        return;
    };
    let typed = gui.config.hotkey.chord(role).to_string();
    if let Some(message) = refusal(&gui.config.hotkey, role, &typed) {
        *gui.config.hotkey.chord_mut(role) = before;
        gui.say(message);
    }
}

fn start_capture(gui: &mut Gui, role: Role) {
    if gui.machine.permissions.readable_event_nodes == 0 {
        gui.say("Grant keyboard access on Overview, or type the key's name.");
        start_typing(gui, role);
        return;
    }
    match Capture::start() {
        Ok(capture) => {
            finish_typing(gui);
            gui.capture = Some((role, capture));
        },
        Err(e) => {
            gui.say(format!("Could not read the keyboard: {}", e));
            start_typing(gui, role);
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

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
    fn only_dictation_is_required() {
        assert!(required(Role::Dictate));
        for role in [Role::Cancel, Role::Command, Role::PasteLast, Role::Learn] {
            assert!(!required(role), "{role:?}");
        }
    }

    #[test]
    fn a_chord_another_action_uses_is_refused_by_name() {
        let mut hotkey = Config::defaults().unwrap().hotkey;
        assert_eq!(
            refusal(&hotkey, Role::Learn, "ctrl+rightalt").as_deref(),
            Some("Command already uses Ctrl+RightAlt. Pick another key.")
        );
        assert_eq!(refusal(&hotkey, Role::Command, "Ctrl+RightAlt"), None);
        assert_eq!(refusal(&hotkey, Role::Learn, "Super+RightAlt"), None);
        assert_eq!(refusal(&hotkey, Role::Learn, ""), None);

        hotkey.paste_last = "F12".into();
        hotkey.learn = "F12".into();
        assert_eq!(
            taken(&hotkey, Role::Dictate, "f12").as_deref(),
            Some("Type last again and Learn selection already use F12.")
        );
    }

    #[test]
    fn a_key_that_types_is_flagged_until_it_is_hidden() {
        let mut hotkey = Config::defaults().unwrap().hotkey;
        assert_eq!(printable_caution(&hotkey, Role::Dictate), None);
        hotkey.combination = "Ctrl+Space".into();
        let caution = printable_caution(&hotkey, Role::Dictate).unwrap();
        assert!(caution.starts_with("Ctrl+Space also reaches"), "{caution}");
        assert_eq!(printable_caution(&hotkey, Role::Command), None);
        hotkey.exclusive = true;
        assert_eq!(printable_caution(&hotkey, Role::Dictate), None);
    }
}
