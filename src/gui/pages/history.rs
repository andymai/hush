//! What Hush typed, newest first, to copy again or delete, and the audio of
//! each take when Hush keeps it, to play or to transcribe again.

use crate::audio::{file, store};
use crate::config::settings::MAX_KEEP_AUDIO_DAYS;
use crate::gui::theme::{field, hint, quiet_button, row, toggle, toggle_row, BAD, MUTED};
use crate::gui::Gui;
use crate::text_processing::transcripts::{self, Transcript};
use anyhow::{Context, Result};
use parking_lot::Mutex;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player};
use std::collections::HashSet;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// Entries drawn before "Show more"; each one is a wrapped label.
const PAGE: usize = 50;
/// A longer transcript shows its start; Copy still copies all of it.
const PREVIEW_CHARS: usize = 400;
const ENTRY_CONTROLS: f32 = 110.0;
const AUDIO_CONTROLS: f32 = 120.0;
const TOOLBAR_CONTROLS: f32 = 310.0;
const RETENTION_CONTROLS: f32 = 190.0;
/// The days that the switch sets when no other number was chosen.
const DEFAULT_KEEP_DAYS: u64 = 30;
const BUSY_REPAINT: Duration = Duration::from_millis(250);

/// The take audio that plays now.
struct Playback {
    name: String,
    player: Player,
    _sink: MixerDeviceSink,
}

/// A take that `hush transcribe` transcribes again, and its result.
struct Again {
    name: String,
    result: Arc<Mutex<Option<Result<String, String>>>>,
}

/// The history file as last read, and how the page shows it.
#[derive(Default)]
pub struct HistoryView {
    entries: Vec<Transcript>,
    /// The file names in the take audio folder at the last read
    audio: HashSet<String>,
    loaded: bool,
    stamp: Option<(SystemTime, u64)>,
    error: Option<String>,
    filter: String,
    shown: usize,
    confirm_clear: bool,
    /// The last number of days chosen, for the switch to restore
    days: u64,
    playback: Option<Playback>,
    again: Option<Again>,
}

impl HistoryView {
    /// Read the file again when its time or size changed since the last read.
    fn refresh(&mut self) {
        let path = transcripts::default_path();
        let stamp = std::fs::metadata(&path)
            .ok()
            .and_then(|meta| Some((meta.modified().ok()?, meta.len())));
        if self.loaded && stamp == self.stamp {
            return;
        }
        self.loaded = true;
        self.stamp = stamp;
        match transcripts::read(&path) {
            Ok(entries) => {
                self.entries = entries;
                self.error = None;
            },
            Err(e) => self.error = Some(format!("Could not read the history: {}", e)),
        }
        self.audio = store::list(&store::takes_dir())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|kept| Some(kept.path.file_name()?.to_string_lossy().into_owned()))
            .collect();
    }

    /// Read the file again at the next frame.
    pub fn reload(&mut self) {
        self.loaded = false;
    }

    fn matches(&self) -> Vec<&Transcript> {
        let needle = self.filter.trim().to_lowercase();
        self.entries
            .iter()
            .rev()
            .filter(|entry| {
                needle.is_empty()
                    || entry.text.to_lowercase().contains(&needle)
                    || entry.app.to_lowercase().contains(&needle)
            })
            .collect()
    }

    /// The entry's audio file name, when the file is there.
    fn audio_of<'a>(&self, entry: &'a Transcript) -> Option<&'a str> {
        entry
            .audio
            .as_deref()
            .filter(|name| self.audio.contains(*name))
    }

    fn playing(&self, name: &str) -> bool {
        self.playback.as_ref().is_some_and(|p| p.name == name)
    }

    /// Forget a playback or a transcription that ended.
    fn settle(&mut self) {
        if self.playback.as_ref().is_some_and(|p| p.player.empty()) {
            self.playback = None;
        }
    }

    fn busy(&self) -> bool {
        self.playback.is_some()
            || self
                .again
                .as_ref()
                .is_some_and(|again| again.result.lock().is_none())
    }
}

enum Action {
    Copy(String),
    Delete(Transcript),
    Clear,
    Play(String),
    StopPlaying,
    Again(String),
}

