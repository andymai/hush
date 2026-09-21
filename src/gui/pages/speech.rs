//! What Hush listens with and how it turns speech into words: the model, the
//! language, the GPU, the microphone, and how long a recording may run.

use crate::gui::meter::{self, Meter};
use crate::gui::theme::{quiet_button, row, section, slider, toggle_row, FIELD_WIDTH, GOOD};
use crate::gui::{megabytes, Gui};
use crate::transcription::device::GpuAvailability;
use crate::transcription::models::{ModelManager, ModelSize};
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
    models(gui, ui);
    listening(gui, ui);
    microphone(gui, ui);
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
        let title = format!("Downloading {}", size_key(size));
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
    toggle_row(
        ui,
        "Spell names from context",
        "Primes Whisper with the focused window's title and your vocabulary.",
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
    gui.runtime.spawn(async move {
        let result = match ModelManager::new(&models_dir) {
            Ok(manager) => {
                let progress = shared.clone();
                manager
                    .ensure_model_downloaded_with(&size, &move |done, total| {
                        let mut download = progress.lock();
                        download.done = done;
                        download.total = total;
                    })
                    .await
                    .map(|_| ())
                    .map_err(|e| e.to_string())
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
