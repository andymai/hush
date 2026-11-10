//! Status icon on the desktop panel, through the StatusNotifierItem protocol
//! that KDE, GNOME (with the AppIndicator extension), and most panels speak.
//!
//! The icon carries the daemon's state and its menu sends the same session
//! commands the hotkey and the socket do, so nothing here is a second code
//! path for dictation. Its Language menu picks the language of later takes.

pub mod icon;

use crate::ipc::{DaemonState, SessionCommand, SharedState};
use crate::text_processing::Trigger;
use crate::transcription::Language;
use icon::IconState;
use ksni::menu::{CheckmarkItem, StandardItem, SubMenu};
use ksni::{Category, Handle, MenuItem, Status, ToolTip, Tray, TrayMethods};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// What the Language menu offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayLanguages {
    /// `transcription.language`
    pub config: Language,
    /// The language of each language chord
    pub chords: Vec<Language>,
    /// The choice that stays until the user changes it; `None` follows the
    /// config language
    pub chosen: Option<Language>,
}

impl TrayLanguages {
    /// The language of a take that starts without `--lang` or a chord.
    pub fn active(&self) -> Language {
        self.chosen.unwrap_or(self.config)
    }

    /// Auto, the config language, each chord language, and a saved choice
    /// that is none of those, each once.
    pub fn options(&self) -> Vec<Language> {
        let mut options: Vec<Language> = Vec::new();
        let candidates = [Language::AUTO, self.config]
            .into_iter()
            .chain(self.chords.iter().copied())
            .chain(self.chosen);
        for language in candidates {
            if !options.contains(&language) {
                options.push(language);
            }
        }
        options
    }

    /// The menu label: the language's name, and `(default)` for the config
    /// language.
    pub fn label(&self, language: Language) -> String {
        if language == self.config {
            format!("{} (default)", language.name())
        } else {
            language.name().to_string()
        }
    }

    /// Choose `language` for later takes. The config language clears the
    /// choice.
    fn choose(&mut self, language: Language) -> Option<Language> {
        self.chosen = (language != self.config).then_some(language);
        self.chosen
    }
}

pub struct HushTray {
    state: DaemonState,
    hotkey: String,
    languages: TrayLanguages,
    control: mpsc::Sender<SessionCommand>,
}

impl HushTray {
    fn send(&self, command: SessionCommand) {
        if self.control.send(command).is_err() {
            warn!("Tray command dropped: the session is gone");
        }
    }

    /// A take is in progress, including one that waits for the microphone.
    fn in_take(&self) -> bool {
        matches!(
            self.state,
            DaemonState::Starting | DaemonState::Recording { .. }
        )
    }

    fn icon_state(&self) -> IconState {
        match self.state {
            DaemonState::Idle => IconState::Idle,
            DaemonState::Recording { .. } => IconState::Recording,
            _ => IconState::Busy,
        }
    }

    /// The line under the title in the tooltip.
    fn summary(&self) -> String {
        match self.state {
            DaemonState::Idle => format!("Idle. Hold {} to dictate.", self.hotkey),
            DaemonState::Starting => "Starting the microphone.".to_string(),
            DaemonState::Recording { elapsed_ms } => {
                format!("Recording, {} s.", elapsed_ms / 1000)
            },
            DaemonState::Transcribing => "Transcribing.".to_string(),
            DaemonState::Inserting => "Typing the text.".to_string(),
            DaemonState::Stopping => "Shutting down.".to_string(),
        }
    }
}

impl Tray for HushTray {
    fn id(&self) -> String {
        "io.github.andymai.hush".to_string()
    }

    fn title(&self) -> String {
        "Hush".to_string()
    }

    fn category(&self) -> Category {
        Category::ApplicationStatus
    }

