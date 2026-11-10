//! What Hush typed, newest first, to copy again, fix, or delete, and the
//! audio of each take when Hush keeps it, to play or to transcribe again. A
//! fix proposes the new words in it for the vocabulary.

use crate::audio::{file, store};
use crate::config::settings::MAX_KEEP_AUDIO_DAYS;
use crate::gui::theme::{
    field, hint, palette, quiet_button, row, toggle, toggle_row, ACCENT, BAD, MUTED,
};
use crate::gui::Gui;
use crate::ipc::{client, DaemonCommand};
use crate::text_processing::transcripts::{self, Transcript};
use crate::text_processing::{corrections, learn, DomainVocabulary};
use anyhow::{Context, Result};
use parking_lot::Mutex;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player};
use std::collections::HashSet;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tracing::warn;

/// Entries drawn before "Show more"; each one is a wrapped label.
const PAGE: usize = 50;
/// A longer transcript shows its start; Copy still copies all of it.
const PREVIEW_CHARS: usize = 400;
const ENTRY_CONTROLS: f32 = 150.0;
const CHIP_ROUNDING: f32 = 12.0;
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

/// An entry open for a fix, and its text as the user edits it.
struct Fix {
    entry: Transcript,
    draft: String,
    /// Put the cursor in the text on the next frame
    focus: bool,
}

/// The terms that a saved fix proposes, shown under its entry.
struct Proposals {
    entry: Transcript,
    terms: Vec<String>,
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
    fix: Option<Fix>,
    proposals: Option<Proposals>,
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

    /// Forget a proposal after Add or Skip, and the whole row after the last.
    fn drop_proposal(&mut self, term: &str) {
        if let Some(proposals) = self.proposals.as_mut() {
            proposals.terms.retain(|kept| kept != term);
            if proposals.terms.is_empty() {
                self.proposals = None;
            }
        }
    }

    /// Forget the fix and the proposals of an entry that is gone.
    fn forget(&mut self, entry: &Transcript) {
        if self.fix.as_ref().is_some_and(|fix| fix.entry == *entry) {
            self.fix = None;
        }
        if self.proposals.as_ref().is_some_and(|p| p.entry == *entry) {
            self.proposals = None;
        }
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

/// The entries whose text or app contains the filter, newest first.
fn matching<'a>(entries: &'a [Transcript], filter: &str) -> Vec<&'a Transcript> {
    let needle = filter.trim().to_lowercase();
    entries
        .iter()
        .rev()
        .filter(|entry| {
            needle.is_empty()
                || entry.text.to_lowercase().contains(&needle)
                || entry.app.to_lowercase().contains(&needle)
        })
        .collect()
}

enum Action {
    Copy(String),
    Delete(Transcript),
    Clear,
    Play(String),
    StopPlaying,
    Again(String),
    Fix(Transcript),
    SaveFix,
    CancelFix,
    Learn(String),
    Skip(String),
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
    let matches = matching(&view.entries, &view.filter);
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
        if let Some(fix) = view.fix.as_mut().filter(|fix| fix.entry == **entry) {
            if let Some(choice) = show_fix(ui, fix, &meta) {
                action = Some(choice);
            }
            continue;
        }
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
                if quiet_button(ui, "Fix")
                    .on_hover_text("Correct the text and learn the new words in it")
                    .clicked()
                {
                    action = Some(Action::Fix((*entry).clone()));
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
        if let Some(proposals) = view.proposals.as_ref().filter(|p| p.entry == **entry) {
            if let Some(choice) = show_proposals(ui, &proposals.terms) {
                action = Some(choice);
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
            gui.history.forget(&entry);
            gui.history.reload();
        },
        Some(Action::Clear) => {
            gui.history.playback = None;
            gui.history.again = None;
            gui.history.fix = None;
            gui.history.proposals = None;
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
        Some(Action::Fix(entry)) => {
            gui.history.fix = Some(Fix {
                draft: entry.text.clone(),
                entry,
                focus: true,
            });
        },
        Some(Action::SaveFix) => save_fix(gui),
        Some(Action::CancelFix) => gui.history.fix = None,
        Some(Action::Learn(term)) => {
            gui.history.drop_proposal(&term);
            learn_term(gui, &term);
        },
        Some(Action::Skip(term)) => gui.history.drop_proposal(&term),
        None => {},
    }
}

/// An entry open for a fix: its text in a box, with Save and Cancel.
fn show_fix(ui: &mut egui::Ui, fix: &mut Fix, meta: &str) -> Option<Action> {
    let mut choice = None;
    ui.add_space(6.0);
    hint(ui, meta);
    let response = ui.add(
        egui::TextEdit::multiline(&mut fix.draft)
            .desired_width(f32::INFINITY)
            .desired_rows(2)
            .margin(egui::vec2(8.0, 6.0)),
    );
    if std::mem::take(&mut fix.focus) {
        response.request_focus();
    }
    ui.horizontal(|ui| {
        if ui.button("Save").clicked() {
            choice = Some(Action::SaveFix);
        }
        if quiet_button(ui, "Cancel").clicked() {
            choice = Some(Action::CancelFix);
        }
        hint(
            ui,
            "The entry keeps its time, app, and audio. Hush then suggests \
             the new words for the vocabulary.",
        );
    });
    ui.add_space(6.0);
    ui.separator();
    choice
}

/// The terms that a saved fix proposes, each as a chip with Add and Skip.
fn show_proposals(ui: &mut egui::Ui, terms: &[String]) -> Option<Action> {
    let mut choice = None;
    let colours = palette(ui.ctx());
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new("Learn from the fix:").color(MUTED));
        for term in terms {
            egui::Frame::none()
                .fill(colours.raised)
                .stroke(egui::Stroke::new(1.0_f32, colours.border_strong))
                .rounding(CHIP_ROUNDING)
                .inner_margin(egui::Margin::symmetric(10.0, 0.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(term).strong());
                        let add = egui::Button::new(egui::RichText::new("Add").color(ACCENT))
                            .frame(false);
                        if ui
                            .add(add)
                            .on_hover_text("Add to the vocabulary, as hush learn does")
                            .clicked()
                        {
                            choice = Some(Action::Learn(term.clone()));
                        }
                        if quiet_button(ui, "Skip").clicked() {
                            choice = Some(Action::Skip(term.clone()));
                        }
                    });
                });
        }
    });
    ui.separator();
    choice
}

