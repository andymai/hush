//! What happens to the words after Whisper: tone by application, polishing
//! with a language model, writing styles, and the words Hush must spell
//! your way.

use crate::config::settings::LlmSetting;
use crate::config::{server, transfer, Config};
use crate::gui::icons;
use crate::gui::pages::text_setting;
use crate::gui::theme::{self, palette, FIELD_WIDTH};
use crate::gui::widgets::{
    self, actions, divider, dropdown, empty, icon_button, intro, list, list_item, option, row,
    section, submitted, switch, switch_row, text_area, value_field, Button, Field, Size, Variant,
};
use crate::gui::Gui;
use crate::gui::{checked_change, choose};
use crate::hotkey::warnings;
use crate::text_processing::learn::{self, ListImport};
use crate::text_processing::styles::{self, StyleConfig, StyleName};
use std::path::{Path, PathBuf};

/// Longer vocabularies get a field that narrows the list.
const FILTER_FROM: usize = 10;
/// The field, Done, and Cancel of a word open for editing.
const EDIT_COLUMN: f32 = FIELD_WIDTH + 140.0;
const ADD_COLUMN: f32 = FIELD_WIDTH + 90.0;
const LIST_COLUMN: f32 = FIELD_WIDTH + 200.0;
const PROMPT_WIDTH: f32 = 320.0;

pub const DEFAULT_WORD_LIST: &str = "~/hush-vocabulary.txt";

/// A vocabulary term open for editing.
pub struct TermEdit {
    pub original: String,
    pub draft: String,
    /// Put the cursor in the field on the next frame
    pub focus: bool,
}

/// The style form: a new style, or the changes to one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleDraft {
    /// The style that the form changes; `None` adds a style
    pub original: Option<String>,
    pub name: String,
    pub prompt: String,
    pub polish: bool,
}

impl Default for StyleDraft {
    fn default() -> Self {
        Self {
            original: None,
            name: String::new(),
            prompt: String::new(),
            polish: true,
        }
    }
}

impl StyleDraft {
    fn of(name: &str, style: &StyleConfig) -> Self {
        Self {
            original: Some(name.to_string()),
            name: name.to_string(),
            prompt: style.prompt.clone(),
            polish: style.polish,
        }
    }
}

pub fn show(gui: &mut Gui, ui: &mut egui::Ui) {
    tone(gui, ui);
    polish(gui, ui);
    writing_styles(gui, ui);
    vocabulary(gui, ui);
}

fn tone(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Tone");
    switch_row(
        ui,
        "text.profiles",
        "Match the application you are typing into",
        "Lower case and no full stop in terminals and editors, casual in chat, \
         full sentences in mail and documents.",
        &mut gui.config.profiles.enabled,
    );
}

fn polish(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Polish");
    let shown_before = gui.pending.provider.unwrap_or(gui.config.llm.provider);
    let waiting = gui.pending.provider.and_then(|provider| {
        checked_change(&gui.config, |config| config.llm.provider = provider)
            .err()
            .map(|reason| format!("Not switched yet: set the server below. {}", reason))
    });
    let mut shown = shown_before;
    row("text.llm_provider", "Polish with")
        .describe(
            "Only the text is ever sent, never the audio. Anthropic needs \
             ANTHROPIC_API_KEY in ~/.config/hush/.env. An OpenAI-compatible server \
             receives the text at the address you set.",
        )
        .problem(waiting.as_deref())
        .show(ui, |ui| {
            dropdown(
                ui,
                "llm_provider",
                FIELD_WIDTH,
                provider_name(shown),
                |ui| {
                    for provider in [
                        LlmSetting::Auto,
                        LlmSetting::Ollama,
                        LlmSetting::Anthropic,
                        LlmSetting::OpenAiCompatible,
                        LlmSetting::None,
                    ] {
                        option(ui, &mut shown, provider, provider_name(provider));
                    }
                },
            );
        });
    if shown != shown_before {
        gui.pending.provider = choose(&mut gui.config, shown, |config, provider| {
            config.llm.provider = provider
        });
    }
    row("text.polish", "Tidy up every transcript")
        .describe("Otherwise the model only handles spoken commands.")
        .reserve(36.0)
        .show(ui, |ui| {
            ui.add_enabled_ui(shown != LlmSetting::None, |ui| {
                switch(ui, &mut gui.config.llm.polish);
            });
        });

    section(ui, "Language models");
    text_setting(
        gui,
        ui,
        row("text.ollama_url", "Ollama server"),
        "http://localhost:11434",
        |config| &mut config.llm.ollama_url,
    );
    text_setting(
        gui,
        ui,
        row("text.ollama_model", "Ollama model"),
        "llama3.2",
        |config| &mut config.llm.ollama_model,
    );
    text_setting(
        gui,
        ui,
        row("text.anthropic_model", "Anthropic model"),
        "claude-haiku-4-5",
        |config| &mut config.llm.anthropic_model,
    );
    let reach = server_reach(&gui.config.llm.server_url);
    text_setting(
        gui,
        ui,
        row("text.server_url", "OpenAI-compatible server").describe(&reach),
        "http://localhost:8080/v1",
        |config| &mut config.llm.server_url,
    );
    text_setting(
        gui,
        ui,
        row("text.server_model", "OpenAI-compatible model"),
        "llama3.2",
        |config| &mut config.llm.server_model,
    );
}

