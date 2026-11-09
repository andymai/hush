//! What happens to the words after Whisper: tone by application, polishing
//! with a language model, and the words Hush must spell your way.

use crate::config::settings::LlmSetting;
use crate::config::{server, transfer};
use crate::gui::theme::{
    field, hint, quiet_button, row, section, toggle, toggle_row, FIELD_WIDTH, MUTED, TOGGLE_COLUMN,
};
use crate::gui::Gui;
use crate::text_processing::learn::{self, ListImport};
use std::path::{Path, PathBuf};

const VOCABULARY_HEIGHT: f32 = 220.0;
const EDIT_COLUMN: f32 = 340.0;
const LIST_COLUMN: f32 = FIELD_WIDTH + 140.0;

pub const DEFAULT_WORD_LIST: &str = "~/hush-vocabulary.txt";

/// A vocabulary term open for editing.
pub struct TermEdit {
    pub original: String,
    pub draft: String,
    /// Put the cursor in the field on the next frame
    pub focus: bool,
}

pub fn show(gui: &mut Gui, ui: &mut egui::Ui) {
    tone(gui, ui);
    polish(gui, ui);
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
