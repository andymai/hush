//! What Hush listens with and how it turns speech into words: where the
//! transcription runs, the model, the language and the languages detection
//! may pick, the GPU, the microphone and whether it sleeps between takes,
//! and how long a recording may run.

use crate::audio::wake::{self, Finding, Plan, Probe, Sleep, Undone};
use crate::config::settings::{TranscriptionBackend, TranscriptionConfig};
use crate::config::{server, Config};
use crate::gui::icons;
use crate::gui::meter::{self, Meter};
use crate::gui::pages::text_setting;
use crate::gui::theme::{palette, FIELD_WIDTH};
use crate::gui::widgets::{
    busy, dropdown, icon_button, list, list_item, meter as level_meter, option, row, section,
    stepper, switch_row, Button, Variant,
};
use crate::gui::{checked_change, choose};
use crate::gui::{megabytes, Gui};
use crate::transcription::device::GpuAvailability;
use crate::transcription::models::{vad_model, ModelManager, ModelSize};
use crate::transcription::Language;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};

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
    let shown_before = gui
        .pending
        .backend
        .unwrap_or(gui.config.transcription.backend);
    let waiting = gui.pending.backend.and_then(|backend| {
        checked_change(&gui.config, |config| config.transcription.backend = backend)
            .err()
            .map(|reason| format!("Not switched yet: set the server below. {}", reason))
    });
    let mut shown = shown_before;
    row("speech.backend", "Transcribe on")
        .describe(
            "This machine keeps the audio here. Groq and OpenAI can be faster on a slow \
             computer, and they receive the audio of every dictation. An OpenAI-compatible \
             server receives the audio at the address you set.",
        )
        .problem(waiting.as_deref())
        .show(ui, |ui| {
            dropdown(ui, "backend", FIELD_WIDTH, backend_label(shown), |ui| {
                for backend in TranscriptionBackend::ALL {
                    option(ui, &mut shown, backend, backend_label(backend));
                }
            });
        });
    if shown != shown_before {
        gui.pending.backend = choose(&mut gui.config, shown, |config, backend| {
            config.transcription.backend = backend
        });
    }

    let backend = shown;
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
    row("speech.key", &title)
        .status(has_key)
        .describe(&description)
        .reserve(0.0)
        .show(ui, |_| {});

    type Slot = fn(&mut Config) -> &mut String;
    let (key, slot, placeholder): (&str, Slot, &str) = match backend {
        TranscriptionBackend::Groq => (
            "speech.groq_model",
            |config| &mut config.transcription.groq_model,
            "whisper-large-v3-turbo",
        ),
        TranscriptionBackend::OpenAi => (
            "speech.openai_model",
            |config| &mut config.transcription.openai_model,
            "gpt-4o-mini-transcribe",
        ),
        TranscriptionBackend::Local | TranscriptionBackend::OpenAiCompatible => return,
    };
    let title = format!("{} model", service);
    text_setting(gui, ui, row(key, &title), placeholder, slot);
}

fn server_settings(gui: &mut Gui, ui: &mut egui::Ui) {
    let (ok, title, description) = server_status(gui);
    row("speech.server", &title)
        .status(ok)
        .describe(&description)
        .reserve(0.0)
        .show(ui, |_| {});
    text_setting(
        gui,
        ui,
        row("speech.server_url", "Server address")
            .describe("Hush adds /v1/audio/transcriptions when the address does not end with it."),
        "http://localhost:8080/v1",
        |config| &mut config.transcription.server_url,
    );
    text_setting(
        gui,
        ui,
        row("speech.server_model", "Server model"),
        "whisper-large-v3",
        |config| &mut config.transcription.server_model,
    );
}