fn writing_styles(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Styles");
    intro(
        ui,
        "A style changes one recording. A style key on the Hotkeys page, \
         hush start --style NAME, or profiles.app_styles in config.toml picks it.",
    );
    let names: Vec<String> = gui.config.styles.keys().cloned().collect();
    let mut remove: Option<String> = None;
    let mut change: Option<String> = None;
    divider(ui);
    list(ui, |ui| {
        list_item(
            "text.styles.verbatim",
            icons::PEN_LINE,
            styles::VERBATIM.as_str(),
        )
        .detail("Built in. No polish for the recording. The rule-based cleanup still runs.")
        .show(ui, 0.0, |_| {});
        for name in &names {
            let about = style_summary(&gui.config.styles[name]);
            let key = format!("text.styles.{}", name);
            list_item(&key, icons::PEN_LINE, name)
                .detail(&about)
                .show(ui, 52.0, |ui| {
                    if icon_button(ui, icons::X, &format!("Remove {}", name)).clicked() {
                        remove = Some(name.clone());
                    }
                    if icon_button(ui, icons::PENCIL, &format!("Change {}", name)).clicked() {
                        change = Some(name.clone());
                    }
                });
        }
    });
    if let Some(name) = remove {
        let mut refused = None;
        gui.remove(
            format!("Removed the style {}", name),
            |gui| match remove_style(&mut gui.config, &name) {
                Ok(()) => {
                    if gui.style_draft.original.as_deref() == Some(name.as_str()) {
                        gui.style_draft = StyleDraft::default();
                    }
                    true
                },
                Err(message) => {
                    refused = Some(message);
                    false
                },
            },
        );
        if let Some(message) = refused {
            gui.say(message);
        }
    }
    if let Some(name) = change {
        if let Some(style) = gui.config.styles.get(&name) {
            gui.style_draft = StyleDraft::of(&name, style);
        }
    }
    style_form(gui, ui);
}

fn style_form(gui: &mut Gui, ui: &mut egui::Ui) {
    let editing = gui.style_draft.original.clone();
    let heading = match &editing {
        Some(name) => format!("Change the style {}", name),
        None => "Add a style".to_string(),
    };
    section(ui, &heading);
    row("text.style_form.name", "Name")
        .describe("Lowercase letters, digits, - and _.")
        .show(ui, |ui| {
            value_field(ui, &mut gui.style_draft.name, "casual");
        });
    row("text.style_form.prompt", "Prompt")
        .describe("One line that Hush adds to the polish prompt.")
        .reserve(PROMPT_WIDTH)
        .show(ui, |ui| {
            text_area(
                ui,
                &mut gui.style_draft.prompt,
                "Casual and brief, like a chat message to a friend.",
                2,
                PROMPT_WIDTH,
            );
        });
    switch_row(
        ui,
        "text.style_form.polish",
        "Polish",
        "The model polishes recordings in this style, whatever Tidy up every transcript says.",
        &mut gui.style_draft.polish,
    );
    let ready = !gui.style_draft.name.trim().is_empty();
    let (done, cancel) = actions(ui, |ui| {
        let label = if editing.is_some() { "Done" } else { "Add" };
        let icon = if editing.is_some() {
            icons::CHECK
        } else {
            icons::PLUS
        };
        let done = ui
            .add(
                Button::new(label)
                    .icon(icon)
                    .variant(Variant::Primary)
                    .enabled(ready),
            )
            .clicked();
        let cancel = editing.is_some()
            && ui
                .add(Button::new("Cancel").variant(Variant::Ghost))
                .clicked();
        (done, cancel)
    });
    if cancel {
        gui.style_draft = StyleDraft::default();
    } else if done {
        match save_style(&mut gui.config, &gui.style_draft) {
            Ok(()) => gui.style_draft = StyleDraft::default(),
            Err(message) => gui.say(message),
        }
    }
}

