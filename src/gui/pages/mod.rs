//! One page per group of settings, in the order a person meets them.

pub mod desktop;
pub mod hotkeys;
pub mod output;
pub mod overview;
pub mod speech;
pub mod text;

use super::Gui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Overview,
    Hotkeys,
    Speech,
    Text,
    Output,
    Desktop,
}

impl Page {
    pub const ALL: [Page; 6] = [
        Page::Overview,
        Page::Hotkeys,
        Page::Speech,
        Page::Text,
        Page::Output,
        Page::Desktop,
    ];

    /// `HUSH_SETTINGS_PAGE=hotkeys hush settings` opens on that page, for
    /// launchers, screenshots, and bug reports.
    pub fn from_env() -> Option<Self> {
        let wanted = std::env::var("HUSH_SETTINGS_PAGE").ok()?;
        Self::ALL
            .into_iter()
            .find(|page| page.title().eq_ignore_ascii_case(wanted.trim()))
    }

    pub fn title(self) -> &'static str {
        match self {
            Page::Overview => "Overview",
            Page::Hotkeys => "Hotkeys",
            Page::Speech => "Speech",
            Page::Text => "Text",
            Page::Output => "Output",
            Page::Desktop => "Desktop",
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
