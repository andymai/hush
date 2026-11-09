//! What Hush listens with and how it turns speech into words: where the
//! transcription runs, the model, the language, the GPU, the microphone, and
//! how long a recording may run.

use crate::config::server;
use crate::config::settings::TranscriptionBackend;
use crate::gui::meter::{self, Meter};
use crate::gui::theme::{field, quiet_button, row, section, slider, toggle_row, FIELD_WIDTH, GOOD};
use crate::gui::{megabytes, Gui};
use crate::transcription::device::GpuAvailability;
use crate::transcription::models::{vad_model, ModelManager, ModelSize};
use std::sync::Arc;

/// The languages Whisper handles best, plus detection.
const LANGUAGES: &[(&str, &str)] = &[
    ("auto", "Detect it"),
    ("en", "English"),
    ("es", "Spanish"),
    ("fr", "French"),
    ("de", "German"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("nl", "Dutch"),
    ("pl", "Polish"),
    ("ru", "Russian"),
    ("uk", "Ukrainian"),
    ("tr", "Turkish"),
    ("ar", "Arabic"),
    ("hi", "Hindi"),
    ("zh", "Chinese"),
    ("ja", "Japanese"),
    ("ko", "Korean"),
];

pub fn show(gui: &mut Gui, ui: &mut egui::Ui) {
    backend(gui, ui);
    if !gui.config.transcription.backend.is_cloud() {
        models(gui, ui);
    }
    listening(gui, ui);
    microphone(gui, ui);
}

fn backend(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Transcription");
    row("Transcribe on")
        .describe(
            "This machine keeps the audio here. Groq and OpenAI can be faster on a slow \
             computer, and they receive the audio of every dictation. An OpenAI-compatible \
             server receives the audio at the address you set.",
        )
        .show(ui, |ui| {
            egui::ComboBox::from_id_salt("backend")
                .width(FIELD_WIDTH)
                .selected_text(backend_label(gui.config.transcription.backend))
                .show_ui(ui, |ui| {
                    for backend in TranscriptionBackend::ALL {
                        ui.selectable_value(
                            &mut gui.config.transcription.backend,
                            backend,
                            backend_label(backend),
                        );
                    }
                });
        });

    let backend = gui.config.transcription.backend;
    if backend == TranscriptionBackend::OpenAiCompatible {
        server_settings(gui, ui);
        return;
    }
    let Some(var) = backend.key_var() else {
        return;
    };
    let service = backend.name();
    let has_key = gui.machine.has_key(&gui.config.transcription);
    let sent = if gui.config.transcription.context_prompt {
        "the audio of each dictation and the focused window's title"
    } else {
        "the audio of each dictation"
    };
    let (title, description) = if has_key {
        (
            format!("Audio leaves this machine for {}", service),
            format!("Hush sends {} to {}.", sent, service),
        )
    } else {
        (
            format!("{} needs an API key", service),
            format!(
                "Put {} in ~/.config/hush/.env. Until then dictation fails, \
                 and Hush never falls back to another service.",
                var
            ),
        )
    };
    row(&title)
        .status(has_key)
        .describe(&description)
        .show(ui, |_| {});

    let (model, placeholder) = match backend {
        TranscriptionBackend::Groq => (
            &mut gui.config.transcription.groq_model,
            "whisper-large-v3-turbo",
        ),
        TranscriptionBackend::OpenAi => (
            &mut gui.config.transcription.openai_model,
            "gpt-4o-mini-transcribe",
        ),
        TranscriptionBackend::Local | TranscriptionBackend::OpenAiCompatible => return,
    };
    let title = format!("{} model", service);
    row(&title).show(ui, |ui| {
        field(ui, model, placeholder);
    });
}

fn server_settings(gui: &mut Gui, ui: &mut egui::Ui) {
    let (ok, title, description) = server_status(gui);
    row(&title)
        .status(ok)
        .describe(&description)
        .show(ui, |_| {});
    row("Server address")
        .describe("Hush adds /v1/audio/transcriptions when the address does not end with it.")
        .show(ui, |ui| {
            field(
                ui,
                &mut gui.config.transcription.server_url,
                "http://localhost:8080/v1",
            );
        });
    row("Server model").show(ui, |ui| {
        field(
            ui,
            &mut gui.config.transcription.server_model,
            "whisper-large-v3",
        );
    });
}

/// Whether the OpenAI-compatible server is set up, a title, and where the
/// audio goes: a local server, or a service that receives it.
pub fn server_status(gui: &Gui) -> (bool, String, String) {
    let transcription = &gui.config.transcription;
    if let Err(e) = transcription.validate_server() {
        return (
            false,
            "Set the server address and model".to_string(),
            e.to_string(),
        );
    }
    if !gui.machine.has_key(transcription) {
        let var = transcription.key_var().unwrap_or_default();
        return (
            false,
            format!("The server needs {}", var),
            format!(
                "Put {} in ~/.config/hush/.env. Until then dictation fails, \
                 and Hush never falls back to another backend.",
                var
            ),
        );
    }
    let host = server::host_label(&transcription.server_url);
    let sent = if transcription.context_prompt {
        "the audio of each dictation and the focused window's title"
    } else {
        "the audio of each dictation"
    };
    if server::is_local(&transcription.server_url) {
        (
            true,
            "A local server transcribes the audio".to_string(),
            format!(
                "Hush sends {} to {}, on this machine or your local network.",
                sent, host
            ),
        )
    } else {
        (
            true,
            format!("Audio leaves this machine for {}", host),
            format!("Hush sends {} to {}.", sent, host),
        )
    }
}

fn backend_label(backend: TranscriptionBackend) -> &'static str {
    match backend {
        TranscriptionBackend::Local => "This machine",
        TranscriptionBackend::Groq => "Groq (cloud)",
        TranscriptionBackend::OpenAi => "OpenAI (cloud)",
        TranscriptionBackend::OpenAiCompatible => "OpenAI-compatible server",
    }
}