fn style_summary(style: &StyleConfig) -> String {
    let prompt = style.prompt.trim();
    match (style.polish, prompt.is_empty()) {
        (false, _) => "No polish.".to_string(),
        (true, true) => "Polish, with no prompt.".to_string(),
        (true, false) => {
            let mut shown: String = prompt.chars().take(80).collect();
            if prompt.chars().count() > 80 {
                shown.push('…');
            }
            format!("Polish: {}", shown)
        },
    }
}

/// Put the draft into the config: add the style, or change the one that it
/// edits. A new name moves each style key and app rule to the new name.
pub fn save_style(config: &mut Config, draft: &StyleDraft) -> Result<(), String> {
    let name = draft.name.trim();
    let parsed = StyleName::parse(name)
        .ok_or_else(|| format!("'{}' is not a style name: {}.", name, styles::NAME_RULE))?;
    if parsed == styles::VERBATIM {
        return Err("Verbatim is built in. Pick another name.".to_string());
    }
    let renamed = draft.original.as_deref().filter(|old| *old != name);
    if draft.original.as_deref() != Some(name) && config.styles.contains_key(name) {
        return Err(format!(
            "A style with the name {} exists. Pick another name.",
            name
        ));
    }
    let style = StyleConfig {
        prompt: draft.prompt.trim().to_string(),
        polish: draft.polish,
    };
    let rewrite_keys: Vec<&String> = draft
        .original
        .as_deref()
        .map(|old| {
            config
                .hotkey
                .rewrite
                .iter()
                .filter(|(_, value)| value.as_str() == old)
                .map(|(chord, _)| chord)
                .collect()
        })
        .unwrap_or_default();
    if let (false, Some(chord)) = (style.can_rewrite(), rewrite_keys.first()) {
        return Err(format!(
            "The rewrite key {} uses this style, so it needs polish on and a prompt.",
            chord
        ));
    }
    if let Some(old) = renamed {
        config.styles.remove(old);
        let uses = config
            .hotkey
            .styles
            .values_mut()
            .chain(config.hotkey.rewrite.values_mut())
            .chain(config.profiles.app_styles.values_mut());
        for value in uses.filter(|value| value.as_str() == old) {
            *value = name.to_string();
        }
    }
    config.styles.insert(name.to_string(), style);
    Ok(())
}

/// Where the config names a style, for a message.
fn style_users(config: &Config, name: &str) -> Vec<String> {
    let keys = config
        .hotkey
        .styles
        .iter()
        .filter(|(_, style)| style.as_str() == name)
        .map(|(chord, _)| format!("the style key {}", chord));
    let rewrites = config
        .hotkey
        .rewrite
        .iter()
        .filter(|(_, style)| style.as_str() == name)
        .map(|(chord, _)| format!("the rewrite key {}", chord));
    let apps = config
        .profiles
        .app_styles
        .iter()
        .filter(|(_, style)| style.as_str() == name)
        .map(|(app, _)| format!("profiles.app_styles for {}", app));
    keys.chain(rewrites).chain(apps).collect()
}

/// Remove a style that nothing uses.
pub fn remove_style(config: &mut Config, name: &str) -> Result<(), String> {
    let users = style_users(config, name);
    if !users.is_empty() {
        return Err(format!(
            "The style {} is in use: {}. Remove that first.",
            name,
            warnings::join(&users)
        ));
    }
    config.styles.remove(name);
    Ok(())
}

