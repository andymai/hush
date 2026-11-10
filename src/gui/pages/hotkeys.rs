//! The keys that start, stop, cancel, and command a dictation, and a key
//! for each language. Each one is a button that captures the next key
//! pressed; the action keys have a text box behind it for keys the capture
//! cannot see.

use crate::config::settings::HotkeyConfig;
use crate::gui::capture::Capture;
use crate::gui::pages::speech::language_picker;
use crate::gui::theme::{
    caution, field, hint, quiet_button, row, section, slider, toggle_row, ACCENT, CONTROL_HEIGHT,
    FIELD_WIDTH, MUTED,
};
use crate::gui::Gui;
use crate::hotkey::warnings;
use crate::hotkey::{HotkeyBackend, HotkeyMode, KeyCombination, Role};
use crate::transcription::Language;

/// What a captured key goes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyTarget {
    Action(Role),
    /// A language chord, by the chord it has now; `None` adds a chord for
    /// the language in [`Gui::new_language`].
    Language(Option<String>),
}

/// Dictation cannot be unbound; every other key can.
fn required(role: Role) -> bool {
    role == Role::Dictate
}

/// Width of the key button plus its Type and Clear actions.
const KEY_COLUMN: f32 = 320.0;

/// Width of the key button, the language picker, and Remove.
const LANGUAGE_COLUMN: f32 = 420.0;

/// Width of the language picker beside a key.
const PICKER_WIDTH: f32 = 130.0;

/// Red text under a hotkey field that does not parse.
pub fn combination_error(text: &str, required: bool) -> Option<String> {
    if text.trim().is_empty() {
        return required.then(|| "Hush needs a key to dictate with.".to_string());
    }
    KeyCombination::parse(text).err().map(|e| e.to_string())
}

/// The other actions that already use `chord`, as a sentence.
pub fn taken(hotkey: &HotkeyConfig, role: Role, chord: &str) -> Option<String> {
    sentence(warnings::holders(hotkey.bound(), role, chord), chord)
}

/// The actions and language chords, other than the language chord `own`,
/// that already use `chord`, as a sentence.
pub fn language_taken(hotkey: &HotkeyConfig, own: Option<&str>, chord: &str) -> Option<String> {
    let others = hotkey
        .bound()
        .into_iter()
        .filter(|(role, text)| !(matches!(role, Role::Language(_)) && Some(*text) == own));
    sentence(warnings::users(others, chord), chord)
}

fn sentence(holders: Vec<Role>, chord: &str) -> Option<String> {
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
        .map(|(_, chord)| reaches_the_app(&chord))
}

/// [`printable_caution`] for the language chord `chord`.
fn language_caution(hotkey: &HotkeyConfig, chord: &str) -> Option<String> {
    let own = hotkey
        .language_keys()
        .into_iter()
        .filter(|(_, text)| *text == chord)
        .map(|(language, text)| (Role::Language(language), text));
    warnings::printable(own, hotkey.hides_keys())
        .first()
        .map(|(_, chord)| reaches_the_app(chord))
}

fn reaches_the_app(chord: &KeyCombination) -> String {
    format!(
        "{} also reaches the focused app. To hide it, turn on Keep the key from \
         other applications.",
        chord
    )
}

