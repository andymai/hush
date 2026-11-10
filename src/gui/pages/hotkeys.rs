//! The keys that start, stop, cancel, and command a dictation, a key for
//! each language and each writing style, and keys that rewrite the
//! selection with a style. Each one is a button that captures the next key
//! pressed; the action keys have a text box behind it for keys the capture
//! cannot see.

use crate::config::settings::{HotkeyConfig, MAX_TAP_MS};
use crate::gui::capture::Capture;
use crate::gui::icons;
use crate::gui::pages::number_setting;
use crate::gui::pages::speech::language_picker;
use crate::gui::theme::palette;
use crate::gui::widgets::{
    dropdown, forget_draft, icon_button, intro, key_field, notice, option, row, section, segmented,
    setting_field, switch_row, FieldKind, FieldOutcome,
};
use crate::gui::Gui;
use crate::hotkey::warnings;
use crate::hotkey::{HotkeyBackend, HotkeyMode, KeyCombination, Role};
use crate::text_processing::styles::{self, StyleName};
use crate::transcription::Language;
use std::collections::BTreeMap;

/// A table under `[hotkey]` that gives each chord a value: a language code
/// or a style name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChordMap {
    Languages,
    Styles,
    Rewrite,
}

impl ChordMap {
    fn entries(self, hotkey: &HotkeyConfig) -> &BTreeMap<String, String> {
        match self {
            ChordMap::Languages => &hotkey.languages,
            ChordMap::Styles => &hotkey.styles,
            ChordMap::Rewrite => &hotkey.rewrite,
        }
    }

    fn entries_mut(self, hotkey: &mut HotkeyConfig) -> &mut BTreeMap<String, String> {
        match self {
            ChordMap::Languages => &mut hotkey.languages,
            ChordMap::Styles => &mut hotkey.styles,
            ChordMap::Rewrite => &mut hotkey.rewrite,
        }
    }

    /// The role of a chord with this value, when the value is valid.
    fn role(self, value: &str) -> Option<Role> {
        match self {
            ChordMap::Languages => Language::parse(value).map(Role::Language),
            ChordMap::Styles => StyleName::parse(value).map(Role::Style),
            ChordMap::Rewrite => StyleName::parse(value).map(Role::Rewrite),
        }
    }

    fn owns(self, role: Role) -> bool {
        match self {
            ChordMap::Languages => matches!(role, Role::Language(_)),
            ChordMap::Styles => matches!(role, Role::Style(_)),
            ChordMap::Rewrite => matches!(role, Role::Rewrite(_)),
        }
    }

    fn title(self, value: &str) -> String {
        match (self.role(value), self) {
            (Some(role), _) => role.label(),
            (None, ChordMap::Languages) => format!("Dictate in {}", value),
            (None, ChordMap::Styles) => format!("Dictate in the {} style", value),
            (None, ChordMap::Rewrite) => format!("Rewrite in the {} style", value),
        }
    }

    /// The value for the next chord, picked before the key is captured.
    fn new_value(self, gui: &mut Gui) -> &mut String {
        match self {
            ChordMap::Languages => &mut gui.new_language,
            ChordMap::Styles => &mut gui.new_style,
            ChordMap::Rewrite => &mut gui.new_rewrite,
        }
    }

    /// The styles that the picker offers: every style for a style key, and
    /// only the styles that can rewrite for a rewrite key.
    fn style_names(self, config: &crate::config::Config) -> Vec<StyleName> {
        match self {
            ChordMap::Rewrite => styles::rewriters(&config.styles),
            _ => styles::names(&config.styles),
        }
    }
}

/// What a captured key goes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyTarget {
    Action(Role),
    /// A chord in a [`ChordMap`], by the chord it has now; `None` adds a
    /// chord for the value that [`ChordMap::new_value`] holds.
    Chord(ChordMap, Option<String>),
}

/// Dictation cannot be unbound; every other key can.
fn required(role: Role) -> bool {
    role == Role::Dictate
}

/// Width of a key field.
const KEY_WIDTH: f32 = 200.0;

/// Width of the key field plus its Type and Clear buttons.
const KEY_COLUMN: f32 = KEY_WIDTH + 64.0;

/// Width of a key field beside a language or style picker.
const CHORD_KEY_WIDTH: f32 = 160.0;

/// Width of the language or style picker beside a key.
const PICKER_WIDTH: f32 = 130.0;