/// Where the text goes when the server URL is set.
fn server_reach(url: &str) -> String {
    if url.trim().is_empty() {
        "Hush adds /v1/chat/completions when the address does not end with it.".to_string()
    } else if server::is_local(url) {
        format!("A local server: {}.", server::host_label(url))
    } else {
        format!("{} receives the text.", server::host_label(url))
    }
}

fn vocabulary(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Vocabulary");
    let mut add = false;
    row("text.vocabulary.add", "Add a word")
        .describe(
            "Hush spells these exactly this way. The Learn key adds the selected \
             text; you can also type one here.",
        )
        .reserve(ADD_COLUMN)
        .show(ui, |ui| {
            let button = Button::new("Add")
                .icon(icons::PLUS)
                .enabled(!gui.new_term.trim().is_empty());
            add |= ui.add(button).clicked();
            let response = widgets::field(ui, &mut gui.new_term, "Kubernetes");
            add |= submitted(ui, &response);
            if add {
                response.request_focus();
            }
        });
    if add {
        match learn::normalise(&gui.new_term) {
            Ok(term) => {
                learn::learn_into(&mut gui.vocabulary, &term);
                gui.new_term.clear();
            },
            Err(e) => gui.say(e.to_string()),
        }
    }
    word_list(gui, ui);

    let mut terms: Vec<String> = gui.vocabulary.technical_terms.values().cloned().collect();
    terms.sort_by_key(|term| term.to_lowercase());
    if terms.is_empty() {
        divider(ui);
        empty(ui, "No words yet.");
        return;
    }
    if terms.len() > FILTER_FROM {
        row("text.vocabulary.find", "Find a word")
            .reserve(FIELD_WIDTH + 32.0)
            .show(ui, |ui| {
                if gui.term_filter.is_empty() {
                    ui.add_space(32.0);
                } else if icon_button(ui, icons::X, "Clear the search").clicked() {
                    gui.term_filter.clear();
                }
                ui.add(Field::new(&mut gui.term_filter, "Type part of a word").icon(icons::SEARCH));
            });
    } else {
        gui.term_filter.clear();
    }
    let editing_original = gui.editing_term.as_ref().map(|edit| edit.original.as_str());
    let shown: Vec<String> = shown_terms(&terms, &gui.term_filter, editing_original)
        .into_iter()
        .cloned()
        .collect();
    let mut remove: Option<String> = None;
    let mut start: Option<String> = None;
    let mut commit = false;
    let mut cancel = false;
    let editing = &mut gui.editing_term;
    divider(ui);
    list(ui, |ui| {
        for term in &shown {
            let key = format!("text.vocabulary.{}", term);
            match editing.as_mut().filter(|edit| edit.original == *term) {
                Some(edit) => {
                    list_item(&key, icons::WHOLE_WORD, term).show(ui, EDIT_COLUMN, |ui| {
                        let small = |label| Button::new(label).size(Size::Small);
                        cancel |= ui.add(small("Cancel").variant(Variant::Ghost)).clicked();
                        commit |= ui
                            .add(small("Done").icon(icons::CHECK).variant(Variant::Primary))
                            .clicked();
                        let response = widgets::field(ui, &mut edit.draft, term);
                        if std::mem::take(&mut edit.focus) {
                            response.request_focus();
                        }
                        if response.lost_focus() {
                            commit |= ui.input(|i| i.key_pressed(egui::Key::Enter));
                            cancel |= ui.input(|i| i.key_pressed(egui::Key::Escape));
                        }
                    });
                },
                None => {
                    list_item(&key, icons::WHOLE_WORD, term).show(ui, 52.0, |ui| {
                        if icon_button(ui, icons::X, &format!("Remove {}", term)).clicked() {
                            remove = Some(term.clone());
                        }
                        if icon_button(ui, icons::PENCIL, &format!("Change {}", term)).clicked() {
                            start = Some(term.clone());
                        }
                    });
                },
            }
        }
    });
    if cancel {
        gui.editing_term = None;
    } else if commit {
        if let Some(edit) = gui.editing_term.take() {
            if let Err(e) = learn::rename_in(&mut gui.vocabulary, &edit.original, &edit.draft) {
                gui.say(e.to_string());
                gui.editing_term = Some(edit);
            }
        }
    }
    if let Some(term) = start {
        gui.editing_term = Some(TermEdit {
            draft: term.clone(),
            original: term,
            focus: true,
        });
    }
    if let Some(term) = remove {
        gui.remove(format!("Removed {}", term), |gui| {
            learn::forget_in(&mut gui.vocabulary, &term)
        });
    }
    let muted = palette(ui.ctx()).muted;
    widgets::text(
        ui,
        &count_hint(shown.len(), terms.len()),
        theme::sans(theme::SMALL),
        muted,
    );
}

