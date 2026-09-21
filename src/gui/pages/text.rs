//! What happens to the words after Whisper: tone by application, polishing
//! with a language model, and the words Hush must spell your way.

use crate::config::settings::LlmSetting;
use crate::gui::theme::{
    field, hint, quiet_button, row, section, toggle, toggle_row, FIELD_WIDTH, MUTED, TOGGLE_COLUMN,
};
use crate::gui::Gui;
use crate::text_processing::learn;

const VOCABULARY_HEIGHT: f32 = 220.0;

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
             ANTHROPIC_API_KEY in ~/.config/hush/.env.",
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

    let mut terms: Vec<String> = gui.vocabulary.technical_terms.values().cloned().collect();
    terms.sort_by_key(|term| term.to_lowercase());
    if terms.is_empty() {
        ui.add_space(6.0);
        ui.label(egui::RichText::new("No words yet.").color(MUTED));
        return;
    }
    let mut remove: Option<String> = None;
    egui::ScrollArea::vertical()
        .max_height(VOCABULARY_HEIGHT)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for term in &terms {
                row(term).show(ui, |ui| {
                    if quiet_button(ui, "Remove").clicked() {
                        remove = Some(term.clone());
                    }
                });
            }
        });
    if let Some(term) = remove {
        gui.vocabulary
            .technical_terms
            .retain(|_, value| *value != term);
    }
    ui.add_space(4.0);
    let count = terms.len();
    if count == 1 {
        hint(ui, "1 word.");
    } else {
        hint(ui, &format!("{} words.", count));
    }
}

fn provider_name(provider: LlmSetting) -> &'static str {
    match provider {
        LlmSetting::Auto => "Ollama, else Anthropic",
        LlmSetting::Ollama => "Ollama",
        LlmSetting::Anthropic => "Anthropic",
        LlmSetting::None => "Nothing",
    }
}