pub fn show(gui: &mut Gui, ui: &mut egui::Ui) {
    gui.history.refresh();
    gui.history.settle();
    if gui.history.busy() {
        ui.ctx().request_repaint_after(BUSY_REPAINT);
    }
    let description = if gui.config.history.enabled {
        format!(
            "Hush keeps the last {} transcripts on this computer, in a file only you can read.",
            gui.config.history.max_entries
        )
    } else {
        "Hush keeps no new transcripts. The ones below stay until you delete them.".to_string()
    };
    ui.add_space(6.0);
    toggle_row(
        ui,
        "Keep a history",
        &description,
        &mut gui.config.history.enabled,
    );
    retention_row(gui, ui);
    if let Some(error) = &gui.history.error {
        ui.add_space(6.0);
        ui.colored_label(BAD, error);
    }

    let view = &mut gui.history;
    let total = view.entries.len();
    if total == 0 {
        ui.add_space(10.0);
        ui.label(egui::RichText::new("Nothing yet. What you dictate shows up here.").color(MUTED));
        return;
    }

    let mut action: Option<Action> = None;
    let summary = if view.confirm_clear {
        format!(
            "Delete all {} transcripts and their audio? You cannot get them back.",
            total
        )
    } else if total == 1 {
        "1 transcript".to_string()
    } else {
        format!("{} transcripts", total)
    };
    row(&summary).reserve(TOOLBAR_CONTROLS).show(ui, |ui| {
        if view.confirm_clear {
            if ui.button("Keep them").clicked() {
                view.confirm_clear = false;
            }
            if ui
                .button(egui::RichText::new("Delete all").color(BAD))
                .clicked()
            {
                action = Some(Action::Clear);
            }
        } else {
            if quiet_button(ui, "Clear all").clicked() {
                view.confirm_clear = true;
            }
            if field(ui, &mut view.filter, "Search").changed() {
                view.shown = PAGE;
            }
        }
    });

    let shown = view.shown.max(PAGE);
    let matches = view.matches();
    if matches.is_empty() {
        ui.add_space(10.0);
        ui.label(egui::RichText::new("No transcript matches.").color(MUTED));
    }
    for entry in matches.iter().take(shown) {
        let when = entry
            .at
            .with_timezone(&chrono::Local)
            .format("%b %-d, %H:%M")
            .to_string();
        let meta = if entry.app.is_empty() {
            when
        } else {
            format!("{} · {}", when, entry.app)
        };
        let audio = view.audio_of(entry);
        let reserve = ENTRY_CONTROLS + if audio.is_some() { AUDIO_CONTROLS } else { 0.0 };
        row(&preview(&entry.text))
            .describe(&meta)
            .reserve(reserve)
            .show(ui, |ui| {
                if quiet_button(ui, "Delete").clicked() {
                    action = Some(Action::Delete((*entry).clone()));
                }
                if quiet_button(ui, "Copy").clicked() {
                    action = Some(Action::Copy(entry.text.clone()));
                }
                if let Some(name) = audio {
                    if quiet_button(ui, "Again")
                        .on_hover_text("Transcribe the audio again with the current model")
                        .clicked()
                    {
                        action = Some(Action::Again(name.to_string()));
                    }
                    if view.playing(name) {
                        if quiet_button(ui, "Stop").clicked() {
                            action = Some(Action::StopPlaying);
                        }
                    } else if quiet_button(ui, "Play").clicked() {
                        action = Some(Action::Play(name.to_string()));
                    }
                }
            });
        if let Some(again) = view
            .again
            .as_ref()
            .filter(|a| Some(a.name.as_str()) == audio)
        {
            if let Some(copy) = show_again(ui, again) {
                action = Some(Action::Copy(copy));
            }
        }
    }
    let remaining = matches.len().saturating_sub(shown);
    if remaining > 0 {
        ui.add_space(8.0);
        if ui
            .button(format!("Show more ({} left)", remaining))
            .clicked()
        {
            view.shown = shown + PAGE;
        }
    }
    ui.add_space(4.0);
    hint(
        ui,
        "Each entry names the window class of its application. Use that name \
         for the rules by application on the Output page.",
    );

    match action {
        Some(Action::Copy(text)) => {
            ui.ctx().copy_text(text);
            gui.say("Copied.");
        },
        Some(Action::Delete(entry)) => {
            if let Some(name) = entry.audio.as_deref() {
                if gui.history.playing(name) {
                    gui.history.playback = None;
                }
                if let Err(e) = store::delete_take(&store::takes_dir(), name) {
                    gui.say(format!("Could not delete the audio: {}", e));
                }
            }
            match transcripts::delete(&transcripts::default_path(), &entry) {
                Ok(_) => gui.history.entries.retain(|kept| *kept != entry),
                Err(e) => gui.say(format!("Could not delete: {}", e)),
            }
            gui.history.reload();
        },
        Some(Action::Clear) => {
            gui.history.playback = None;
            gui.history.again = None;
            match transcripts::clear(&transcripts::default_path())
                .and_then(|()| store::clear(&store::takes_dir()))
            {
                Ok(()) => gui.history.entries.clear(),
                Err(e) => gui.say(format!("Could not clear the history: {}", e)),
            }
            gui.history.confirm_clear = false;
            gui.history.reload();
        },
        Some(Action::Play(name)) => {
            gui.history.playback = None;
            match play(&name) {
                Ok(playback) => gui.history.playback = Some(playback),
                Err(e) => gui.say(format!("Could not play the audio: {:#}", e)),
            }
        },
        Some(Action::StopPlaying) => gui.history.playback = None,
        Some(Action::Again(name)) => {
            gui.history.again = Some(transcribe_again(&name, ui.ctx()));
        },
        None => {},
    }
}