fn models(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Model");
    if gui.catalogue.is_empty() {
        row("The models directory is not readable")
            .status(false)
            .show(ui, |_| {});
        return;
    }
    let recommended = gui.recommended;
    let sizes: Vec<ModelSize> = gui.catalogue.iter().map(|(size, _)| *size).collect();
    let selected: Option<ModelSize> = gui.config.transcription.model_size.parse().ok();
    let size_of = |size: ModelSize| -> String {
        gui.catalogue
            .iter()
            .find(|(candidate, _)| *candidate == size)
            .map(|(_, bytes)| megabytes(*bytes))
            .unwrap_or_default()
    };
    let describe = |size: ModelSize| -> String {
        let installed = gui.machine.installed_models.contains(&size);
        let bytes = size_of(size);
        if installed {
            format!("{} ({})", size_key(size), bytes)
        } else {
            format!("{} ({}, not downloaded)", size_key(size), bytes)
        }
    };
    let description = format!(
        "Runs on your machine. Larger models are more accurate and slower; {} suits this machine.",
        size_key(recommended)
    );
    row("Whisper model").describe(&description).show(ui, |ui| {
        egui::ComboBox::from_id_salt("model")
            .width(FIELD_WIDTH)
            .selected_text(
                selected
                    .map(describe)
                    .unwrap_or_else(|| gui.config.transcription.model_size.clone()),
            )
            .show_ui(ui, |ui| {
                for size in sizes {
                    ui.selectable_value(
                        &mut gui.config.transcription.model_size,
                        size_key(size).to_string(),
                        describe(size),
                    );
                }
            });
    });

    let downloading = gui.download.lock().size;
    if let Some(size) = downloading {
        let (done, total) = {
            let download = gui.download.lock();
            (download.done, download.total)
        };
        let fraction = if total > 0 {
            done as f32 / total as f32
        } else {
            0.0
        };
        let title = if gui.machine.installed_models.contains(&size) {
            "Downloading voice detection".to_string()
        } else {
            format!("Downloading {}", size_key(size))
        };
        let progress = format!("{} of {}", megabytes(done), megabytes(total));
        row(&title).describe(&progress).show(ui, |ui| {
            ui.add(
                egui::ProgressBar::new(fraction)
                    .desired_width(FIELD_WIDTH)
                    .fill(GOOD),
            );
        });
        return;
    }
    if let Some(size) = selected {
        if !gui.machine.installed_models.contains(&size) {
            let title = format!("{} is not downloaded yet", size_key(size));
            let description = format!("{} from Hugging Face, once.", size_of(size));
            row(&title)
                .status(false)
                .describe(&description)
                .show(ui, |ui| {
                    if ui.button("Download").clicked() {
                        start_download(gui, size);
                    }
                });
        } else if gui.config.transcription.vad && !gui.machine.vad_installed {
            row("Voice detection is not downloaded yet")
                .status(false)
                .describe(&format!(
                    "{} from Hugging Face, once. Until then, Whisper gets the silence in each take too.",
                    megabytes(vad_model().expected_size)
                ))
                .show(ui, |ui| {
                    if ui.button("Download").clicked() {
                        start_download(gui, size);
                    }
                });
        }
    }
}