/// Whether the OpenAI-compatible server is set up, a title, and where the
/// audio goes: a local server, or a service that receives it.
pub fn server_status(gui: &Gui) -> (bool, String, String) {
    let mut transcription = gui.config.transcription.clone();
    if let Some(backend) = gui.pending.backend {
        transcription.backend = backend;
    }
    let transcription = &transcription;
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
        row("speech.models_dir", "The models directory is not readable")
            .status(false)
            .reserve(0.0)
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
    let description = model_description(selected, recommended);
    let before = gui.config.transcription.model_size.clone();
    row("speech.model", "Speech model")
        .describe(&description)
        .reserve(240.0)
        .show(ui, |ui| {
            let shown = selected
                .map(describe)
                .unwrap_or_else(|| gui.config.transcription.model_size.clone());
            let current = &mut gui.config.transcription.model_size;
            dropdown(ui, "model", 240.0, &shown, |ui| {
                for size in sizes {
                    option(ui, current, size_key(size).to_string(), &describe(size));
                }
            });
        });
    model_picked(&mut gui.config.transcription, &before);

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
        row("speech.download", &title)
            .describe(&progress)
            .reserve(FIELD_WIDTH + 48.0)
            .show(ui, |ui| {
                let colours = palette(ui.ctx());
                ui.label(
                    egui::RichText::new(format!("{:.0}%", fraction * 100.0))
                        .font(crate::gui::theme::mono(crate::gui::theme::MONO))
                        .color(colours.muted),
                );
                level_meter(ui, fraction, FIELD_WIDTH);
            });
        return;
    }
    if let Some(size) = selected {
        if !gui.machine.installed_models.contains(&size) {
            let title = format!("{} is not downloaded yet", size_key(size));
            let description = format!("{} from Hugging Face, once.", size_of(size));
            row("speech.download", &title)
                .status(false)
                .describe(&description)
                .reserve(120.0)
                .show(ui, |ui| {
                    if ui
                        .add(Button::new("Download").icon(icons::DOWNLOAD))
                        .clicked()
                    {
                        start_download(gui, size);
                    }
                });
        } else if gui.config.transcription.vad && !gui.machine.vad_installed {
            row("speech.download", "Voice detection is not downloaded yet")
                .status(false)
                .describe(&format!(
                    "{} from Hugging Face, once. Until then, Whisper gets the silence in each take too.",
                    megabytes(vad_model().expected_size)
                ))
                .reserve(120.0)
                .show(ui, |ui| {
                    if ui
                        .add(Button::new("Download").icon(icons::DOWNLOAD))
                        .clicked()
                    {
                        start_download(gui, size);
                    }
                });
        }
    }
}