/// The switch and the days of `history.keep_audio_days`.
fn retention_row(gui: &mut Gui, ui: &mut egui::Ui) {
    let days = gui.config.history.keep_audio_days;
    if days > 0 {
        gui.history.days = days;
    }
    let description = if days > 0 {
        format!(
            "Hush keeps the audio of each take for {} days, in files only you can read. \
             Play it or transcribe it again below.",
            days
        )
    } else {
        format!(
            "Hush keeps no audio. The audio of a take that Hush could not transcribe \
             stays for {} days for `hush recover`.",
            store::RECOVERY_DAYS
        )
    };
    let caution = (days == 0 && gui.saved.history.keep_audio_days > 0)
        .then_some("Saving deletes the audio that Hush kept.");
    ui.add_enabled_ui(gui.config.history.enabled, |ui| {
        row("Keep the audio")
            .describe(&description)
            .caution(caution)
            .reserve(RETENTION_CONTROLS)
            .show(ui, |ui| {
                let mut on = days > 0;
                if toggle(ui, &mut on).changed() {
                    gui.config.history.keep_audio_days = if on {
                        Some(gui.history.days)
                            .filter(|days| *days > 0)
                            .unwrap_or(DEFAULT_KEEP_DAYS)
                    } else {
                        0
                    };
                }
                if on {
                    ui.label("days");
                    let mut chosen = gui.config.history.keep_audio_days;
                    let drag = egui::DragValue::new(&mut chosen).range(1..=MAX_KEEP_AUDIO_DAYS);
                    if ui.add(drag).changed() {
                        gui.config.history.keep_audio_days = chosen;
                    }
                }
            });
    });
}

/// The text of a take transcribed again, with a Copy button. Returns the
/// text when the user copies it.
fn show_again(ui: &mut egui::Ui, again: &Again) -> Option<String> {
    let result = again.result.lock().clone();
    let mut copy = None;
    ui.horizontal_wrapped(|ui| match result {
        None => {
            ui.spinner();
            ui.label(egui::RichText::new("Transcribing again…").color(MUTED));
        },
        Some(Ok(text)) if text.is_empty() => {
            ui.label(egui::RichText::new("Hush heard no speech in the audio.").color(MUTED));
        },
        Some(Ok(text)) => {
            if quiet_button(ui, "Copy").clicked() {
                copy = Some(text.clone());
            }
            ui.label(egui::RichText::new(format!("Again: {}", preview(&text))).italics());
        },
        Some(Err(error)) => {
            ui.colored_label(BAD, format!("Could not transcribe again: {}", error));
        },
    });
    ui.separator();
    copy
}

fn play(name: &str) -> Result<Playback> {
    let decoded = file::read(&store::takes_dir().join(name))?;
    let rate = NonZeroU32::new(decoded.sample_rate).context("The file has no sample rate")?;
    let mut sink = DeviceSinkBuilder::open_default_sink().context("No audio output")?;
    sink.log_on_drop(false);
    let player = Player::connect_new(sink.mixer());
    player.append(rodio::buffer::SamplesBuffer::new(
        rodio::nz!(1),
        rate,
        decoded.samples,
    ));
    Ok(Playback {
        name: name.to_string(),
        player,
        _sink: sink,
    })
}

