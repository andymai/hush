//! Each change in the window saves at once. A change that the daemon reads
//! also restarts it, once, after the changes stop for a moment, and never
//! while a take records, transcribes, or types.

use crate::config::Config;
use crate::ipc::DaemonState;
use std::time::{Duration, Instant};

/// How long after the last change the daemon restarts, so a burst of
/// changes, such as five clicks on a stepper, restarts it once.
pub const RESTART_DEBOUNCE: Duration = Duration::from_millis(600);

/// Whether `config` differs from `saved` in more than the window's own
/// settings, which the daemon never reads.
pub fn daemon_reads_a_change(saved: &Config, config: &Config) -> bool {
    let mut window_only = saved.clone();
    window_only.gui = config.gui.clone();
    window_only != *config
}

/// What one save of the configuration did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Saved {
    /// Written; `restart` when the daemon reads a change
    Written { restart: bool },
    /// Not written, with the reason
    Refused(String),
}

/// Write `config` over `saved` with `write`, which checks it first.
pub fn save_config(
    saved: &Config,
    config: &Config,
    write: impl FnOnce(&Config) -> anyhow::Result<()>,
) -> Saved {
    match write(config) {
        Ok(()) => Saved::Written {
            restart: daemon_reads_a_change(saved, config),
        },
        Err(e) => Saved::Refused(format!("Not saved: {}", e)),
    }
}

/// What the daemon does, as far as a restart cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Daemon {
    /// Not running: it reads the file when it starts
    Stopped,
    /// In a take, or in a restart already, so a restart now would cut it
    Busy,
    Idle,
}

impl Daemon {
    pub fn of(state: Option<DaemonState>, restarting: bool) -> Self {
        match state {
            None => Daemon::Stopped,
            Some(_) if restarting => Daemon::Busy,
            Some(DaemonState::Idle) => Daemon::Idle,
            Some(_) => Daemon::Busy,
        }
    }
}

/// A restart that waits until the changes stop and the daemon is idle.
#[derive(Debug, Default)]
pub struct Debounce {
    due: Option<Instant>,
}

impl Debounce {
    /// A change the daemon reads: restart [`RESTART_DEBOUNCE`] after it.
    pub fn change(&mut self, now: Instant) {
        self.due = Some(now + RESTART_DEBOUNCE);
    }

    /// Restart as soon as the daemon allows, as when the window closes.
    pub fn hurry(&mut self, now: Instant) {
        if self.due.is_some() {
            self.due = Some(now);
        }
    }

    pub fn pending(&self) -> bool {
        self.due.is_some()
    }

    /// Whether to restart now. A stopped daemon needs no restart; a busy one
    /// keeps the restart waiting.
    pub fn fire(&mut self, now: Instant, daemon: Daemon) -> bool {
        let Some(due) = self.due else {
            return false;
        };
        match daemon {
            Daemon::Stopped => {
                self.due = None;
                false
            },
            Daemon::Busy => false,
            Daemon::Idle if now >= due => {
                self.due = None;
                true
            },
            Daemon::Idle => false,
        }
    }

    /// How long until the restart is due, for the next repaint.
    pub fn wait(&self, now: Instant) -> Option<Duration> {
        self.due.map(|due| due.saturating_duration_since(now))
    }
}