pub fn show(gui: &mut Gui, ui: &mut egui::Ui) {
    hint(
        ui,
        "Click a key, then press the one you want. A modifier on its own, such as \
         RightAlt, never reaches other applications.",
    );
    if let Some(altgr) = warnings::altgr(gui.config.hotkey.bound(), gui.layout.as_ref()) {
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
    language_keys(gui, ui);
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

fn language_keys(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Language keys");
    hint(
        ui,
        "Each key dictates in its own language, with the same hold, double-tap, and \
         cancel as the dictation key. The tray menu sets the language of the dictation key.",
    );
    let chords: Vec<String> = gui.config.hotkey.languages.keys().cloned().collect();
    for chord in chords {
        language_row(gui, ui, &chord);
    }
    new_language_row(gui, ui);
}

fn language_row(gui: &mut Gui, ui: &mut egui::Ui, chord: &str) {
    let target = KeyTarget::Language(Some(chord.to_string()));
    let capturing = gui.capture.as_ref().is_some_and(|(t, _)| *t == target);
    let code = gui
        .config
        .hotkey
        .languages
        .get(chord)
        .cloned()
        .unwrap_or_default();
    let title = match Language::parse(&code) {
        Some(language) => Role::Language(language).label(),
        None => format!("Dictate in {}", code),
    };
    let error = combination_error(chord, true)
        .or_else(|| language_taken(&gui.config.hotkey, Some(chord), chord));
    let notice = language_caution(&gui.config.hotkey, chord);
    row(&title)
        .problem(error.as_deref())
        .caution(notice.as_deref())
        .reserve(LANGUAGE_COLUMN)
        .show(ui, |ui| {
            let button = capture_button(capturing, egui::RichText::new(chord).strong());
            if ui
                .add(button)
                .on_hover_text("Click, then press the key you want")
                .clicked()
            {
                toggle_capture(gui, target);
            }
            if let Some(code) = gui.config.hotkey.languages.get_mut(chord) {
                language_picker(ui, ("language_key", chord), code, true, PICKER_WIDTH, "");
            }
            if quiet_button(ui, "Remove").clicked() {
                gui.capture = None;
                gui.config.hotkey.languages.remove(chord);
            }
        });
}

fn new_language_row(gui: &mut Gui, ui: &mut egui::Ui) {
    let target = KeyTarget::Language(None);
    let capturing = gui.capture.as_ref().is_some_and(|(t, _)| *t == target);
    row("Add a language key")
        .describe("Pick a language, then click the button and press the key.")
        .reserve(LANGUAGE_COLUMN)
        .show(ui, |ui| {
            let button = capture_button(capturing, egui::RichText::new("Choose the key"));
            let ready = !gui.new_language.is_empty();
            if ui.add_enabled(ready, button).clicked() {
                toggle_capture(gui, target);
            }
            language_picker(
                ui,
                "new_language_key",
                &mut gui.new_language,
                true,
                PICKER_WIDTH,
                "Language",
            );
        });
}

/// The button that captures a key: `idle` until it is clicked, then a
/// prompt to press the key.
fn capture_button(capturing: bool, idle: egui::RichText) -> egui::Button<'static> {
    if capturing {
        egui::Button::new(egui::RichText::new("Press a key or button…").color(ACCENT))
            .min_size(egui::vec2(FIELD_WIDTH, CONTROL_HEIGHT))
            .stroke(egui::Stroke::new(1.5_f32, ACCENT))
    } else {
        egui::Button::new(idle).min_size(egui::vec2(FIELD_WIDTH, CONTROL_HEIGHT))
    }
}

fn toggle_capture(gui: &mut Gui, target: KeyTarget) {
    if gui.capture.as_ref().is_some_and(|(t, _)| *t == target) {
        gui.capture = None;
        return;
    }
    if gui.machine.permissions.readable_event_nodes == 0 {
        gui.say("Grant keyboard access on Overview, or add the key under [hotkey] in config.toml.");
        return;
    }
    match Capture::start() {
        Ok(capture) => {
            finish_typing(gui);
            gui.capture = Some((target, capture));
        },
        Err(e) => gui.say(format!("Could not read the keyboard: {}", e)),
    }
}

/// Give the captured `chord` to a language: move the language chord `own`
/// to it, or add a chord for [`Gui::new_language`]. Another action's chord
/// is refused.
pub fn bind_language(gui: &mut Gui, own: Option<String>, chord: String) {
    if let Some(message) = language_taken(&gui.config.hotkey, own.as_deref(), &chord) {
        gui.say(format!("{} Pick another key.", message));
        return;
    }
    let languages = &mut gui.config.hotkey.languages;
    let code = match own {
        Some(own) => languages.remove(&own),
        None => Some(std::mem::take(&mut gui.new_language)).filter(|code| !code.is_empty()),
    };
    if let Some(code) = code {
        languages.insert(chord, code);
    }
}

fn key_row(gui: &mut Gui, ui: &mut egui::Ui, role: Role, hint_text: &str) {
    let capturing = gui
        .capture
        .as_ref()
        .is_some_and(|(target, _)| *target == KeyTarget::Action(role));
    let editing = gui.editing_key.as_ref().is_some_and(|(r, _)| *r == role);
    let value = gui.config.hotkey.chord(role).to_string();
    let error = combination_error(&value, required(role))
        .or_else(|| taken(&gui.config.hotkey, role, &value));
    let notice = printable_caution(&gui.config.hotkey, role);
    row(&role.label())
        .describe(hint_text)
        .problem(error.as_deref())
        .caution(notice.as_deref())
        .reserve(KEY_COLUMN)
        .show(ui, |ui| {
            if editing {
                let Some(slot) = gui.config.hotkey.chord_mut(role) else {
                    return;
                };
                let response = field(ui, slot, "Ctrl+Shift+Space");
                if gui.focus_editor {
                    response.request_focus();
                    gui.focus_editor = false;
                } else if response.lost_focus() {
                    finish_typing(gui);
                }
                return;
            }

            let idle = if value.is_empty() {
                egui::RichText::new("Not set").color(MUTED)
            } else {
                egui::RichText::new(&value).strong()
            };
            let button = capture_button(capturing, idle);
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
                if let Some(slot) = gui.config.hotkey.chord_mut(role) {
                    slot.clear();
                }
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
        if let Some(slot) = gui.config.hotkey.chord_mut(role) {
            *slot = before;
        }
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
            gui.capture = Some((KeyTarget::Action(role), capture));
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
    fn language_chords_are_taken_keys_but_not_their_own() {
        let mut hotkey = Config::defaults().unwrap().hotkey;
        hotkey
            .languages
            .insert("Shift+RightAlt".into(), "de".into());
        assert_eq!(
            refusal(&hotkey, Role::PasteLast, "shift+rightalt").as_deref(),
            Some("Dictate in German already uses Shift+RightAlt. Pick another key.")
        );
        assert_eq!(
            language_taken(&hotkey, Some("Shift+RightAlt"), "Shift+RightAlt"),
            None,
            "a language chord never conflicts with itself"
        );
        assert_eq!(
            language_taken(&hotkey, None, "Shift+RightAlt").as_deref(),
            Some("Dictate in German already uses Shift+RightAlt.")
        );
        assert_eq!(
            language_taken(&hotkey, Some("Shift+RightAlt"), "Ctrl+RightAlt").as_deref(),
            Some("Command already uses Ctrl+RightAlt.")
        );
        assert_eq!(language_taken(&hotkey, None, "F13"), None);
    }

    #[test]
    fn a_language_key_that_types_is_flagged() {
        let mut hotkey = Config::defaults().unwrap().hotkey;
        hotkey.languages.insert("Ctrl+G".into(), "de".into());
        hotkey.languages.insert("F13".into(), "fr".into());
        let caution = language_caution(&hotkey, "Ctrl+G").unwrap();
        assert!(caution.starts_with("Ctrl+G also reaches"), "{caution}");
        assert_eq!(language_caution(&hotkey, "F13"), None);
        hotkey.exclusive = true;
        assert_eq!(language_caution(&hotkey, "Ctrl+G"), None);
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
