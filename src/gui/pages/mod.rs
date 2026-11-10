//! One page per group of settings, in the order a person meets them.

pub mod desktop;
pub mod history;
pub mod hotkeys;
pub mod output;
pub mod overview;
pub mod speech;
pub mod text;

use super::icons;
use super::widgets::Row;
use super::Gui;
use crate::config::Config;
use std::ops::RangeInclusive;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Overview,
    Hotkeys,
    Speech,
    Text,
    Output,
    Desktop,
    History,
}

impl Page {
    pub const ALL: [Page; 7] = [
        Page::Overview,
        Page::Hotkeys,
        Page::Speech,
        Page::Text,
        Page::Output,
        Page::Desktop,
        Page::History,
    ];

    /// `HUSH_SETTINGS_PAGE=hotkeys hush settings` opens on that page, for
    /// launchers, screenshots, and bug reports.
    pub fn from_env() -> Option<Self> {
        let wanted = std::env::var("HUSH_SETTINGS_PAGE").ok()?;
        Self::ALL
            .into_iter()
            .find(|page| page.title().eq_ignore_ascii_case(wanted.trim()))
    }

    pub fn icon(self) -> char {
        match self {
            Page::Overview => icons::HOUSE,
            Page::Hotkeys => icons::KEYBOARD,
            Page::Speech => icons::MIC,
            Page::Text => icons::TYPE,
            Page::Output => icons::TEXT_CURSOR_INPUT,
            Page::Desktop => icons::MONITOR,
            Page::History => icons::HISTORY,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Page::Overview => "Overview",
            Page::Hotkeys => "Hotkeys",
            Page::Speech => "Speech",
            Page::Text => "Text",
            Page::Output => "Output",
            Page::Desktop => "Desktop",
            Page::History => "History",
        }
    }
}

pub fn show(gui: &mut Gui, ui: &mut egui::Ui) {
    match gui.page {
        Page::Overview => overview::show(gui, ui),
        Page::Hotkeys => hotkeys::show(gui, ui),
        Page::Speech => speech::show(gui, ui),
        Page::Text => text::show(gui, ui),
        Page::Output => output::show(gui, ui),
        Page::Desktop => desktop::show(gui, ui),
        Page::History => history::show(gui, ui),
    }
}

/// A row with a field for the text setting that `slot` reaches. A commit
/// saves, or the row shows why it cannot.
pub fn text_setting(
    gui: &mut Gui,
    ui: &mut egui::Ui,
    row: Row<'_>,
    placeholder: &str,
    slot: fn(&mut Config) -> &mut String,
) {
    let saved = slot(&mut gui.config).clone();
    if let Some(commit) = row.value_field(ui, &saved, placeholder) {
        let text = commit.text.clone();
        if let Err(problem) = gui.try_set(|config| *slot(config) = text) {
            commit.reject(ui.ctx(), problem);
        }
    }
}

/// A row with a field for the whole number that `slot` reaches, from
/// `range`, with `unit` after it.
pub fn number_setting(
    gui: &mut Gui,
    ui: &mut egui::Ui,
    row: Row<'_>,
    range: RangeInclusive<u64>,
    unit: &str,
    slot: fn(&mut Config) -> &mut u64,
) {
    let saved = *slot(&mut gui.config);
    if let Some((number, commit)) = row.number_field(ui, saved, range, unit) {
        if let Err(problem) = gui.try_set(|config| *slot(config) = number) {
            commit.reject(ui.ctx(), problem);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_page_has_a_distinct_title() {
        let mut titles: Vec<&str> = Page::ALL.iter().map(|page| page.title()).collect();
        titles.sort_unstable();
        titles.dedup();
        assert_eq!(titles.len(), Page::ALL.len());
    }
}