/// Width of the key field, the picker, and Remove.
const CHORD_COLUMN: f32 = CHORD_KEY_WIDTH + PICKER_WIDTH + 40.0;

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

/// The actions and chords, other than the chord `own` in `map`, that
/// already use `chord`, as a sentence.
pub fn map_taken(
    hotkey: &HotkeyConfig,
    map: ChordMap,
    own: Option<&str>,
    chord: &str,
) -> Option<String> {
    let others = hotkey
        .bound()
        .into_iter()
        .filter(|(role, text)| !(map.owns(*role) && Some(*text) == own));
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

/// [`printable_caution`] for the chord `chord` in `map`.
fn map_caution(hotkey: &HotkeyConfig, map: ChordMap, chord: &str) -> Option<String> {
    let own = map
        .entries(hotkey)
        .iter()
        .filter(|(text, _)| text.as_str() == chord)
        .filter_map(|(text, value)| Some((map.role(value)?, text.as_str())));
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
    section(ui, "Dictation");
    intro(
        ui,
        "Click a key, then press the one you want. A modifier on its own, such as \
         RightAlt, types no character in other applications.",
    );
    if let Some(altgr) = warnings::altgr(
        gui.config.hotkey.bound(),
        gui.layout.as_ref(),
        &gui.modifier_keys,
    ) {
        let warn = palette(ui.ctx()).warn;
        notice(ui, icons::TRIANGLE_ALERT, warn, &altgr_caution(&altgr));
        ui.add_space(8.0);
    }

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
    row("hotkeys.mode", "Mode")
        .describe("Hold to talk, or tap to start and tap to stop.")
        .reserve(250.0)
        .show(ui, |ui| {
            let options =
                [HotkeyMode::Hold, HotkeyMode::Toggle].map(|mode| (mode, mode_name(mode)));
            segmented(ui, &mut gui.config.hotkey.mode, &options);
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
    row("hotkeys.backend", "Read keys through")
        .describe("The input devices work on Wayland and X11 alike. X11 is the fallback.")
        .reserve(280.0)
        .show(ui, |ui| {
            let options = [
                HotkeyBackend::Auto,
                HotkeyBackend::Evdev,
                HotkeyBackend::X11,
            ]
            .map(|backend| (backend, backend_name(backend)));
            segmented(ui, &mut gui.config.hotkey.backend, &options);
        });
    number_setting(
        gui,
        ui,
        row("hotkeys.tap_ms", "Tap is shorter than")
            .describe("A shorter press is a tap: taps lock and unlock, holds record.")
            .reserve(110.0),
        0..=MAX_TAP_MS,
        "ms",
        |config| &mut config.hotkey.tap_ms,
    );
    switch_row(
        ui,
        "hotkeys.exclusive",
        "Keep the key from other applications",
        "Grabs the device the key belongs to. Needed for mouse buttons, which \
         browsers otherwise treat as Back.",
        &mut gui.config.hotkey.exclusive,
    );
    switch_row(
        ui,
        "hotkeys.mask_menu",
        "Keep menus closed",
        "With an Alt or Super key on its own, Hush taps Ctrl when the key goes down. \
         Applications then open no menu, and the desktop opens no launcher. On Sway, \
         Hush cannot read the keyboard options and sends no tap.",
        &mut gui.config.hotkey.mask_menu,
    );
    chord_keys(
        gui,
        ui,
        ChordMap::Languages,
        "Language keys",
        "Each key dictates in its own language, with the same hold, double-tap, and \
         cancel as the dictation key. The tray menu sets the language of the dictation key.",
    );
    chord_keys(
        gui,
        ui,
        ChordMap::Styles,
        "Style keys",
        "Each key dictates in its own writing style, with the same hold, double-tap, and \
         cancel as the dictation key. Verbatim turns polish off. Add styles on the Text page.",
    );
    chord_keys(
        gui,
        ui,
        ChordMap::Rewrite,
        "Rewrite keys",
        "Each key rewrites the selected text with the prompt of a style, with no speech. \
         A style needs polish on and a prompt to rewrite.",
    );
}

fn altgr_caution(altgr: &warnings::AltGr) -> String {
    let advice = altgr.advice();
    let mut chars = advice.chars();
    let advice = match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => advice,
    };
    format!("{}. {}.", altgr, advice)
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

fn chord_keys(gui: &mut Gui, ui: &mut egui::Ui, map: ChordMap, title: &str, about: &str) {
    section(ui, title);
    intro(ui, about);
    let chords: Vec<String> = map.entries(&gui.config.hotkey).keys().cloned().collect();
    for chord in chords {
        chord_row(gui, ui, map, &chord);
    }
    new_chord_row(gui, ui, map);
}

/// The dropdown that picks the value of a chord in `map`.
fn picker(
    ui: &mut egui::Ui,
    map: ChordMap,
    id_salt: impl std::hash::Hash,
    value: &mut String,
    style_names: &[StyleName],
    placeholder: &str,
) {
    match map {
        ChordMap::Languages => {
            language_picker(ui, id_salt, value, true, PICKER_WIDTH, placeholder);
        },
        ChordMap::Styles | ChordMap::Rewrite => {
            let current = value.clone();
            let shown = if current.is_empty() {
                placeholder.to_string()
            } else {
                current.clone()
            };
            dropdown(ui, id_salt, PICKER_WIDTH, &shown, |ui| {
                if !current.is_empty() && !style_names.iter().any(|n| n.as_str() == current) {
                    option(ui, value, current.clone(), &current);
                }
                for name in style_names {
                    option(ui, value, name.to_string(), name.as_str());
                }
            });
        },
    }
}

fn chord_row(gui: &mut Gui, ui: &mut egui::Ui, map: ChordMap, chord: &str) {
    let target = KeyTarget::Chord(map, Some(chord.to_string()));
    let capturing = gui.capture.as_ref().is_some_and(|(t, _)| *t == target);
    let value = map
        .entries(&gui.config.hotkey)
        .get(chord)
        .cloned()
        .unwrap_or_default();
    let error = combination_error(chord, true)
        .or_else(|| map_taken(&gui.config.hotkey, map, Some(chord), chord));
    let notice = map_caution(&gui.config.hotkey, map, chord);
    let style_names = map.style_names(&gui.config);
    let key = format!("hotkeys.{:?}.{}", map, chord).to_lowercase();
    row(&key, &map.title(&value))
        .problem(error.as_deref())
        .caution(notice.as_deref())
        .reserve(CHORD_COLUMN)
        .show(ui, |ui| {
            if icon_button(ui, icons::X, "Remove this key").clicked() {
                gui.capture = None;
                map.entries_mut(&mut gui.config.hotkey).remove(chord);
                return;
            }
            if let Some(value) = map.entries_mut(&mut gui.config.hotkey).get_mut(chord) {
                picker(ui, map, (map, chord), value, &style_names, "");
            }
            if key_field(ui, chord, capturing, CHORD_KEY_WIDTH, "Not set")
                .on_hover_text("Click, then press the key you want")
                .clicked()
            {
                toggle_capture(gui, target);
            }
        });
}

fn new_chord_row(gui: &mut Gui, ui: &mut egui::Ui, map: ChordMap) {
    let target = KeyTarget::Chord(map, None);
    let capturing = gui.capture.as_ref().is_some_and(|(t, _)| *t == target);
    let (title, about, placeholder) = match map {
        ChordMap::Languages => (
            "Add a language key",
            "Pick a language, then click the button and press the key.",
            "Language",
        ),
        ChordMap::Styles => (
            "Add a style key",
            "Pick a style, then click the button and press the key.",
            "Style",
        ),
        ChordMap::Rewrite => (
            "Add a rewrite key",
            "Pick a style, then click the button and press the key.",
            "Style",
        ),
    };
    let style_names = map.style_names(&gui.config);
    let key = format!("hotkeys.{:?}.new", map).to_lowercase();
    row(&key, title)
        .describe(about)
        .reserve(CHORD_KEY_WIDTH + PICKER_WIDTH + 8.0)
        .show(ui, |ui| {
            let ready = !map.new_value(gui).is_empty();
            let clicked = ui
                .add_enabled_ui(ready, |ui| {
                    key_field(ui, "", capturing, CHORD_KEY_WIDTH, "Choose the key").clicked()
                })
                .inner;
            if clicked {
                toggle_capture(gui, target);
            }
            picker(
                ui,
                map,
                ("new_chord", map),
                map.new_value(gui),
                &style_names,
                placeholder,
            );
        });
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
            stop_typing(gui);
            gui.capture = Some((target, capture));
        },
        Err(e) => gui.say(format!("Could not read the keyboard: {}", e)),
    }
}

/// Give the captured `chord` to `map`: move the chord `own` to it, or add a
/// chord for the value that [`ChordMap::new_value`] holds. Another action's
/// chord is refused.
pub fn bind_chord(gui: &mut Gui, map: ChordMap, own: Option<String>, chord: String) {
    if let Some(message) = map_taken(&gui.config.hotkey, map, own.as_deref(), &chord) {
        gui.say(format!("{} Pick another key.", message));
        return;
    }
    let value = match own {
        Some(own) => map.entries_mut(&mut gui.config.hotkey).remove(&own),
        None => Some(std::mem::take(map.new_value(gui))).filter(|value| !value.is_empty()),
    };
    if let Some(value) = value {
        map.entries_mut(&mut gui.config.hotkey).insert(chord, value);
    }
}

fn key_row(gui: &mut Gui, ui: &mut egui::Ui, role: Role, hint_text: &str) {
    let capturing = gui
        .capture
        .as_ref()
        .is_some_and(|(target, _)| *target == KeyTarget::Action(role));
    let editing = gui.editing_key == Some(role);
    let value = gui.config.hotkey.chord(role).to_string();
    let error = combination_error(&value, required(role))
        .or_else(|| taken(&gui.config.hotkey, role, &value));
    let notice = printable_caution(&gui.config.hotkey, role);
    let key = format!("hotkeys.{:?}", role).to_lowercase();
    row(&key, &role.label())
        .describe(hint_text)
        .problem(error.as_deref())
        .caution(notice.as_deref())
        .reserve(KEY_COLUMN)
        .show(ui, |ui| {
            if editing {
                ui.add_space(64.0);
                let focus = std::mem::take(&mut gui.focus_editor);
                let typed = setting_field(
                    ui,
                    &key,
                    &value,
                    "Ctrl+Shift+Space",
                    FieldKind::Value,
                    focus,
                );
                match typed {
                    FieldOutcome::Commit(commit) => match set_typed(gui, role, &commit.text) {
                        Ok(()) => gui.editing_key = None,
                        Err(problem) => commit.reject(ui.ctx(), problem),
                    },
                    FieldOutcome::Closed => gui.editing_key = None,
                    FieldOutcome::Idle | FieldOutcome::Editing => {},
                }
                return;
            }
            forget_draft(ui.ctx(), &key);

            let clear = !required(role) && !value.is_empty();
            if !clear {
                ui.add_space(32.0);
            }
            if clear && icon_button(ui, icons::X, "Clear the key").clicked() {
                gui.capture = None;
                if let Some(slot) = gui.config.hotkey.chord_mut(role) {
                    slot.clear();
                }
            }
            if icon_button(ui, icons::PENCIL, "Type the key's name instead").clicked() {
                gui.capture = None;
                start_typing(gui, role);
            }
            if key_field(ui, &value, capturing, KEY_WIDTH, "Not set")
                .on_hover_text("Click, then press the key you want")
                .clicked()
            {
                if capturing {
                    gui.capture = None;
                } else {
                    start_capture(gui, role);
                }
            }
        });
}

fn start_typing(gui: &mut Gui, role: Role) {
    gui.editing_key = Some(role);
    gui.focus_editor = true;
}

/// Close the typed key's field without saving what it holds.
fn stop_typing(gui: &mut Gui) {
    gui.editing_key = None;
}

/// Save a typed key: one that parses, that dictation has when it needs one,
/// and that no other action uses.
fn set_typed(gui: &mut Gui, role: Role, typed: &str) -> Result<(), String> {
    let typed = typed.trim();
    if let Some(problem) = combination_error(typed, required(role)) {
        return Err(problem);
    }
    if let Some(problem) = refusal(&gui.config.hotkey, role, typed) {
        return Err(problem);
    }
    let typed = typed.to_string();
    gui.try_set(|config| {
        if let Some(slot) = config.hotkey.chord_mut(role) {
            *slot = typed;
        }
    })
}

fn start_capture(gui: &mut Gui, role: Role) {
    if gui.machine.permissions.readable_event_nodes == 0 {
        gui.say("Grant keyboard access on Overview, or type the key's name.");
        start_typing(gui, role);
        return;
    }
    match Capture::start() {
        Ok(capture) => {
            stop_typing(gui);
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
        let languages = ChordMap::Languages;
        assert_eq!(
            map_taken(&hotkey, languages, Some("Shift+RightAlt"), "Shift+RightAlt"),
            None,
            "a language chord never conflicts with itself"
        );
        assert_eq!(
            map_taken(&hotkey, languages, None, "Shift+RightAlt").as_deref(),
            Some("Dictate in German already uses Shift+RightAlt.")
        );
        assert_eq!(
            map_taken(&hotkey, languages, Some("Shift+RightAlt"), "Ctrl+RightAlt").as_deref(),
            Some("Command already uses Ctrl+RightAlt.")
        );
        assert_eq!(map_taken(&hotkey, languages, None, "F13"), None);
    }

    #[test]
    fn style_chords_are_taken_keys_but_not_their_own() {
        let mut hotkey = Config::defaults().unwrap().hotkey;
        hotkey
            .languages
            .insert("Shift+RightAlt".into(), "de".into());
        hotkey
            .styles
            .insert("Super+RightAlt".into(), "verbatim".into());
        let style_map = ChordMap::Styles;
        assert_eq!(
            refusal(&hotkey, Role::Learn, "super+rightalt").as_deref(),
            Some("Dictate in the verbatim style already uses Super+RightAlt. Pick another key.")
        );
        assert_eq!(
            map_taken(&hotkey, style_map, Some("Super+RightAlt"), "Super+RightAlt"),
            None,
            "a style chord never conflicts with itself"
        );
        assert_eq!(
            map_taken(&hotkey, style_map, Some("Super+RightAlt"), "Shift+RightAlt").as_deref(),
            Some("Dictate in German already uses Shift+RightAlt.")
        );
        assert_eq!(
            map_taken(
                &hotkey,
                ChordMap::Languages,
                Some("Super+RightAlt"),
                "Super+RightAlt"
            )
            .as_deref(),
            Some("Dictate in the verbatim style already uses Super+RightAlt."),
            "only a chord of the same table is its own"
        );
        assert_eq!(style_map.title("verbatim"), "Dictate in the verbatim style");
        assert_eq!(style_map.title("Bad"), "Dictate in the Bad style");
        hotkey.styles.insert("Ctrl+G".into(), "verbatim".into());
        assert!(map_caution(&hotkey, style_map, "Ctrl+G").is_some());
        assert_eq!(map_caution(&hotkey, style_map, "Super+RightAlt"), None);
    }

    #[test]
    fn rewrite_chords_are_taken_keys_but_not_their_own() {
        let mut config = Config::defaults().unwrap();
        config.styles.insert(
            "formal".into(),
            crate::text_processing::StyleConfig {
                prompt: "Formal.".into(),
                polish: true,
            },
        );
        config
            .hotkey
            .styles
            .insert("Super+RightAlt".into(), "formal".into());
        config
            .hotkey
            .rewrite
            .insert("Super+F".into(), "formal".into());
        let rewrite = ChordMap::Rewrite;
        assert_eq!(
            refusal(&config.hotkey, Role::Learn, "super+f").as_deref(),
            Some("Rewrite in the formal style already uses Super+F. Pick another key.")
        );
        assert_eq!(
            map_taken(&config.hotkey, rewrite, Some("Super+F"), "Super+F"),
            None
        );
        assert_eq!(
            map_taken(&config.hotkey, rewrite, Some("Super+F"), "Super+RightAlt").as_deref(),
            Some("Dictate in the formal style already uses Super+RightAlt.")
        );
        assert_eq!(rewrite.title("formal"), "Rewrite in the formal style");
        let formal = StyleName::parse("formal").unwrap();
        assert_eq!(rewrite.style_names(&config), vec![formal]);
        assert_eq!(
            ChordMap::Styles.style_names(&config),
            vec![styles::VERBATIM, formal],
            "verbatim cannot rewrite, but it can dictate"
        );
    }

    #[test]
    fn a_language_key_that_types_is_flagged() {
        let mut hotkey = Config::defaults().unwrap().hotkey;
        hotkey.languages.insert("Ctrl+G".into(), "de".into());
        hotkey.languages.insert("F13".into(), "fr".into());
        let caution = map_caution(&hotkey, ChordMap::Languages, "Ctrl+G").unwrap();
        assert!(caution.starts_with("Ctrl+G also reaches"), "{caution}");
        assert_eq!(map_caution(&hotkey, ChordMap::Languages, "F13"), None);
        hotkey.exclusive = true;
        assert_eq!(map_caution(&hotkey, ChordMap::Languages, "Ctrl+G"), None);
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
