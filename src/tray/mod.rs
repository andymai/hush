//! Status icon on the desktop panel, through the StatusNotifierItem protocol
//! that KDE, GNOME (with the AppIndicator extension), and most panels speak.
//!
//! The icon carries the daemon's state and its menu sends the same session
//! commands the hotkey and the socket do, so nothing here is a second code
//! path for dictation.

pub mod icon;

use crate::ipc::{DaemonState, SessionCommand, SharedState};
use icon::IconState;
use ksni::menu::StandardItem;
use ksni::{Category, Handle, MenuItem, Status, ToolTip, Tray, TrayMethods};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

const POLL_INTERVAL: Duration = Duration::from_millis(250);

pub struct HushTray {
    state: DaemonState,
    hotkey: String,
    control: mpsc::Sender<SessionCommand>,
}

impl HushTray {
    fn send(&self, command: SessionCommand) {
        if self.control.send(command).is_err() {
            warn!("Tray command dropped: the session is gone");
        }
    }

    fn is_recording(&self) -> bool {
        matches!(self.state, DaemonState::Recording { .. })
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
        if self.is_recording() {
            self.send(SessionCommand::StopRecording);
        } else {
            self.send(SessionCommand::StartRecording);
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let recording = self.is_recording();
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
                label: "Edit settings".into(),
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

/// Write the effective configuration the first time, so the file the user
/// opens lists every key rather than being empty, then hand it to the desktop.
fn open_settings() {
    let path = crate::config::paths::config_path();
    if !path.exists() {
        match crate::config::Config::load().and_then(|config| config.save_to_file(&path)) {
            Ok(()) => info!("Wrote the default configuration to {}", path.display()),
            Err(e) => {
                warn!("Could not write {}: {}", path.display(), e);
                return;
            },
        }
    }
    match std::process::Command::new("xdg-open").arg(&path).spawn() {
        Ok(_) => debug!("Opened {}", path.display()),
        Err(e) => warn!("Could not open {}: {}", path.display(), e),
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
) -> Option<TrayGuard> {
    let tray = HushTray {
        state: shared.snapshot(),
        hotkey,
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

    fn tray(state: DaemonState) -> (HushTray, mpsc::Receiver<SessionCommand>) {
        let (tx, rx) = mpsc::channel();
        (
            HushTray {
                state,
                hotkey: "RightAlt".to_string(),
                control: tx,
            },
            rx,
        )
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
    }

    #[test]
    fn clicking_toggles_recording() {
        let (mut idle, rx) = tray(DaemonState::Idle);
        idle.activate(0, 0);
        assert_eq!(rx.try_recv(), Ok(SessionCommand::StartRecording));

        let (mut recording, rx) = tray(DaemonState::Recording { elapsed_ms: 0 });
        recording.activate(0, 0);
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
