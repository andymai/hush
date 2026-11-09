//! What Hush typed, newest first, to copy again or delete.

use crate::gui::theme::{field, hint, quiet_button, row, toggle_row, BAD, MUTED};
use crate::gui::Gui;
use crate::text_processing::transcripts::{self, Transcript};
use std::time::SystemTime;

/// Entries drawn before "Show more"; each one is a wrapped label.
const PAGE: usize = 50;
/// A longer transcript shows its start; Copy still copies all of it.
const PREVIEW_CHARS: usize = 400;
const ENTRY_CONTROLS: f32 = 110.0;
const TOOLBAR_CONTROLS: f32 = 310.0;

/// The history file as last read, and how the page shows it.
#[derive(Default)]
pub struct HistoryView {
    entries: Vec<Transcript>,
    loaded: bool,
    stamp: Option<(SystemTime, u64)>,
    error: Option<String>,
    filter: String,
    shown: usize,
    confirm_clear: bool,
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
}

enum Action {
    Copy(String),
    Delete(Transcript),
    Clear,
}

pub fn show(gui: &mut Gui, ui: &mut egui::Ui) {
    gui.history.refresh();
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
            "Delete all {} transcripts? You cannot get them back.",
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
        row(&preview(&entry.text))
            .describe(&meta)
            .reserve(ENTRY_CONTROLS)
            .show(ui, |ui| {
                if quiet_button(ui, "Delete").clicked() {
                    action = Some(Action::Delete((*entry).clone()));
                }
                if quiet_button(ui, "Copy").clicked() {
                    action = Some(Action::Copy(entry.text.clone()));
                }
            });
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
            match transcripts::delete(&transcripts::default_path(), &entry) {
                Ok(_) => gui.history.entries.retain(|kept| *kept != entry),
                Err(e) => gui.say(format!("Could not delete: {}", e)),
            }
            gui.history.loaded = false;
        },
        Some(Action::Clear) => {
            match transcripts::clear(&transcripts::default_path()) {
                Ok(()) => gui.history.entries.clear(),
                Err(e) => gui.say(format!("Could not clear the history: {}", e)),
            }
            gui.history.confirm_clear = false;
            gui.history.loaded = false;
        },
        None => {},
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
}