fn listening(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Listening");
    row("Language")
        .describe(
            "Detecting the language costs a little accuracy, so name it when you mostly speak one.",
        )
        .show(ui, |ui| {
            let current = gui.config.transcription.language.clone();
            egui::ComboBox::from_id_salt("language")
                .width(FIELD_WIDTH)
                .selected_text(language_name(&current))
                .show_ui(ui, |ui| {
                    if !LANGUAGES.iter().any(|(code, _)| *code == current) {
                        ui.selectable_value(
                            &mut gui.config.transcription.language,
                            current.clone(),
                            &current,
                        );
                    }
                    for (code, name) in LANGUAGES {
                        ui.selectable_value(
                            &mut gui.config.transcription.language,
                            code.to_string(),
                            *name,
                        );
                    }
                });
        });

    let backend = gui.config.transcription.backend;
    if !backend.is_cloud() {
        let gpu = GpuAvailability::detect();
        let gpu_text = if gpu.available {
            format!("{} via {}.", gpu.device_name, gpu.gpu_type)
        } else {
            "No GPU found, so transcription runs on the CPU.".to_string()
        };
        toggle_row(
            ui,
            "Use the GPU",
            &gpu_text,
            &mut gui.config.transcription.use_gpu,
        );
    }
    if !backend.is_cloud() {
        toggle_row(
            ui,
            "Trim silence",
            "Sends Whisper only the speech in each take, so silence does not turn into invented text.",
            &mut gui.config.transcription.vad,
        );
    }
    let context_text = if backend.is_cloud() {
        let receiver = match backend {
            TranscriptionBackend::OpenAiCompatible => "the server",
            hosted => hosted.name(),
        };
        format!(
            "Sends the focused window's title and your vocabulary to {} with the audio.",
            receiver
        )
    } else {
        "Primes Whisper with the focused window's title and your vocabulary.".to_string()
    };
    toggle_row(
        ui,
        "Spell names from context",
        &context_text,
        &mut gui.config.transcription.context_prompt,
    );

    let mut minutes = (gui.config.audio.max_recording_secs / 60) as u32;
    row("Stop recording after")
        .describe("A warning sounds a minute before a hands-free recording stops.")
        .show(ui, |ui| {
            if slider(
                ui,
                egui::Slider::new(&mut minutes, 0..=30).custom_formatter(|v, _| {
                    if v == 0.0 {
                        "Never".to_string()
                    } else {
                        format!("{} min", v)
                    }
                }),
            )
            .changed()
            {
                gui.config.audio.max_recording_secs = u64::from(minutes) * 60;
            }
        });

    let mut pause = gui.config.audio.silence_stop_secs.min(10) as u32;
    row("Stop after a pause")
        .describe("Ends a hands-free recording when you stop talking for this long.")
        .show(ui, |ui| {
            if slider(
                ui,
                egui::Slider::new(&mut pause, 0..=10).custom_formatter(|v, _| {
                    if v == 0.0 {
                        "Never".to_string()
                    } else {
                        format!("{} s", v)
                    }
                }),
            )
            .changed()
            {
                gui.config.audio.silence_stop_secs = u64::from(pause);
            }
        });
}

fn microphone(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Microphone");
    ensure_meter(gui);
    row("Input device")
        .describe("The system default follows whatever your desktop picks.")
        .reserve(FIELD_WIDTH + 90.0)
        .show(ui, |ui| {
            let current = gui
                .config
                .audio
                .device
                .clone()
                .unwrap_or_else(|| meter::DEFAULT.to_string());
            egui::ComboBox::from_id_salt("microphone")
                .width(FIELD_WIDTH)
                .selected_text(&current)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut gui.config.audio.device, None, meter::DEFAULT);
                    if let Some(chosen) = gui.config.audio.device.clone() {
                        if !gui.microphones.contains(&chosen) {
                            ui.selectable_value(
                                &mut gui.config.audio.device,
                                Some(chosen.clone()),
                                &chosen,
                            );
                        }
                    }
                    for name in &gui.microphones {
                        ui.selectable_value(&mut gui.config.audio.device, Some(name.clone()), name);
                    }
                });
            if quiet_button(ui, "Refresh")
                .on_hover_text("Look for microphones again")
                .clicked()
            {
                gui.microphones = meter::input_devices();
            }
        });
    let level_text = match &gui.meter_error {
        Some(error) => error.clone(),
        None => "Say something: the bar moves when Hush hears you.".to_string(),
    };
    row("Level").describe(&level_text).show(ui, |ui| {
        let level = gui.meter.as_ref().map(Meter::level).unwrap_or(0.0);
        gui.level_shown = level.max(gui.level_shown * 0.85);
        ui.add(
            egui::ProgressBar::new(gui.level_shown)
                .desired_width(FIELD_WIDTH)
                .fill(GOOD),
        );
    });
}