fn listening(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Listening");
    row("speech.language", "Language")
        .describe(
            "Detecting the language costs a little accuracy, so name it when you mostly speak one.",
        )
        .show(ui, |ui| {
            language_picker(
                ui,
                "language",
                &mut gui.config.transcription.language,
                true,
                FIELD_WIDTH,
                "",
            );
        });
    detection(gui, ui);

    let backend = gui.config.transcription.backend;
    if !backend.is_cloud() {
        let gpu = GpuAvailability::detect();
        let gpu_text = if gpu.available {
            format!("{} via {}.", gpu.device_name, gpu.gpu_type)
        } else {
            "No GPU found, so transcription runs on the CPU.".to_string()
        };
        switch_row(
            ui,
            "speech.gpu",
            "Use the GPU",
            &gpu_text,
            &mut gui.config.transcription.use_gpu,
        );
    }
    if !backend.is_cloud() {
        switch_row(
            ui,
            "speech.vad",
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
    switch_row(
        ui,
        "speech.context",
        "Spell names from context",
        &context_text,
        &mut gui.config.transcription.context_prompt,
    );

    let mut minutes = (gui.config.audio.max_recording_secs / 60).min(MAX_MINUTES);
    row("speech.max_recording", "Stop recording after")
        .describe("A warning sounds a minute before a hands-free recording stops.")
        .reserve(130.0)
        .show(ui, |ui| {
            if stepper(ui, &mut minutes, 0..=MAX_MINUTES, |n| never_or(n, "min")).changed() {
                gui.config.audio.max_recording_secs = minutes * 60;
            }
        });

    let mut pause = gui.config.audio.silence_stop_secs.min(MAX_PAUSE);
    row("speech.silence_stop", "Stop after a pause")
        .describe("Ends a hands-free recording when you stop talking for this long.")
        .reserve(130.0)
        .show(ui, |ui| {
            if stepper(ui, &mut pause, 0..=MAX_PAUSE, |n| never_or(n, "s")).changed() {
                gui.config.audio.silence_stop_secs = pause;
            }
        });
}

/// The longest cap the stepper sets, in minutes, and the longest pause, in
/// seconds.
const MAX_MINUTES: u64 = 30;
const MAX_PAUSE: u64 = 10;

/// `0` is Never; any other number gets its unit.
fn never_or(value: u64, unit: &str) -> String {
    if value == 0 {
        "Never".to_string()
    } else {
        format!("{} {}", value, unit)
    }
}

/// The languages that detection may pick. The row shows while the language
/// is Detect it, and while the list holds a language to remove.
fn detection(gui: &mut Gui, ui: &mut egui::Ui) {
    let transcription = &gui.config.transcription;
    if transcription.language != "auto" && transcription.detect_languages.is_empty() {
        return;
    }
    let description = if transcription.backend.is_cloud() {
        "Only Whisper on this machine uses this list. The service detects the language on its own."
    } else {
        "With Detect it, a language-ID model picks the most likely of these languages, and \
         Whisper writes in it. The daemon downloads the model (27 MB) when it needs it. With \
         none, Whisper picks from every language."
    };
    let mut added = String::new();
    row("speech.detect_languages", "Detect only")
        .describe(description)
        .show(ui, |ui| {
            language_picker(
                ui,
                "detect_add",
                &mut added,
                false,
                FIELD_WIDTH,
                "Add a language",
            );
        });
    let languages = &mut gui.config.transcription.detect_languages;
    if !added.is_empty() && !languages.contains(&added) {
        languages.push(added);
    }
    let mut remove = None;
    list(ui, |ui| {
        for (index, code) in languages.iter().enumerate() {
            let name = language_name(code);
            let key = format!("speech.detect_languages.{}", code);
            list_item(&key, icons::LANGUAGES, &name).show(ui, 24.0, |ui| {
                if icon_button(ui, icons::X, &format!("Remove {}", name)).clicked() {
                    remove = Some(index);
                }
            });
        }
    });
    if let Some(index) = remove {
        let name = language_name(&gui.config.transcription.detect_languages[index]);
        gui.remove(format!("Removed {}", name), |gui| {
            gui.config.transcription.detect_languages.remove(index);
            true
        });
    }
}

/// A drop-down of the languages Whisper handles best, plus `value` when it
/// is another language. `with_auto` offers detection. An empty `value`
/// shows `placeholder`.
pub fn language_picker(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    value: &mut String,
    with_auto: bool,
    width: f32,
    placeholder: &str,
) {
    let current = value.clone();
    let shown = if current.is_empty() {
        placeholder.to_string()
    } else {
        language_name(&current)
    };
    dropdown(ui, id_salt, width, &shown, |ui| {
        if !current.is_empty() && !LANGUAGES.iter().any(|(code, _)| *code == current) {
            option(ui, value, current.clone(), &language_name(&current));
        }
        for (code, name) in LANGUAGES
            .iter()
            .filter(|(code, _)| with_auto || *code != "auto")
        {
            option(ui, value, code.to_string(), name);
        }
    });
}

fn microphone(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Microphone");
    if !gui.screenshot_mode() {
        ensure_meter(gui);
        refresh_wake(gui, ui.ctx());
    }
    let problem = wake_finding(gui)
        .filter(|finding| finding.warn)
        .map(|finding| format!("{}.", finding.detail));
    row("speech.microphone", "Input device")
        .describe("The system default follows whatever your desktop picks.")
        .caution(problem.as_deref())
        .reserve(FIELD_WIDTH + 32.0)
        .show(ui, |ui| {
            if icon_button(ui, icons::REFRESH_CW, "Look for microphones again").clicked() {
                gui.microphones = meter::input_devices();
            }
            let shown = gui
                .config
                .audio
                .device
                .clone()
                .unwrap_or_else(|| meter::DEFAULT.to_string());
            let device = &mut gui.config.audio.device;
            let microphones = &gui.microphones;
            dropdown(ui, "microphone", FIELD_WIDTH, &shown, |ui| {
                option(ui, device, None, meter::DEFAULT);
                if let Some(chosen) = device.clone() {
                    if !microphones.contains(&chosen) {
                        option(ui, device, Some(chosen.clone()), &chosen);
                    }
                }
                for name in microphones {
                    option(ui, device, Some(name.clone()), name);
                }
            });
        });
    wake_row(gui, ui);
    let level_text = match &gui.meter_error {
        Some(error) => error.clone(),
        None => "Say something: the bar moves when Hush hears you.".to_string(),
    };
    row("speech.level", "Level")
        .describe(&level_text)
        .show(ui, |ui| {
            let level = gui.meter.as_ref().map(Meter::level).unwrap_or(0.0);
            gui.level_shown = level.max(gui.level_shown * 0.85);
            level_meter(ui, gui.level_shown, FIELD_WIDTH);
        });
}

/// How often the page reads the PipeWire node of the microphone again.
const WAKE_REFRESH: Duration = Duration::from_secs(15);

const RESTART_CAUTION: &str =
    "WirePlumber restarts to read the change, so all sound stops for about a second.";

/// Whether the microphone sleeps between takes, read in the background, and
/// the action of the wake row.
#[derive(Default)]
pub struct WakeView {
    shared: Arc<Mutex<WakeShared>>,
    /// The action that waits for its second click
    pub confirm: Option<WakeAction>,
    /// When the page last asked for a read, and for which `audio.device`
    asked: Option<(Instant, Option<String>)>,
}

#[derive(Default)]
struct WakeShared {
    probe: Option<Probe>,
    /// The rule of Hush names the source of the probe
    hush_rule: bool,
    /// Hush has a rule file, for any source
    any_rule: bool,
    reading: bool,
    running: Option<WakeAction>,
    /// What the last action did, for a toast
    finished: Option<String>,
}

/// The two buttons of the wake row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeAction {
    /// Write the rule that keeps the microphone awake
    Keep,
    /// Remove the rule of Hush
    Release,
}

/// The finding of `hush doctor` for the microphone, after the first read.
fn wake_finding(gui: &Gui) -> Option<Finding> {
    let shared = gui.wake.shared.lock();
    let probe = shared.probe.as_ref()?;
    Some(wake::finding(
        probe,
        shared.hush_rule,
        gui.machine.start_delay,
    ))
}

/// The action that the wake row offers now: Keep it awake when `hush setup
/// mic` fixes a warning, else Let it sleep again when a rule of Hush exists.
pub fn wake_offer(gui: &Gui) -> Option<WakeAction> {
    let finding = wake_finding(gui);
    let shared = gui.wake.shared.lock();
    if shared.running.is_some() {
        return None;
    }
    if finding.is_some_and(|finding| finding.warn && finding.fixable) {
        Some(WakeAction::Keep)
    } else if shared.any_rule {
        Some(WakeAction::Release)
    } else {
        None
    }
}

/// Read the node of the microphone again when the choice changed or the last
/// read is old. `pw-dump` and the ALSA device list stay off the draw thread.
fn refresh_wake(gui: &mut Gui, ctx: &egui::Context) {
    let device = gui.config.audio.device.clone();
    let due = gui
        .wake
        .asked
        .as_ref()
        .is_none_or(|(at, asked)| *asked != device || at.elapsed() >= WAKE_REFRESH);
    {
        let mut shared = gui.wake.shared.lock();
        if !due || shared.reading || shared.running.is_some() {
            return;
        }
        shared.reading = true;
    }
    gui.wake.asked = Some((Instant::now(), device.clone()));
    let shared = Arc::clone(&gui.wake.shared);
    let ctx = ctx.clone();
    gui.runtime.spawn(async move {
        let probe = wake::look(device.as_deref()).await;
        let (hush_rule, any_rule) = match wake::config_dir() {
            Ok(dir) => (
                matches!(&probe, Probe::Found(source) if wake::rule_names(&dir, source)),
                !wake::rules_in(&dir).is_empty(),
            ),
            Err(_) => (false, false),
        };
        {
            let mut state = shared.lock();
            state.probe = Some(probe);
            state.hush_rule = hush_rule;
            state.any_rule = any_rule;
            state.reading = false;
        }
        ctx.request_repaint();
    });
}

/// Run `action` in the background: the same code as `hush setup mic`, after
/// the second click.
fn run_wake(gui: &mut Gui, action: WakeAction, ctx: &egui::Context) {
    gui.wake.confirm = None;
    {
        let mut shared = gui.wake.shared.lock();
        if shared.running.is_some() {
            return;
        }
        shared.running = Some(action);
    }
    let device = gui.config.audio.device.clone();
    let shared = Arc::clone(&gui.wake.shared);
    let ctx = ctx.clone();
    gui.runtime.spawn(async move {
        let message = match action {
            WakeAction::Keep => keep_awake(device.as_deref())
                .await
                .unwrap_or_else(|e| format!("Could not keep it awake: {:#}", e)),
            WakeAction::Release => let_sleep(device.as_deref())
                .await
                .unwrap_or_else(|e| format!("Could not remove the rule: {:#}", e)),
        };
        {
            let mut state = shared.lock();
            state.running = None;
            state.finished = Some(message);
        }
        ctx.request_repaint();
    });
}

async fn keep_awake(device: Option<&str>) -> anyhow::Result<String> {
    let dir = wake::config_dir()?;
    let probe = wake::look(device).await;
    match wake::plan(&wake::Host, &dir, probe).await? {
        Plan::Awake { source, .. } => Ok(format!("{} stays awake already.", source.label)),
        Plan::Write(change) => {
            let source = wake::apply(&wake::Host, &change).await?;
            Ok(format!("{} stays awake between takes now.", source.label))
        },
    }
}

async fn let_sleep(device: Option<&str>) -> anyhow::Result<String> {
    let dir = wake::config_dir()?;
    let probe = wake::look(device).await;
    Ok(match wake::undo(&wake::Host, &dir, probe).await? {
        Undone::Nothing => "Hush has no rule to remove.".to_string(),
        Undone::Removed {
            source: Some(source),
            ..
        } if source.sleep == Sleep::Never => format!(
            "The rule of Hush is gone. Another WirePlumber rule keeps {} awake.",
            source.label
        ),
        Undone::Removed {
            source: Some(source),
            ..
        } => format!("{} sleeps between takes again.", source.label),
        Undone::Removed { source: None, .. } => "The rule of Hush is gone.".to_string(),
    })
}

/// Show what the last action did. WirePlumber restarted, so the meter opens
/// its stream again and the page reads the node again.
pub fn finish_wake(gui: &mut Gui) {
    let message = {
        let mut shared = gui.wake.shared.lock();
        let Some(message) = shared.finished.take() else {
            return;
        };
        shared.probe = None;
        message
    };
    gui.say(message);
    gui.meter = None;
    gui.meter_error = None;
    gui.wake.asked = None;
}

/// Keep it awake when the microphone sleeps and wakes slowly, Let it sleep
/// again when a rule of Hush exists. Each asks a second time, because
/// WirePlumber restarts.
fn wake_row(gui: &mut Gui, ui: &mut egui::Ui) {
    if gui.wake.shared.lock().running.is_some() {
        row("speech.wake", "WirePlumber restarts")
            .describe("All sound stops for about a second.")
            .reserve(140.0)
            .show(ui, |ui| busy(ui, "Please wait"));
        return;
    }
    let Some(action) = wake_offer(gui) else {
        return;
    };
    let confirming = gui.wake.confirm == Some(action);
    let (title, description, ok, label, icon, stay) = match action {
        WakeAction::Keep => (
            "Keep the microphone awake",
            "A WirePlumber rule stops this microphone from sleeping between takes, so a take \
             keeps its first word. No app records while it is awake.",
            false,
            "Keep it awake",
            icons::MIC,
            "Not now",
        ),
        WakeAction::Release => (
            "Hush keeps a microphone awake",
            "A WirePlumber rule of Hush stops the microphone from sleeping between takes. \
             Remove the rule to let it sleep again.",
            true,
            "Let it sleep again",
            icons::ROTATE_CCW,
            "Keep it awake",
        ),
    };
    let mut run = false;
    row("speech.wake", title)
        .status(ok)
        .describe(description)
        .caution(confirming.then_some(RESTART_CAUTION))
        .reserve(if confirming { 320.0 } else { 170.0 })
        .show(ui, |ui| {
            if !confirming {
                if ui.add(Button::new(label).icon(icon)).clicked() {
                    gui.wake.confirm = Some(action);
                }
                return;
            }
            let restart = Button::new("Restart WirePlumber")
                .icon(icons::ROTATE_CW)
                .variant(Variant::Primary);
            if ui.add(restart).clicked() {
                run = true;
            }
            if ui.add(Button::new(stay).focus_on_jump()).clicked() {
                gui.wake.confirm = None;
            }
        });
    if run {
        run_wake(gui, action, ui.ctx());
    }
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
        .map(|(_, name)| *name)
        .or_else(|| Language::parse(code).map(Language::name))
        .unwrap_or(code)
        .to_string()
}

/// The model that the Download button on this page fetches, when the page
/// shows the button: the chosen model, or voice detection for it.
pub fn download_offer(gui: &Gui) -> Option<ModelSize> {
    if gui.config.transcription.backend.is_cloud()
        || gui.catalogue.is_empty()
        || gui.download.lock().size.is_some()
    {
        return None;
    }
    let size: ModelSize = gui.config.transcription.model_size.parse().ok()?;
    let installed = gui.machine.installed_models.contains(&size);
    let vad_missing = gui.config.transcription.vad && !gui.machine.vad_installed;
    (!installed || vad_missing).then_some(size)
}

pub fn start_download(gui: &mut Gui, size: ModelSize) {
    let fetch_model = !gui.machine.installed_models.contains(&size);
    {
        let mut download = gui.download.lock();
        download.size = Some(size);
        download.fetches_model = fetch_model;
        download.done = 0;
        download.total = 0;
        download.finished = None;
    }
    let shared = Arc::clone(&gui.download);
    let models_dir = crate::config::paths::models_dir();
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
        ModelSize::ParakeetV3 => "parakeet-v3",
    }
}