    fn status(&self) -> Status {
        Status::Active
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        icon::pixmaps(self.icon_state())
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "Hush".to_string(),
            description: self.summary(),
            ..Default::default()
        }
    }

    /// A left click starts or stops dictation, like `hush toggle`.
    fn activate(&mut self, _x: i32, _y: i32) {
        if self.in_take() {
            self.send(SessionCommand::StopRecording);
        } else {
            self.send(SessionCommand::StartRecording(Trigger::Tray, None));
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let recording = self.in_take();
        let busy = !recording && !matches!(self.state, DaemonState::Idle);
        vec![
            StandardItem {
                label: if recording {
                    "Stop and insert".into()
                } else {
                    "Start dictation".into()
                },
                enabled: !busy,
                activate: Box::new(|tray: &mut Self| tray.activate(0, 0)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Discard recording".into(),
                enabled: recording,
                activate: Box::new(|tray: &mut Self| tray.send(SessionCommand::CancelRecording)),
                ..Default::default()
            }
            .into(),
            self.language_menu(),
            MenuItem::Separator,
            StandardItem {
                label: "Type last transcript".into(),
                enabled: !recording,
                activate: Box::new(|tray: &mut Self| tray.send(SessionCommand::PasteLast)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Learn selected word".into(),
                enabled: !recording,
                activate: Box::new(|tray: &mut Self| tray.send(SessionCommand::Learn)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Settings".into(),
                activate: Box::new(|_| open_settings()),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit Hush".into(),
                activate: Box::new(|tray: &mut Self| tray.send(SessionCommand::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

impl HushTray {
    fn language_menu(&self) -> MenuItem<Self> {
        let active = self.languages.active();
        let submenu = self
            .languages
            .options()
            .into_iter()
            .map(|language| {
                CheckmarkItem {
                    label: self.languages.label(language),
                    checked: language == active,
                    activate: Box::new(move |tray: &mut Self| {
                        let choice = tray.languages.choose(language);
                        tray.send(SessionCommand::ChooseLanguage(choice));
                    }),
                    ..Default::default()
                }
                .into()
            })
            .collect();
        SubMenu {
            label: format!("Language: {}", active.name()),
            submenu,
            ..Default::default()
        }
        .into()
    }
}

/// Open the settings window in its own process, so the daemon never owns one.
fn open_settings() {
    let binary = std::env::current_exe().unwrap_or_else(|_| "hush".into());
    match std::process::Command::new(&binary).arg("settings").spawn() {
        Ok(_) => debug!("Opened the settings window"),
        Err(e) => warn!("Could not open the settings window: {}", e),
    }
}

/// Keeps the icon in step with the daemon and removes it when dropped.
pub struct TrayGuard {
    handle: Handle<HushTray>,
    poller: tokio::task::JoinHandle<()>,
}

impl Drop for TrayGuard {
    fn drop(&mut self) {
        self.poller.abort();
        // The awaiter only reports completion; the request is already sent.
        drop(self.handle.shutdown());
    }
}

/// Show the icon. Returns `None` when no panel is listening, which is normal
/// on a bare compositor and never fatal.
pub async fn spawn(
    shared: Arc<SharedState>,
    control: mpsc::Sender<SessionCommand>,
    hotkey: String,
    languages: TrayLanguages,
) -> Option<TrayGuard> {
    let tray = HushTray {
        state: shared.snapshot(),
        hotkey,
        languages,
        control,
    };
    let handle = match tray.spawn().await {
        Ok(handle) => handle,
        Err(e) => {
            info!("No tray icon: {}", e);
            return None;
        },
    };
    info!("Tray icon registered");

    let poller = tokio::spawn({
        let handle = handle.clone();
        async move {
            let mut last = shared.snapshot();
            loop {
                tokio::time::sleep(POLL_INTERVAL).await;
                let now = shared.snapshot();
                if !worth_redrawing(last, now) {
                    continue;
                }
                last = now;
                if handle.update(|tray| tray.state = now).await.is_none() {
                    break;
                }
            }
        }
    });
    Some(TrayGuard { handle, poller })
}

/// The elapsed milliseconds in `Recording` change constantly; only redraw when
/// the icon or the menu would actually differ.
fn worth_redrawing(before: DaemonState, after: DaemonState) -> bool {
    !matches!(
        (before, after),
        (DaemonState::Recording { .. }, DaemonState::Recording { .. })
    ) && std::mem::discriminant(&before) != std::mem::discriminant(&after)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lang(code: &str) -> Language {
        Language::parse(code).unwrap()
    }

    fn tray(state: DaemonState) -> (HushTray, mpsc::Receiver<SessionCommand>) {
        let (tx, rx) = mpsc::channel();
        (
            HushTray {
                state,
                hotkey: "RightAlt".to_string(),
                languages: TrayLanguages {
                    config: lang("en"),
                    chords: vec![lang("de")],
                    chosen: None,
                },
                control: tx,
            },
            rx,
        )
    }

    /// The Language submenu: each label, and whether it has a check mark.
    fn language_items(tray: &HushTray) -> (String, Vec<(String, bool)>) {
        let menu = tray.menu();
        let submenu = menu
            .iter()
            .find_map(|item| match item {
                MenuItem::SubMenu(submenu) => Some(submenu),
                _ => None,
            })
            .expect("a Language submenu");
        let items = submenu
            .submenu
            .iter()
            .map(|item| match item {
                MenuItem::Checkmark(item) => (item.label.clone(), item.checked),
                other => panic!("only check items, not {:?}", other),
            })
            .collect();
        (submenu.label.clone(), items)
    }

    fn pick(tray: &mut HushTray, label: &str) {
        let menu = tray.menu();
        let Some(MenuItem::SubMenu(submenu)) = menu
            .into_iter()
            .find(|item| matches!(item, MenuItem::SubMenu(_)))
        else {
            panic!("a Language submenu")
        };
        for item in submenu.submenu {
            if let MenuItem::Checkmark(item) = item {
                if item.label == label {
                    (item.activate)(tray);
                    return;
                }
            }
        }
        panic!("no item {label}");
    }

    #[test]
    fn the_language_menu_checks_the_active_language() {
        let (tray, _rx) = tray(DaemonState::Idle);
        let (title, items) = language_items(&tray);
        assert_eq!(title, "Language: English");
        assert_eq!(
            items,
            [
                ("Auto".to_string(), false),
                ("English (default)".to_string(), true),
                ("German".to_string(), false),
            ]
        );
    }

    #[test]
    fn a_choice_stays_and_the_default_clears_it() {
        let (mut tray, rx) = tray(DaemonState::Idle);
        pick(&mut tray, "German");
        assert_eq!(
            rx.try_recv(),
            Ok(SessionCommand::ChooseLanguage(Some(lang("de"))))
        );
        let (title, items) = language_items(&tray);
        assert_eq!(title, "Language: German");
        assert!(items.contains(&("German".to_string(), true)));

        pick(&mut tray, "Auto");
        assert_eq!(
            rx.try_recv(),
            Ok(SessionCommand::ChooseLanguage(Some(Language::AUTO)))
        );
        pick(&mut tray, "English (default)");
        assert_eq!(rx.try_recv(), Ok(SessionCommand::ChooseLanguage(None)));
        assert_eq!(tray.languages.active(), lang("en"));
    }

    #[test]
    fn a_saved_choice_that_no_key_names_still_shows() {
        let languages = TrayLanguages {
            config: Language::AUTO,
            chords: vec![lang("de"), lang("de")],
            chosen: Some(lang("fr")),
        };
        assert_eq!(
            languages.options(),
            [Language::AUTO, lang("de"), lang("fr")],
            "Auto is the config language, and each language shows once"
        );
        assert_eq!(languages.label(Language::AUTO), "Auto (default)");
        assert_eq!(languages.active(), lang("fr"));
    }

    #[test]
    fn the_icon_and_summary_follow_the_state() {
        let (idle, _rx) = tray(DaemonState::Idle);
        assert_eq!(idle.icon_state(), IconState::Idle);
        assert!(idle.summary().contains("RightAlt"));

        let (recording, _rx) = tray(DaemonState::Recording { elapsed_ms: 4200 });
        assert_eq!(recording.icon_state(), IconState::Recording);
        assert_eq!(recording.summary(), "Recording, 4 s.");

        let (busy, _rx) = tray(DaemonState::Transcribing);
        assert_eq!(busy.icon_state(), IconState::Busy);

        let (starting, _rx) = tray(DaemonState::Starting);
        assert_eq!(
            starting.icon_state(),
            IconState::Busy,
            "the recording look waits for the first audio frame"
        );
        assert_eq!(starting.summary(), "Starting the microphone.");
    }

    #[test]
    fn clicking_toggles_recording() {
        let (mut idle, rx) = tray(DaemonState::Idle);
        idle.activate(0, 0);
        assert_eq!(
            rx.try_recv(),
            Ok(SessionCommand::StartRecording(Trigger::Tray, None))
        );

        let (mut recording, rx) = tray(DaemonState::Recording { elapsed_ms: 0 });
        recording.activate(0, 0);
        assert_eq!(rx.try_recv(), Ok(SessionCommand::StopRecording));

        let (mut starting, rx) = tray(DaemonState::Starting);
        starting.activate(0, 0);
        assert_eq!(rx.try_recv(), Ok(SessionCommand::StopRecording));
    }

    #[test]
    fn the_menu_matches_the_state() {
        let (idle, _rx) = tray(DaemonState::Idle);
        let labels: Vec<String> = idle
            .menu()
            .iter()
            .map(|item| match item {
                MenuItem::Standard(item) => item.label.clone(),
                _ => "-".to_string(),
            })
            .collect();
        assert_eq!(labels[0], "Start dictation");
        assert!(labels.contains(&"Type last transcript".to_string()));
        assert!(labels.contains(&"Settings".to_string()));
        assert!(labels.contains(&"Quit Hush".to_string()));

        let (recording, _rx) = tray(DaemonState::Recording { elapsed_ms: 0 });
        let menu = recording.menu();
        let MenuItem::Standard(first) = &menu[0] else {
            panic!("the first item is a standard item")
        };
        assert_eq!(first.label, "Stop and insert");
        let MenuItem::Standard(discard) = &menu[1] else {
            panic!("the second item is a standard item")
        };
        assert!(discard.enabled, "discard is only available while recording");
    }

    #[test]
    fn menu_actions_reach_the_session() {
        let (mut tray, rx) = tray(DaemonState::Recording { elapsed_ms: 0 });
        let menu = tray.menu();
        for item in menu {
            let MenuItem::Standard(item) = item else {
                continue;
            };
            if item.label == "Discard recording" {
                (item.activate)(&mut tray);
            }
        }
        assert_eq!(rx.try_recv(), Ok(SessionCommand::CancelRecording));
    }

    #[test]
    fn only_real_changes_redraw() {
        assert!(!worth_redrawing(
            DaemonState::Recording { elapsed_ms: 10 },
            DaemonState::Recording { elapsed_ms: 900 }
        ));
        assert!(!worth_redrawing(DaemonState::Idle, DaemonState::Idle));
        assert!(worth_redrawing(
            DaemonState::Idle,
            DaemonState::Recording { elapsed_ms: 0 }
        ));
        assert!(worth_redrawing(
            DaemonState::Transcribing,
            DaemonState::Inserting
        ));
    }
}