/// Keep a meter open on the chosen microphone, reopening when the choice
/// changes and remembering a failure so it is not retried every frame.
fn ensure_meter(gui: &mut Gui) {
    let wanted = gui.config.audio.device.clone();
    if gui.meter.as_ref().is_some_and(|m| m.device == wanted) {
        return;
    }
    if gui.meter.is_none() && gui.meter_error.is_some() && gui.meter_wanted == wanted {
        return;
    }
    gui.meter_wanted = wanted.clone();
    match Meter::open(wanted.as_deref()) {
        Ok(meter) => {
            gui.meter = Some(meter);
            gui.meter_error = None;
        },
        Err(e) => {
            gui.meter = None;
            gui.meter_error = Some(e.to_string());
        },
    }
}

fn language_name(code: &str) -> String {
    LANGUAGES
        .iter()
        .find(|(candidate, _)| *candidate == code)
        .map(|(_, name)| (*name).to_string())
        .unwrap_or_else(|| code.to_string())
}

fn start_download(gui: &mut Gui, size: ModelSize) {
    {
        let mut download = gui.download.lock();
        download.size = Some(size);
        download.done = 0;
        download.total = 0;
        download.finished = None;
    }
    let shared = Arc::clone(&gui.download);
    let models_dir = crate::config::paths::models_dir();
    let fetch_model = !gui.machine.installed_models.contains(&size);
    let fetch_vad = gui.config.transcription.vad && !gui.machine.vad_installed;
    gui.runtime.spawn(async move {
        let result = match ModelManager::new(&models_dir) {
            Ok(manager) => {
                let progress = shared.clone();
                let report = move |done, total| {
                    let mut download = progress.lock();
                    download.done = done;
                    download.total = total;
                };
                let model = if fetch_model {
                    manager
                        .ensure_model_downloaded_with(&size, &report)
                        .await
                        .map(|_| ())
                        .map_err(|e| e.to_string())
                } else {
                    Ok(())
                };
                match model {
                    Ok(()) if fetch_vad => manager
                        .ensure_vad_downloaded_with(&report)
                        .await
                        .map(|_| ())
                        .map_err(|e| format!("the voice detection model: {}", e)),
                    other => other,
                }
            },
            Err(e) => Err(e.to_string()),
        };
        shared.lock().finished = Some(result);
    });
    gui.say("Downloading…");
}

/// The name this model has in `transcription.model_size`.
pub fn size_key(size: ModelSize) -> &'static str {
    match size {
        ModelSize::Tiny => "tiny",
        ModelSize::Base => "base",
        ModelSize::Small => "small",
        ModelSize::Medium => "medium",
        ModelSize::Large => "large",
        ModelSize::LargeV2 => "large-v2",
        ModelSize::LargeV3 => "large-v3",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::MODEL_ORDER;

    #[test]
    fn every_model_name_parses_back() {
        for size in MODEL_ORDER {
            let key = size_key(size);
            assert_eq!(
                key.parse::<ModelSize>().ok(),
                Some(size),
                "{key} round trips"
            );
        }
    }

    #[test]
    fn hosted_services_are_labelled_as_cloud() {
        for backend in TranscriptionBackend::ALL {
            assert_eq!(
                backend_label(backend).contains("(cloud)"),
                backend.key_var().is_some(),
                "{backend:?}"
            );
        }
        assert_eq!(
            backend_label(TranscriptionBackend::OpenAiCompatible),
            "OpenAI-compatible server"
        );
    }

    #[test]
    fn language_codes_show_their_name() {
        assert_eq!(language_name("de"), "German");
        assert_eq!(language_name("auto"), "Detect it");
        assert_eq!(
            language_name("cy"),
            "cy",
            "an unlisted code shows as itself"
        );
        for (code, _) in LANGUAGES {
            assert!(
                crate::config::Config::valid_language(code),
                "{code} passes validation"
            );
        }
    }
}