/// The terms that contain `filter`, ignoring case, in their order. The term
/// open for editing stays shown, so a filter change cannot hide the form.
fn shown_terms<'a>(terms: &'a [String], filter: &str, editing: Option<&str>) -> Vec<&'a String> {
    let filter = filter.trim().to_lowercase();
    terms
        .iter()
        .filter(|term| {
            filter.is_empty()
                || term.to_lowercase().contains(&filter)
                || editing == Some(term.as_str())
        })
        .collect()
}

fn count_hint(shown: usize, total: usize) -> String {
    let words = |n: usize| {
        if n == 1 {
            "1 word".to_string()
        } else {
            format!("{} words", n)
        }
    };
    match shown {
        _ if shown == total => format!("{}.", words(total)),
        0 => format!("No word matches. {} in all.", words(total)),
        _ => format!("{} of {}.", shown, words(total)),
    }
}

/// The vocabulary page lists a search field over the words when it has
/// more than this many.
pub fn many_words(gui: &Gui) -> bool {
    gui.vocabulary.technical_terms.len() > FILTER_FROM
}

/// Import reads the word list file named in its field.
pub fn can_import(gui: &Gui) -> bool {
    !gui.word_list.trim().is_empty()
}

/// Export writes every word to the file named in its field.
pub fn can_export(gui: &Gui) -> bool {
    can_import(gui) && !gui.vocabulary.technical_terms.is_empty()
}

fn word_list(gui: &mut Gui, ui: &mut egui::Ui) {
    let has_path = can_import(gui);
    let has_terms = !gui.vocabulary.technical_terms.is_empty();
    let (import, export) = row("text.vocabulary.word_list", "Word list")
        .describe(
            "A text file with one word on each line. Import adds each word that \
             the vocabulary does not have. Export writes every word to a new file.",
        )
        .reserve(LIST_COLUMN)
        .show(ui, |ui| {
            let export = ui
                .add(
                    Button::new("Export")
                        .icon(icons::FILE_OUTPUT)
                        .enabled(has_path && has_terms),
                )
                .clicked();
            let import = ui
                .add(
                    Button::new("Import")
                        .icon(icons::FILE_INPUT)
                        .enabled(has_path),
                )
                .clicked();
            value_field(ui, &mut gui.word_list, DEFAULT_WORD_LIST);
            (import, export)
        });
    if import {
        import_word_list(gui);
    }
    if export {
        export_word_list(gui);
    }
}

fn word_list_path(gui: &Gui) -> PathBuf {
    list_path(&gui.word_list, &dirs::home_dir().unwrap_or_default())
}

/// Add each word of the word list file that the vocabulary does not have.
pub fn import_word_list(gui: &mut Gui) {
    let path = word_list_path(gui);
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            let result = learn::import_list(&mut gui.vocabulary, &text);
            gui.say(import_message(result));
        },
        Err(e) => gui.say(format!("Could not read {}: {}", path.display(), e)),
    }
}

/// Write every word to a new word list file.
pub fn export_word_list(gui: &mut Gui) {
    let path = word_list_path(gui);
    if path.exists() {
        gui.say(format!(
            "{} already exists. Type the name of a new file.",
            path.display()
        ));
        return;
    }
    let text = learn::to_list(&gui.vocabulary);
    match transfer::write_atomic(&path, text.as_bytes(), false) {
        Ok(()) => gui.say(format!(
            "Wrote {} to {}.",
            words(text.lines().count()),
            path.display()
        )),
        Err(e) => gui.say(format!("Could not export: {:#}", e)),
    }
}

/// `~/` is the home folder. A relative path also starts there, because the
/// window has no working folder that the user chose.
fn list_path(text: &str, home: &Path) -> PathBuf {
    let text = text.trim();
    home.join(text.strip_prefix("~/").unwrap_or(text))
}