/// A model picked from the list, unlike the model that was `before`, replaces
/// a `model_path` set by hand, which would load in its place.
fn model_picked(transcription: &mut TranscriptionConfig, before: &str) {
    if transcription.model_size == before {
        return;
    }
    if let Ok(size) = transcription.model_size.parse() {
        transcription.select_model(size);
    }
}

/// After a download, the model it `fetched`, if any, becomes the model when
/// another model is set now. A download of voice detection alone, or of the
/// model that is set, leaves a `model_path` set by hand in place.
pub fn model_downloaded(transcription: &mut TranscriptionConfig, fetched: Option<ModelSize>) {
    let Some(size) = fetched else {
        return;
    };
    if transcription.model_size.parse::<ModelSize>().ok() != Some(size) {
        transcription.select_model(size);
    }
}

/// What the model row says about the chosen model. Parakeet's licence asks
/// for the credit.
fn model_description(selected: Option<ModelSize>, recommended: ModelSize) -> String {
    if selected.is_some_and(|size| size.is_parakeet()) {
        return "NVIDIA Parakeet TDT 0.6B v3 (CC BY 4.0), converted to GGUF and quantized to \
                Q8_0. It knows 25 European languages and picks among them on its own, so the \
                language setting does not apply. It reads no prompt: the window title, learned \
                terms, and the text before a part do not prime it."
            .to_string();
    }
    format!(
        "Runs on your machine. Larger Whisper models are more accurate and slower; {} suits \
         this machine. parakeet-v3 is NVIDIA's Parakeet: fast and accurate in 25 European \
         languages, without priming.",
        size_key(recommended)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::MODEL_ORDER;

    fn slow_yeti() -> wake::Source {
        wake::Source {
            name: "alsa_input.usb-Blue_Microphones_Yeti_Stereo_Microphone_REV8-00.iec958-stereo"
                .to_string(),
            label: "Yeti Stereo Microphone".to_string(),
            profile: Some("iec958-stereo".to_string()),
            serial: Some(1),
            kind: wake::Kind::Alsa,
            sleep: Sleep::After(5),
        }
    }

    #[test]
    fn the_wake_row_offers_keep_for_a_sleeping_source_and_release_for_a_rule() {
        let mut gui = crate::gui::tests::window();
        assert_eq!(wake_offer(&gui), None, "before the first read");
        assert!(wake_finding(&gui).is_none());

        gui.wake.shared.lock().probe = Some(Probe::Found(slow_yeti()));
        assert_eq!(wake_offer(&gui), Some(WakeAction::Keep), "nothing measured");
        gui.machine.start_delay = Some(crate::audio::take::StartDelay {
            last_ms: 700,
            worst_ms: 700,
            takes: 1,
        });
        assert_eq!(wake_offer(&gui), Some(WakeAction::Keep));
        assert!(wake_finding(&gui).unwrap().detail.contains("700 ms"));

        gui.machine.start_delay = Some(crate::audio::take::StartDelay {
            last_ms: 8,
            worst_ms: 40,
            takes: 6,
        });
        assert_eq!(wake_offer(&gui), None, "it wakes fast");
        gui.wake.shared.lock().any_rule = true;
        assert_eq!(wake_offer(&gui), Some(WakeAction::Release));
        gui.wake.shared.lock().running = Some(WakeAction::Release);
        assert_eq!(wake_offer(&gui), None, "an action runs");
    }

    #[test]
    fn the_wake_row_draws_each_offer_and_its_second_click() {
        let mut gui = crate::gui::tests::window();
        gui.wake.shared.lock().probe = Some(Probe::Found(slow_yeti()));
        let ctx = egui::Context::default();
        crate::gui::theme::install(&ctx);
        let draw = |gui: &mut Gui| {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| wake_row(gui, ui));
            });
        };
        for confirm in [None, Some(WakeAction::Keep)] {
            gui.wake.confirm = confirm;
            draw(&mut gui);
        }
        gui.wake.shared.lock().probe = None;
        gui.wake.shared.lock().any_rule = true;
        for confirm in [None, Some(WakeAction::Release)] {
            gui.wake.confirm = confirm;
            draw(&mut gui);
        }
        gui.wake.shared.lock().running = Some(WakeAction::Release);
        draw(&mut gui);
    }

    #[test]
    fn a_page_change_drops_the_second_click_of_the_wake_row() {
        let mut gui = crate::gui::tests::window();
        gui.wake.confirm = Some(WakeAction::Keep);
        gui.go_to(crate::gui::pages::Page::Text);
        assert_eq!(gui.wake.confirm, None);
    }

    #[test]
    fn a_finished_action_says_so_and_reads_the_microphone_again() {
        let mut gui = crate::gui::tests::window();
        gui.wake.asked = Some((Instant::now(), None));
        gui.meter_error = Some("busy".to_string());
        gui.wake.shared.lock().finished =
            Some("Yeti Stereo Microphone stays awake between takes now.".to_string());
        finish_wake(&mut gui);
        assert_eq!(
            gui.toast.as_ref().map(|(message, _)| message.as_str()),
            Some("Yeti Stereo Microphone stays awake between takes now.")
        );
        assert!(gui.wake.asked.is_none(), "the next frame reads again");
        assert!(wake_finding(&gui).is_none(), "the old read is gone");
        assert!(gui.meter_error.is_none(), "the meter opens again");
        finish_wake(&mut gui);
    }

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
    fn only_a_new_pick_clears_a_model_path_set_by_hand() {
        let mut transcription = crate::Config::defaults().unwrap().transcription;
        transcription.model_path = Some("/elsewhere/ggml-base.bin".into());
        model_picked(&mut transcription, "base");
        assert!(
            transcription.model_path.is_some(),
            "the same model keeps the path"
        );

        transcription.model_size = "parakeet-v3".to_string();
        model_picked(&mut transcription, "base");
        assert_eq!(transcription.model_size, "parakeet-v3");
        assert_eq!(transcription.model_path, None);
    }

    #[test]
    fn only_a_download_of_another_model_clears_a_model_path_set_by_hand() {
        let mut transcription = crate::Config::defaults().unwrap().transcription;
        transcription.model_path = Some("/elsewhere/ggml-base.bin".into());
        model_downloaded(&mut transcription, None);
        assert!(transcription.model_path.is_some(), "voice detection alone");
        model_downloaded(&mut transcription, Some(ModelSize::Base));
        assert!(transcription.model_path.is_some(), "the model that is set");

        model_downloaded(&mut transcription, Some(ModelSize::ParakeetV3));
        assert_eq!(transcription.model_size, "parakeet-v3");
        assert_eq!(transcription.model_path, None);
    }

    #[test]
    fn the_parakeet_row_credits_nvidia_and_names_its_limits() {
        let parakeet = model_description(Some(ModelSize::ParakeetV3), ModelSize::Base);
        for words in [
            "NVIDIA",
            "CC BY 4.0",
            "converted to GGUF and quantized to Q8_0",
            "no prompt",
        ] {
            assert!(parakeet.contains(words), "{words}: {parakeet}");
        }
        let whisper = model_description(Some(ModelSize::Base), ModelSize::Small);
        assert!(whisper.contains("small suits this machine"), "{whisper}");
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
            "Welsh",
            "another Whisper language shows its name"
        );
        assert_eq!(language_name("xx"), "xx", "an unknown code shows as itself");
        for (code, _) in LANGUAGES {
            assert!(
                crate::config::Config::valid_language(code),
                "{code} passes validation"
            );
        }
    }
}