/// Write the fixed text into its entry, then show the terms that the fix
/// proposes.
fn save_fix(gui: &mut Gui) {
    let Some(fix) = gui.history.fix.take() else {
        return;
    };
    let text = fix.draft.trim().to_string();
    if text.is_empty() {
        gui.say("Type the correct text, or delete the entry.");
        gui.history.fix = Some(fix);
        return;
    }
    if text == fix.entry.text {
        return;
    }
    match transcripts::set_text(&transcripts::default_path(), &fix.entry, &text) {
        Ok(Some(fixed)) => {
            let terms = corrections::proposals(&fix.entry.text, &text, &gui.vocabulary);
            if let Some(kept) = gui.history.entries.iter_mut().find(|e| **e == fix.entry) {
                *kept = fixed.clone();
            }
            gui.history.forget(&fix.entry);
            if terms.is_empty() {
                gui.say("Saved the fix. It has no new words to learn.");
            } else {
                gui.say("Saved the fix.");
                gui.history.proposals = Some(Proposals {
                    entry: fixed,
                    terms,
                });
            }
        },
        Ok(None) => gui.say("Could not save the fix: the history no longer has this entry."),
        Err(e) => {
            gui.say(format!("Could not save the fix: {:#}", e));
            gui.history.fix = Some(fix);
        },
    }
    gui.history.reload();
}

/// Learn the term as `hush learn` does: write it into the vocabulary file,
/// then tell a running daemon, which reloads the file. The window's copy of
/// the vocabulary gets the term too, so a later Save keeps it.
fn learn_term(gui: &mut Gui, term: &str) {
    let term = match learn::learn_at(&DomainVocabulary::default_path(), term) {
        Ok(term) => term,
        Err(e) => {
            gui.say(format!("Could not add the word: {:#}", e));
            return;
        },
    };
    learn::learn_into(&mut gui.vocabulary, &term);
    learn::learn_into(&mut gui.saved_vocabulary, &term);
    let command = DaemonCommand::Learn {
        text: Some(term.clone()),
    };
    gui.runtime.spawn(async move {
        match client::send(command).await {
            Ok(reply) if reply.ok => {},
            Err(e) if e.is::<client::NotRunning>() => {},
            _ => warn!("The daemon did not reload the vocabulary after a fix"),
        }
    });
    gui.say(format!("Added {} to the vocabulary.", term));
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
            matching(&view.entries, &view.filter)
                .iter()
                .map(|e| e.text.clone())
                .collect()
        };
        assert_eq!(texts(&view), ["Hello Sam", "deploy the build"]);
        view.filter = " KONSOLE ".into();
        assert_eq!(texts(&view), ["deploy the build"]);
        view.filter = "sam".into();
        assert_eq!(texts(&view), ["Hello Sam"]);
        view.filter = "nothing".into();
        assert!(texts(&view).is_empty());
    }

    #[test]
    fn each_proposal_goes_after_add_or_skip_and_the_row_after_the_last() {
        let entry = Transcript::new("Kubernetes and New Relic", "");
        let mut view = view(&[]);
        view.proposals = Some(Proposals {
            entry: entry.clone(),
            terms: vec!["Kubernetes".into(), "New Relic".into()],
        });
        view.drop_proposal("Kubernetes");
        assert_eq!(
            view.proposals.as_ref().map(|p| p.terms.clone()),
            Some(vec!["New Relic".to_string()])
        );
        view.drop_proposal("New Relic");
        assert!(view.proposals.is_none());

        view.fix = Some(Fix {
            entry: entry.clone(),
            draft: String::new(),
            focus: false,
        });
        view.forget(&Transcript::new("another", ""));
        assert!(view.fix.is_some());
        view.forget(&entry);
        assert!(view.fix.is_none());
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
