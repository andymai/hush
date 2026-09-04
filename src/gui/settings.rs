//! Every setting the configuration file carries, in the order a person meets
//! them: the key you hold, what it does with what you said, and where the text
//! goes.

use super::theme::{hint, section};
use super::Gui;
use crate::config::settings::LlmSetting;
use crate::hotkey::{HotkeyBackend, HotkeyMode, KeyCombination};
use crate::text::InsertionMethod;

pub fn show(gui: &mut Gui, ui: &mut egui::Ui) {
    hotkey(gui, ui);
    dictation(gui, ui);
    text(gui, ui);
    insertion(gui, ui);
    desktop(gui, ui);
}

/// Red text under a hotkey field that does not parse.
fn combination_error(text: &str) -> Option<String> {
    if text.trim().is_empty() {
        return None;
    }
    KeyCombination::parse(text).err().map(|e| e.to_string())
}

fn key_field(ui: &mut egui::Ui, label: &str, value: &mut String, hint_text: &str) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::TextEdit::singleline(value).desired_width(180.0));
    });
    if let Some(error) = combination_error(value) {
        ui.colored_label(egui::Color32::from_rgb(0xd9, 0x3c, 0x33), error);
    } else {
        hint(ui, hint_text);
    }
}

fn hotkey(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "The key you hold");
    key_field(
        ui,
        "Hotkey",
        &mut gui.config.hotkey.combination,
        "A bare modifier such as RightAlt never reaches other applications. \
         F13 to F24 and Mouse4 to Mouse8 work too.",
    );

    ui.horizontal(|ui| {
        ui.label("Pressing it");
        egui::ComboBox::from_id_salt("hotkey_mode")
            .selected_text(match gui.config.hotkey.mode {
                HotkeyMode::Hold => "records while held",
                HotkeyMode::Toggle => "starts and stops",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut gui.config.hotkey.mode,
                    HotkeyMode::Hold,
                    "records while held",
                );
                ui.selectable_value(
                    &mut gui.config.hotkey.mode,
                    HotkeyMode::Toggle,
                    "starts and stops",
                );
            });
    });
    hint(
        ui,
        "Either way, a double tap keeps recording hands-free until you press again.",
    );

    key_field(
        ui,
        "Discard",
        &mut gui.config.hotkey.cancel,
        "Throws away the recording in progress. Leave empty for none.",
    );
    key_field(
        ui,
        "Command",
        &mut gui.config.hotkey.command,
        "Hold, say what to change, release: the text you selected is rewritten.",
    );

    ui.collapsing("More keys", |ui| {
        key_field(
            ui,
            "Type last again",
            &mut gui.config.hotkey.paste_last,
            "Types the last transcript once more.",
        );
        key_field(
            ui,
            "Learn selection",
            &mut gui.config.hotkey.learn,
            "Adds the selected word to the vocabulary with its exact spelling.",
        );
        ui.checkbox(
            &mut gui.config.hotkey.exclusive,
            "Keep the key from other applications",
        );
        hint(
            ui,
            "Grabs the device the key belongs to. Needed for mouse buttons, \
             which browsers otherwise treat as Back.",
        );
        ui.horizontal(|ui| {
            ui.label("Read the key through");
            egui::ComboBox::from_id_salt("hotkey_backend")
                .selected_text(match gui.config.hotkey.backend {
                    HotkeyBackend::Auto => "whatever works",
                    HotkeyBackend::Evdev => "the input devices",
                    HotkeyBackend::X11 => "X11",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut gui.config.hotkey.backend,
                        HotkeyBackend::Auto,
                        "whatever works",
                    );
                    ui.selectable_value(
                        &mut gui.config.hotkey.backend,
                        HotkeyBackend::Evdev,
                        "the input devices",
                    );
                    ui.selectable_value(&mut gui.config.hotkey.backend, HotkeyBackend::X11, "X11");
                });
        });
        ui.add(
            egui::Slider::new(&mut gui.config.hotkey.tap_ms, 100..=1000)
                .suffix(" ms")
                .text("Counts as a tap under"),
        );
    });
}

