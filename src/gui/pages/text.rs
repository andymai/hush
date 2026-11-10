//! What happens to the words after Whisper: tone by application, polishing
//! with a language model, writing styles, and the words Hush must spell
//! your way.

use crate::config::settings::LlmSetting;
use crate::config::{server, transfer, Config};
use crate::gui::theme::{
    field, hint, quiet_button, row, section, toggle, toggle_row, CONTROL_HEIGHT, FIELD_WIDTH,
    MUTED, TOGGLE_COLUMN,
};
use crate::gui::Gui;
use crate::hotkey::warnings;
use crate::text_processing::learn::{self, ListImport};
use crate::text_processing::styles::{self, StyleConfig, StyleName};
use std::path::{Path, PathBuf};

const VOCABULARY_HEIGHT: f32 = 220.0;
const EDIT_COLUMN: f32 = 340.0;
const LIST_COLUMN: f32 = FIELD_WIDTH + 140.0;
const PROMPT_WIDTH: f32 = 320.0;
const PROMPT_HEIGHT: f32 = 56.0;

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
    toggle_row(
        ui,
        "Match the application you are typing into",
        "Lower case and no full stop in terminals and editors, casual in chat, \
         full sentences in mail and documents.",
        &mut gui.config.profiles.enabled,
    );
}

fn polish(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Polish");
    row("Polish with")
        .describe(
            "Only the text is ever sent, never the audio. Anthropic needs \
             ANTHROPIC_API_KEY in ~/.config/hush/.env. An OpenAI-compatible server \
             receives the text at the address you set.",
        )
        .show(ui, |ui| {
            egui::ComboBox::from_id_salt("llm_provider")
                .width(FIELD_WIDTH)
                .selected_text(provider_name(gui.config.llm.provider))
                .show_ui(ui, |ui| {
                    for provider in [
                        LlmSetting::Auto,
                        LlmSetting::Ollama,
                        LlmSetting::Anthropic,
                        LlmSetting::OpenAiCompatible,
                        LlmSetting::None,
                    ] {
                        ui.selectable_value(
                            &mut gui.config.llm.provider,
                            provider,
                            provider_name(provider),
                        );
                    }
                });
        });
    row("Tidy up every transcript")
        .describe("Otherwise the model only handles spoken commands.")
        .reserve(TOGGLE_COLUMN)
        .show(ui, |ui| {
            ui.add_enabled_ui(gui.config.llm.provider != LlmSetting::None, |ui| {
                toggle(ui, &mut gui.config.llm.polish);
            });
        });

    section(ui, "Language models");
    row("Ollama server").show(ui, |ui| {
        field(ui, &mut gui.config.llm.ollama_url, "http://localhost:11434");
    });
    row("Ollama model").show(ui, |ui| {
        field(ui, &mut gui.config.llm.ollama_model, "llama3.2");
    });
    row("Anthropic model").show(ui, |ui| {
        field(ui, &mut gui.config.llm.anthropic_model, "claude-haiku-4-5");
    });
    row("OpenAI-compatible server")
        .describe(&server_reach(&gui.config.llm.server_url))
        .show(ui, |ui| {
            field(
                ui,
                &mut gui.config.llm.server_url,
                "http://localhost:8080/v1",
            );
        });
    row("OpenAI-compatible model").show(ui, |ui| {
        field(ui, &mut gui.config.llm.server_model, "llama3.2");
    });
}