/// The last save, for the footer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Feedback {
    Saved(Instant),
    /// The window holds a change that the file cannot take yet
    Failed(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::Appearance;

    #[test]
    fn only_a_setting_the_daemon_reads_restarts_it() {
        let saved = Config::defaults().unwrap();
        let mut config = saved.clone();
        assert!(!daemon_reads_a_change(&saved, &config));
        config.gui.appearance = Appearance::Dark;
        assert!(
            !daemon_reads_a_change(&saved, &config),
            "the appearance is the window's own"
        );
        config.tray.enabled = !config.tray.enabled;
        assert!(daemon_reads_a_change(&saved, &config));
    }

    #[test]
    fn a_burst_of_changes_restarts_the_daemon_once() {
        let start = Instant::now();
        let mut debounce = Debounce::default();
        let mut restarts = 0;
        for click in 0..5 {
            let now = start + Duration::from_millis(100 * click);
            debounce.change(now);
            restarts += usize::from(debounce.fire(now, Daemon::Idle));
        }
        let last = start + Duration::from_millis(400);
        assert_eq!(restarts, 0, "no restart while the clicks go on");
        assert!(!debounce.fire(last + RESTART_DEBOUNCE / 2, Daemon::Idle));
        assert_eq!(
            debounce.wait(last + RESTART_DEBOUNCE / 2),
            Some(RESTART_DEBOUNCE / 2)
        );
        assert!(debounce.fire(last + RESTART_DEBOUNCE, Daemon::Idle));
        assert!(!debounce.pending());
        assert!(
            !debounce.fire(last + RESTART_DEBOUNCE * 2, Daemon::Idle),
            "once"
        );
    }

    #[test]
    fn each_click_writes_the_file_and_the_burst_restarts_once() {
        let start = Instant::now();
        let mut saved = Config::defaults().unwrap();
        let mut config = saved.clone();
        let mut debounce = Debounce::default();
        let mut writes = 0;
        let mut restarts = 0;
        for click in 0..5_u64 {
            let now = start + Duration::from_millis(150 * click);
            config.audio.silence_stop_secs += 1;
            let outcome = save_config(&saved, &config, |next| {
                writes += 1;
                next.validate()
            });
            assert_eq!(outcome, Saved::Written { restart: true });
            saved = config.clone();
            debounce.change(now);
            restarts += usize::from(debounce.fire(now, Daemon::Idle));
        }
        let quiet = start + Duration::from_millis(600) + RESTART_DEBOUNCE;
        restarts += usize::from(debounce.fire(quiet, Daemon::Idle));
        assert_eq!((writes, restarts), (5, 1));
    }

    #[test]
    fn a_window_only_change_writes_without_a_restart() {
        let saved = Config::defaults().unwrap();
        let mut config = saved.clone();
        config.gui.appearance = Appearance::Light;
        let outcome = save_config(&saved, &config, |next| next.validate());
        assert_eq!(outcome, Saved::Written { restart: false });
    }

    #[test]
    fn settings_that_fail_their_check_are_not_written() {
        let saved = Config::defaults().unwrap();
        let mut config = saved.clone();
        config.transcription.backend =
            crate::config::settings::TranscriptionBackend::OpenAiCompatible;
        let mut written = false;
        let outcome = save_config(&saved, &config, |next| {
            next.validate()?;
            written = true;
            Ok(())
        });
        assert!(matches!(outcome, Saved::Refused(ref why) if why.starts_with("Not saved: ")));
        assert!(!written);
    }

    #[test]
    fn a_take_in_progress_keeps_the_restart_waiting() {
        let start = Instant::now();
        let mut debounce = Debounce::default();
        debounce.change(start);
        let later = start + RESTART_DEBOUNCE * 10;
        assert!(!debounce.fire(later, Daemon::Busy));
        assert!(debounce.pending());
        assert!(debounce.fire(later, Daemon::Idle));
    }

    #[test]
    fn a_stopped_daemon_needs_no_restart() {
        let start = Instant::now();
        let mut debounce = Debounce::default();
        debounce.change(start);
        assert!(!debounce.fire(start + RESTART_DEBOUNCE, Daemon::Stopped));
        assert!(!debounce.pending());
    }

    #[test]
    fn closing_the_window_restarts_without_the_wait() {
        let start = Instant::now();
        let mut debounce = Debounce::default();
        debounce.hurry(start);
        assert!(!debounce.pending(), "nothing to hurry");
        debounce.change(start);
        debounce.hurry(start);
        assert!(debounce.fire(start, Daemon::Idle));
    }

    #[test]
    fn the_daemon_state_says_whether_a_restart_cuts_something() {
        assert_eq!(Daemon::of(None, false), Daemon::Stopped);
        assert_eq!(Daemon::of(Some(DaemonState::Idle), false), Daemon::Idle);
        assert_eq!(Daemon::of(Some(DaemonState::Idle), true), Daemon::Busy);
        for state in [
            DaemonState::Starting,
            DaemonState::Recording { elapsed_ms: 5 },
            DaemonState::Transcribing,
            DaemonState::Inserting,
            DaemonState::Stopping,
        ] {
            assert_eq!(Daemon::of(Some(state), false), Daemon::Busy, "{state:?}");
        }
    }
}