fn dictation(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Listening");
    ui.horizontal(|ui| {
        ui.label("Language");
        ui.add(
            egui::TextEdit::singleline(&mut gui.config.transcription.language).desired_width(70.0),
        );
        ui.checkbox(&mut gui.config.transcription.use_gpu, "Use the GPU");
    });
    hint(
        ui,
        "A language code such as en, de, or fr. Set auto to detect it.",
    );

    let mut minutes = gui.config.audio.max_recording_secs as f32 / 60.0;
    if ui
        .add(
            egui::Slider::new(&mut minutes, 0.0..=30.0)
                .suffix(" min")
                .text("Stop recording after"),
        )
        .changed()
    {
        gui.config.audio.max_recording_secs = (minutes * 60.0).round() as u64;
    }
    hint(ui, "A warning sounds a minute before. Zero means never.");
    ui.checkbox(
        &mut gui.config.transcription.context_prompt,
        "Use the window title and learned words to spell names",
    );
}

fn text(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "The text it writes");
    ui.checkbox(
        &mut gui.config.profiles.enabled,
        "Match the application you are typing into",
    );
    hint(
        ui,
        "Lower case and no full stop in terminals and editors, casual in chat, \
         full sentences in mail and documents.",
    );

    ui.horizontal(|ui| {
        ui.label("Polish with");
        egui::ComboBox::from_id_salt("llm_provider")
            .selected_text(match gui.config.llm.provider {
                LlmSetting::Auto => "Ollama, else Anthropic",
                LlmSetting::Ollama => "Ollama",
                LlmSetting::Anthropic => "Anthropic",
                LlmSetting::None => "nothing",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut gui.config.llm.provider,
                    LlmSetting::Auto,
                    "Ollama, else Anthropic",
                );
                ui.selectable_value(&mut gui.config.llm.provider, LlmSetting::Ollama, "Ollama");
                ui.selectable_value(
                    &mut gui.config.llm.provider,
                    LlmSetting::Anthropic,
                    "Anthropic",
                );
                ui.selectable_value(&mut gui.config.llm.provider, LlmSetting::None, "nothing");
            });
    });
    hint(
        ui,
        "Only the text is ever sent, never the audio. Anthropic needs \
         ANTHROPIC_API_KEY in ~/.config/hush/.env.",
    );
    ui.checkbox(
        &mut gui.config.llm.polish,
        "Tidy up every transcript, not just spoken commands",
    );

    ui.collapsing("Models", |ui| {
        ui.horizontal(|ui| {
            ui.label("Ollama");
            ui.add(egui::TextEdit::singleline(&mut gui.config.llm.ollama_url).desired_width(190.0));
            ui.add(
                egui::TextEdit::singleline(&mut gui.config.llm.ollama_model).desired_width(120.0),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Anthropic");
            ui.add(
                egui::TextEdit::singleline(&mut gui.config.llm.anthropic_model)
                    .desired_width(190.0),
            );
        });
    });
}

fn insertion(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "How it types");
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("insertion_method")
            .selected_text(match gui.config.insertion.method {
                InsertionMethod::Auto => "Type it, paste when it cannot",
                InsertionMethod::Uinput => "Always type it",
                InsertionMethod::Clipboard => "Always paste it",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut gui.config.insertion.method,
                    InsertionMethod::Auto,
                    "Type it, paste when it cannot",
                );
                ui.selectable_value(
                    &mut gui.config.insertion.method,
                    InsertionMethod::Uinput,
                    "Always type it",
                );
                ui.selectable_value(
                    &mut gui.config.insertion.method,
                    InsertionMethod::Clipboard,
                    "Always paste it",
                );
            });
    });
    hint(
        ui,
        "Typing works everywhere, including password fields and games. \
         Pasting handles emoji, other alphabets, and very long text.",
    );
    ui.add(
        egui::Slider::new(&mut gui.config.insertion.typing_delay_ms, 0..=50)
            .suffix(" ms")
            .text("Between keystrokes"),
    );
}

fn desktop(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Desktop");
    ui.checkbox(&mut gui.config.tray.enabled, "Show the panel icon");
    ui.checkbox(
        &mut gui.config.feedback.audio_enabled,
        "Play a sound when recording starts and stops",
    );
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.button("Open the configuration file").clicked() {
            let path = crate::config::paths::config_path();
            if !path.exists() {
                let _ = gui.config.save_to_file(&path);
            }
            if let Err(e) = std::process::Command::new("xdg-open").arg(&path).spawn() {
                gui.say(format!("Could not open it: {}", e));
            }
        }
        if ui.button("Reset everything").clicked() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_unparseable_keys_are_flagged() {
        assert!(combination_error("RightAlt").is_none());
        assert!(combination_error("").is_none(), "empty means unbound");
        assert!(combination_error("  ").is_none());
        assert!(combination_error("Ctrl+Nope").is_some());
    }
}