fn writing_styles(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Styles");
    hint(
        ui,
        "A style changes one recording. A style key on the Hotkeys page, \
         hush start --style NAME, or profiles.app_styles in config.toml picks it.",
    );
    row(styles::VERBATIM.as_str())
        .describe("Built in. No polish for the recording. The rule-based cleanup still runs.")
        .show(ui, |_| {});
    let names: Vec<String> = gui.config.styles.keys().cloned().collect();
    let mut remove: Option<String> = None;
    let mut change: Option<String> = None;
    for name in &names {
        let about = style_summary(&gui.config.styles[name]);
        row(name).describe(&about).show(ui, |ui| {
            if quiet_button(ui, "Remove").clicked() {
                remove = Some(name.clone());
            }
            if quiet_button(ui, "Edit").clicked() {
                change = Some(name.clone());
            }
        });
    }
    if let Some(name) = remove {
        match remove_style(&mut gui.config, &name) {
            Ok(()) => {
                if gui.style_draft.original.as_deref() == Some(name.as_str()) {
                    gui.style_draft = StyleDraft::default();
                }
            },
            Err(message) => gui.say(message),
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
    ui.add_space(6.0);
    match &editing {
        Some(name) => hint(ui, &format!("Change the style {}:", name)),
        None => hint(ui, "Add a style:"),
    }
    row("Name")
        .describe("Lowercase letters, digits, - and _.")
        .show(ui, |ui| {
            field(ui, &mut gui.style_draft.name, "casual");
        });
    row("Prompt")
        .describe("One line that Hush adds to the polish prompt.")
        .reserve(PROMPT_WIDTH + 10.0)
        .show(ui, |ui| {
            ui.add_sized(
                [PROMPT_WIDTH, PROMPT_HEIGHT],
                egui::TextEdit::multiline(&mut gui.style_draft.prompt)
                    .hint_text("Casual and brief, like a chat message to a friend."),
            );
        });
    toggle_row(
        ui,
        "Polish",
        "The model polishes recordings in this style, whatever Tidy up every transcript says.",
        &mut gui.style_draft.polish,
    );
    let ready = !gui.style_draft.name.trim().is_empty();
    let (done, cancel) = row("").reserve(FIELD_WIDTH).show(ui, |ui| {
        let label = if editing.is_some() { "Done" } else { "Add" };
        let done = ui
            .add_enabled(
                ready,
                egui::Button::new(label).min_size(egui::vec2(80.0, CONTROL_HEIGHT)),
            )
            .clicked();
        let cancel = editing.is_some() && quiet_button(ui, "Cancel").clicked();
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
    if let Some(old) = renamed {
        config.styles.remove(old);
        let uses = config
            .hotkey
            .styles
            .values_mut()
            .chain(config.profiles.app_styles.values_mut());
        for value in uses.filter(|value| value.as_str() == old) {
            *value = name.to_string();
        }
    }
    config.styles.insert(
        name.to_string(),
        StyleConfig {
            prompt: draft.prompt.trim().to_string(),
            polish: draft.polish,
        },
    );
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
    let apps = config
        .profiles
        .app_styles
        .iter()
        .filter(|(_, style)| style.as_str() == name)
        .map(|(app, _)| format!("profiles.app_styles for {}", app));
    keys.chain(apps).collect()
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
    row("Add a word")
        .describe(
            "Hush spells these exactly this way. The Learn key adds the selected \
             text; you can also type one here.",
        )
        .show(ui, |ui| {
            add |= ui
                .add_enabled(!gui.new_term.trim().is_empty(), egui::Button::new("Add"))
                .clicked();
            let response = field(ui, &mut gui.new_term, "Kubernetes");
            add |= response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
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
        ui.add_space(6.0);
        ui.label(egui::RichText::new("No words yet.").color(MUTED));
        return;
    }
    let mut remove: Option<String> = None;
    let mut start: Option<String> = None;
    let mut commit = false;
    let mut cancel = false;
    let editing = &mut gui.editing_term;
    egui::ScrollArea::vertical()
        .max_height(VOCABULARY_HEIGHT)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for term in &terms {
                match editing.as_mut().filter(|edit| edit.original == *term) {
                    Some(edit) => {
                        row(term).reserve(EDIT_COLUMN).show(ui, |ui| {
                            cancel |= quiet_button(ui, "Cancel").clicked();
                            commit |= ui.button("Done").clicked();
                            let response = field(ui, &mut edit.draft, term);
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
                        row(term).show(ui, |ui| {
                            if quiet_button(ui, "Remove").clicked() {
                                remove = Some(term.clone());
                            }
                            if quiet_button(ui, "Edit").clicked() {
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
        learn::forget_in(&mut gui.vocabulary, &term);
    }
    ui.add_space(4.0);
    let count = terms.len();
    if count == 1 {
        hint(ui, "1 word.");
    } else {
        hint(ui, &format!("{} words.", count));
    }
}

fn word_list(gui: &mut Gui, ui: &mut egui::Ui) {
    let has_path = !gui.word_list.trim().is_empty();
    let has_terms = !gui.vocabulary.technical_terms.is_empty();
    let (import, export) = row("Word list")
        .describe(
            "A text file with one word on each line. Import adds each word that \
             the vocabulary does not have. Export writes every word to a new file.",
        )
        .reserve(LIST_COLUMN)
        .show(ui, |ui| {
            let export = ui
                .add_enabled(has_path && has_terms, egui::Button::new("Export"))
                .clicked();
            let import = ui
                .add_enabled(has_path, egui::Button::new("Import"))
                .clicked();
            field(ui, &mut gui.word_list, DEFAULT_WORD_LIST);
            (import, export)
        });
    if !import && !export {
        return;
    }
    let path = list_path(&gui.word_list, &dirs::home_dir().unwrap_or_default());
    if import {
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let result = learn::import_list(&mut gui.vocabulary, &text);
                gui.say(import_message(result));
            },
            Err(e) => gui.say(format!("Could not read {}: {}", path.display(), e)),
        }
    }
    if export {
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
    if result.added > 0 {
        message.push_str(" Save to keep them.");
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
             5 words or 64 characters. Save to keep them."
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