/// Run `hush transcribe` on the take in the background, so it uses the
/// daemon's model when the daemon runs.
fn transcribe_again(name: &str, ctx: &egui::Context) -> Again {
    let result = Arc::new(Mutex::new(None));
    let slot = Arc::clone(&result);
    let path = store::takes_dir().join(name);
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let binary = std::env::current_exe().unwrap_or_else(|_| "hush".into());
        let outcome = match std::process::Command::new(binary)
            .arg("transcribe")
            .arg(&path)
            .output()
        {
            Ok(output) => transcribed(output.status.code(), &output.stdout, &output.stderr),
            Err(e) => Err(e.to_string()),
        };
        *slot.lock() = Some(outcome);
        ctx.request_repaint();
    });
    Again {
        name: name.to_string(),
        result,
    }
}

/// The text from the exit code and output of `hush transcribe`.
fn transcribed(code: Option<i32>, stdout: &[u8], stderr: &[u8]) -> Result<String, String> {
    let no_speech = i32::from(crate::cli::exit::ExitStatus::NoSpeech.code());
    match code {
        Some(0) => Ok(String::from_utf8_lossy(stdout).trim().to_string()),
        Some(code) if code == no_speech => Ok(String::new()),
        _ => {
            let stderr = String::from_utf8_lossy(stderr);
            let first = stderr
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or("hush transcribe failed");
            Err(first.trim_start_matches("Error: ").to_string())
        },
    }
}

fn preview(text: &str) -> String {
    if text.chars().count() <= PREVIEW_CHARS {
        return text.to_string();
    }
    let start: String = text.chars().take(PREVIEW_CHARS).collect();
    format!("{}…", start.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(texts: &[(&str, &str)]) -> HistoryView {
        HistoryView {
            entries: texts
                .iter()
                .map(|(text, app)| Transcript::new(*text, *app))
                .collect(),
            ..HistoryView::default()
        }
    }

    #[test]
    fn matches_are_newest_first_and_search_text_and_app() {
        let mut view = view(&[
            ("deploy the build", "org.kde.konsole"),
            ("Hello Sam", "slack"),
        ]);
        let texts = |view: &HistoryView| -> Vec<String> {
            view.matches().iter().map(|e| e.text.clone()).collect()
        };
        assert_eq!(texts(&view), ["Hello Sam", "deploy the build"]);
        view.filter = " KONSOLE ".into();
        assert_eq!(texts(&view), ["deploy the build"]);
        view.filter = "sam".into();
        assert_eq!(texts(&view), ["Hello Sam"]);
        view.filter = "nothing".into();
        assert!(view.matches().is_empty());
    }

    #[test]
    fn long_transcripts_show_their_start() {
        assert_eq!(preview("short"), "short");
        let long = "word ".repeat(200);
        let shown = preview(&long);
        assert!(shown.ends_with('…'));
        assert_eq!(shown.chars().count(), PREVIEW_CHARS);
    }

    #[test]
    fn only_an_entry_whose_file_is_there_offers_its_audio() {
        let mut view = view(&[]);
        let with = Transcript::new("kept", "").with_audio("a.flac");
        let swept = Transcript::new("swept", "").with_audio("b.flac");
        let without = Transcript::new("plain", "");
        view.audio = HashSet::from(["a.flac".to_string()]);
        assert_eq!(view.audio_of(&with), Some("a.flac"));
        assert_eq!(view.audio_of(&swept), None);
        assert_eq!(view.audio_of(&without), None);
    }

    #[test]
    fn transcribing_again_reads_the_exit_code() {
        assert_eq!(
            transcribed(Some(0), b"  Ship it.\n", b""),
            Ok("Ship it.".to_string())
        );
        assert_eq!(
            transcribed(Some(4), b"", b"Hush heard no speech."),
            Ok(String::new())
        );
        assert_eq!(
            transcribed(
                Some(1),
                b"",
                b"\nError: The model did not load\n\nCaused by: x"
            ),
            Err("The model did not load".to_string())
        );
        assert!(transcribed(None, b"", b"").is_err());
    }
}