fn import_message(result: ListImport) -> String {
    let mut message = format!("Added {}.", words(result.added));
    if result.skipped == 1 {
        message.push_str(" Skipped 1 duplicate.");
    } else if result.skipped > 1 {
        message.push_str(&format!(" Skipped {} duplicates.", result.skipped));
    }
    if result.invalid > 0 {
        message.push_str(&format!(
            " Ignored {} longer than {} words or {} characters.",
            lines(result.invalid),
            learn::MAX_WORDS,
            learn::MAX_CHARS
        ));
    }
    message
}

fn words(n: usize) -> String {
    if n == 1 {
        "1 word".to_string()
    } else {
        format!("{} words", n)
    }
}

fn lines(n: usize) -> String {
    if n == 1 {
        "1 line".to_string()
    } else {
        format!("{} lines", n)
    }
}

fn provider_name(provider: LlmSetting) -> &'static str {
    match provider {
        LlmSetting::Auto => "Ollama, else Anthropic",
        LlmSetting::Ollama => "Ollama",
        LlmSetting::Anthropic => "Anthropic",
        LlmSetting::OpenAiCompatible => "OpenAI-compatible server",
        LlmSetting::None => "Nothing",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terms(list: &[&str]) -> Vec<String> {
        list.iter().map(|term| term.to_string()).collect()
    }

    #[test]
    fn the_filter_keeps_terms_that_contain_it_ignoring_case() {
        let all = terms(&["Kubernetes", "kubectl", "PostgreSQL", "Rust"]);
        let shown = shown_terms(&all, " KUB ", None);
        assert_eq!(shown, vec!["Kubernetes", "kubectl"]);
        assert_eq!(shown_terms(&all, "", None).len(), 4);
        assert!(shown_terms(&all, "zzz", None).is_empty());
    }

    #[test]
    fn the_term_open_for_editing_stays_shown() {
        let all = terms(&["Kubernetes", "PostgreSQL"]);
        assert_eq!(
            shown_terms(&all, "kube", Some("PostgreSQL")),
            vec!["Kubernetes", "PostgreSQL"]
        );
    }

    #[test]
    fn the_count_says_how_many_words_the_filter_shows() {
        assert_eq!(count_hint(1, 1), "1 word.");
        assert_eq!(count_hint(12, 12), "12 words.");
        assert_eq!(count_hint(3, 12), "3 of 12 words.");
        assert_eq!(count_hint(0, 12), "No word matches. 12 words in all.");
    }

    #[test]
    fn the_server_row_says_local_server_or_names_the_receiver() {
        assert!(server_reach("").contains("/v1/chat/completions"));
        assert_eq!(
            server_reach("http://192.168.1.9:8000/v1"),
            "A local server: 192.168.1.9:8000."
        );
        assert_eq!(
            server_reach("https://llm.example.com"),
            "llm.example.com receives the text."
        );
    }

    #[test]
    fn a_word_list_path_starts_at_home() {
        let home = Path::new("/home/me");
        assert_eq!(
            list_path(" ~/words.txt ", home),
            Path::new("/home/me/words.txt")
        );
        assert_eq!(
            list_path("words.txt", home),
            Path::new("/home/me/words.txt")
        );
        assert_eq!(
            list_path("/tmp/words.txt", home),
            Path::new("/tmp/words.txt")
        );
    }

    fn draft(original: Option<&str>, name: &str, prompt: &str, polish: bool) -> StyleDraft {
        StyleDraft {
            original: original.map(str::to_string),
            name: name.to_string(),
            prompt: prompt.to_string(),
            polish,
        }
    }

    #[test]
    fn the_style_form_adds_and_changes_styles() {
        let mut config = Config::defaults().unwrap();
        save_style(&mut config, &draft(None, " casual ", " Brief. ", true)).unwrap();
        assert_eq!(config.styles["casual"].prompt, "Brief.");
        assert!(config.styles["casual"].polish);

        let err = save_style(&mut config, &draft(None, "casual", "x", true)).unwrap_err();
        assert!(err.contains("exists"), "{err}");
        let err = save_style(&mut config, &draft(None, "verbatim", "x", true)).unwrap_err();
        assert!(err.contains("built in"), "{err}");
        let err = save_style(&mut config, &draft(None, "Big Name", "x", true)).unwrap_err();
        assert!(err.contains("'Big Name'"), "{err}");

        save_style(
            &mut config,
            &draft(Some("casual"), "casual", "Short.", false),
        )
        .unwrap();
        assert_eq!(config.styles["casual"].prompt, "Short.");
        assert!(!config.styles["casual"].polish);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn a_new_name_moves_the_keys_and_app_rules() {
        let mut config = Config::defaults().unwrap();
        save_style(&mut config, &draft(None, "casual", "Brief.", true)).unwrap();
        save_style(&mut config, &draft(None, "formal", "Formal.", true)).unwrap();
        config.hotkey.styles.insert("F14".into(), "casual".into());
        config.hotkey.styles.insert("F15".into(), "verbatim".into());
        config
            .profiles
            .app_styles
            .insert("slack".into(), "casual".into());

        let err = save_style(&mut config, &draft(Some("casual"), "formal", "x", true)).unwrap_err();
        assert!(err.contains("exists"), "{err}");
        save_style(&mut config, &draft(Some("casual"), "chat", "Brief.", true)).unwrap();
        assert!(!config.styles.contains_key("casual"));
        assert_eq!(config.hotkey.styles["F14"], "chat");
        assert_eq!(config.hotkey.styles["F15"], "verbatim");
        assert_eq!(config.profiles.app_styles["slack"], "chat");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn a_rewrite_key_keeps_its_style_able_to_rewrite() {
        let mut config = Config::defaults().unwrap();
        save_style(&mut config, &draft(None, "formal", "Formal.", true)).unwrap();
        config
            .hotkey
            .rewrite
            .insert("Super+F".into(), "formal".into());
        for (prompt, polish) in [("Formal.", false), (" ", true)] {
            let err = save_style(
                &mut config,
                &draft(Some("formal"), "formal", prompt, polish),
            )
            .unwrap_err();
            assert!(err.contains("rewrite key Super+F"), "{err}");
        }
        save_style(
            &mut config,
            &draft(Some("formal"), "proper", "Proper.", true),
        )
        .unwrap();
        assert_eq!(config.hotkey.rewrite["Super+F"], "proper");
        assert!(config.validate().is_ok());
        let err = remove_style(&mut config, "proper").unwrap_err();
        assert!(err.contains("the rewrite key Super+F"), "{err}");
    }

    #[test]
    fn a_style_in_use_is_not_removed() {
        let mut config = Config::defaults().unwrap();
        save_style(&mut config, &draft(None, "casual", "Brief.", true)).unwrap();
        config.hotkey.styles.insert("F14".into(), "casual".into());
        config
            .profiles
            .app_styles
            .insert("slack".into(), "casual".into());
        let err = remove_style(&mut config, "casual").unwrap_err();
        assert_eq!(
            err,
            "The style casual is in use: the style key F14 and profiles.app_styles for \
             slack. Remove that first."
        );
        config.hotkey.styles.clear();
        config.profiles.app_styles.clear();
        remove_style(&mut config, "casual").unwrap();
        assert!(config.styles.is_empty());
    }

    #[test]
    fn a_style_summary_says_what_the_style_does() {
        let style = |prompt: &str, polish| StyleConfig {
            prompt: prompt.to_string(),
            polish,
        };
        assert_eq!(style_summary(&style("Brief.", false)), "No polish.");
        assert_eq!(style_summary(&style(" ", true)), "Polish, with no prompt.");
        assert_eq!(style_summary(&style("Brief.", true)), "Polish: Brief.");
        assert!(style_summary(&style(&"x".repeat(100), true)).ends_with('…'));
    }

    #[test]
    fn the_import_message_counts_each_outcome() {
        let result = ListImport {
            added: 2,
            skipped: 1,
            invalid: 3,
        };
        assert_eq!(
            import_message(result),
            "Added 2 words. Skipped 1 duplicate. Ignored 3 lines longer than \
             5 words or 64 characters."
        );
        assert_eq!(
            import_message(ListImport {
                skipped: 4,
                ..ListImport::default()
            }),
            "Added 0 words. Skipped 4 duplicates."
        );
    }
}
