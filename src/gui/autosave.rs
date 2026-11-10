//! Each change in the window saves at once. A change that the daemon reads
//! also restarts it, once, after the changes stop for a moment, and never
//! while a take records, transcribes, or types.

use crate::config::settings::{LlmSetting, TranscriptionBackend};
use crate::config::{paths, transfer, Config};
use crate::ipc::DaemonState;
use crate::text_processing::DomainVocabulary;
use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

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
    /// A daemon in a restart is down for a moment, which is no reason to
    /// drop the restart that a later change needs.
    pub fn of(state: Option<DaemonState>, restarting: bool) -> Self {
        match state {
            _ if restarting => Daemon::Busy,
            None => Daemon::Stopped,
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

    /// When to draw again: at the due time, then every `poll` while a take
    /// keeps the restart waiting.
    pub fn wait(&self, now: Instant, poll: Duration) -> Option<Duration> {
        self.due
            .map(|due| match due.saturating_duration_since(now) {
                Duration::ZERO => poll,
                left => left,
            })
    }
}

/// The last time and size of a file, to see that something else wrote it.
pub type Stamp = Option<(SystemTime, u64)>;

pub fn stamp(path: &Path) -> Stamp {
    let metadata = std::fs::metadata(path).ok()?;
    Some((metadata.modified().ok()?, metadata.len()))
}

/// `ours` rebased onto `theirs`: each value that `ours` changed since
/// `base` replaces the one in `theirs`, and everything else in `theirs`
/// stays. So a word that the Learn key added survives a word that the window
/// removed. Tables merge by key; lists and other values are replaced whole.
pub fn rebase<T: Serialize + DeserializeOwned>(base: &T, ours: &T, theirs: &T) -> Result<T> {
    let base = serde_json::to_value(base).context("Failed to read the saved settings")?;
    let ours = serde_json::to_value(ours).context("Failed to read the window's settings")?;
    let mut theirs = serde_json::to_value(theirs).context("Failed to read the file")?;
    merge(&base, &ours, &mut theirs);
    serde_json::from_value(theirs).context("The changes do not fit the file")
}

fn merge(base: &Value, ours: &Value, theirs: &mut Value) {
    if base == ours {
        return;
    }
    match (base, ours, theirs) {
        (Value::Object(base), Value::Object(ours), Value::Object(theirs)) => {
            for (key, value) in ours {
                match (base.get(key), theirs.get_mut(key)) {
                    (Some(base), Some(theirs)) => merge(base, value, theirs),
                    (_, theirs_value) => {
                        if base.get(key) != Some(value) {
                            match theirs_value {
                                Some(slot) => *slot = value.clone(),
                                None => {
                                    theirs.insert(key.clone(), value.clone());
                                },
                            }
                        }
                    },
                }
            }
            for key in base.keys().filter(|key| !ours.contains_key(*key)) {
                theirs.remove(key);
            }
        },
        (_, ours, theirs) => *theirs = ours.clone(),
    }
}

/// The files behind the window: where they are, when the window last read
/// or wrote them, and why one did not load.
#[derive(Debug)]
pub struct Files {
    config_path: PathBuf,
    vocabulary_path: PathBuf,
    config_stamp: Stamp,
    vocabulary_stamp: Stamp,
    /// The configuration file did not load. Nothing writes it until the user
    /// replaces it.
    pub config_error: Option<String>,
    pub vocabulary_error: Option<String>,
}

impl Files {
    /// The default files, as the window just read them.
    pub fn read(config_error: Option<String>, vocabulary_error: Option<String>) -> Self {
        Self::at(
            paths::config_path(),
            DomainVocabulary::default_path(),
            config_error,
            vocabulary_error,
        )
    }

    fn at(
        config_path: PathBuf,
        vocabulary_path: PathBuf,
        config_error: Option<String>,
        vocabulary_error: Option<String>,
    ) -> Self {
        Self {
            config_stamp: stamp(&config_path),
            vocabulary_stamp: stamp(&vocabulary_path),
            config_path,
            vocabulary_path,
            config_error,
            vocabulary_error,
        }
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn vocabulary_path(&self) -> &Path {
        &self.vocabulary_path
    }

    /// Write `ours` over the configuration file in one step. When something
    /// else wrote the file since the window read or wrote it, the window's
    /// changes since `saved` land on that file instead. Returns the settings
    /// the file held and the settings written.
    pub fn write_config(&mut self, saved: &Config, ours: &Config) -> Result<(Config, Config)> {
        let path = self.config_path.clone();
        let (base, next) = if path.exists() && stamp(&path) != self.config_stamp {
            let theirs = Config::load_from_file(&path)
                .context("The file changed on disk and does not load")?;
            let merged = rebase(saved, ours, &theirs)?;
            (theirs, merged)
        } else {
            (saved.clone(), ours.clone())
        };
        transfer::replace_file(&path, next.to_toml()?.as_bytes())?;
        self.config_stamp = stamp(&path);
        Ok((base, next))
    }

    /// [`Files::write_config`] for the vocabulary, such as after the Learn
    /// key added a word.
    pub fn write_vocabulary(
        &mut self,
        saved: &DomainVocabulary,
        ours: &DomainVocabulary,
    ) -> Result<DomainVocabulary> {
        let path = self.vocabulary_path.clone();
        let next = if path.exists() && stamp(&path) != self.vocabulary_stamp {
            let theirs = DomainVocabulary::load_from_file(&path)
                .context("The file changed on disk and does not load")?;
            rebase(saved, ours, &theirs)?
        } else {
            ours.clone()
        };
        let text =
            serde_json::to_string_pretty(&next).context("Failed to serialize the vocabulary")?;
        transfer::replace_file(&path, text.as_bytes())?;
        self.vocabulary_stamp = stamp(&path);
        Ok(next)
    }

    /// Copy the configuration file that does not load to `.bak`, then write
    /// `config` in its place. Returns the copy's path.
    pub fn replace_config(&mut self, config: &Config) -> Result<Option<PathBuf>> {
        let text = config.to_toml()?;
        let backup = transfer::back_up(&self.config_path)?;
        transfer::replace_file(&self.config_path, text.as_bytes())?;
        self.config_stamp = stamp(&self.config_path);
        self.config_error = None;
        Ok(backup)
    }

    /// [`Files::replace_config`] for the vocabulary.
    pub fn replace_vocabulary(&mut self, vocabulary: &DomainVocabulary) -> Result<Option<PathBuf>> {
        let text = serde_json::to_string_pretty(vocabulary)
            .context("Failed to serialize the vocabulary")?;
        let backup = transfer::back_up(&self.vocabulary_path)?;
        transfer::replace_file(&self.vocabulary_path, text.as_bytes())?;
        self.vocabulary_stamp = stamp(&self.vocabulary_path);
        self.vocabulary_error = None;
        Ok(backup)
    }

    /// The window had the vocabulary file written for it, as a word learned
    /// from a fix is.
    pub fn vocabulary_written(&mut self) {
        self.vocabulary_stamp = stamp(&self.vocabulary_path);
    }
}

/// What a request to close the window does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseStep {
    /// Move the focus away so the field commits, then close
    CommitField,
    /// Say what would be lost; a second close discards it
    Warn,
    /// Wait until an owed restart starts
    WaitForRestart,
    Close,
}

/// The state of the window when the close request came.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CloseState {
    pub focused: bool,
    /// A close is under way already
    pub closing: bool,
    /// The user saw what a close loses
    pub warned: bool,
    pub unsaved: bool,
    pub restart_owed: bool,
}

pub fn close_step(state: CloseState) -> CloseStep {
    if state.focused {
        CloseStep::CommitField
    } else if state.closing {
        CloseStep::Close
    } else if state.unsaved && !state.warned {
        CloseStep::Warn
    } else if state.restart_owed {
        CloseStep::WaitForRestart
    } else {
        CloseStep::Close
    }
}

/// Choices that wait until the other settings can take them, such as an
/// OpenAI-compatible server picked before its address.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Pending {
    pub backend: Option<TranscriptionBackend>,
    pub provider: Option<LlmSetting>,
}

/// How long Undo stays in the footer.
pub const UNDO_DURATION: Duration = Duration::from_secs(10);

/// The settings before the last removal, for Undo.
#[derive(Debug, Clone)]
pub struct Undo {
    pub label: String,
    pub before: (Config, DomainVocabulary),
    at: Instant,
    /// The settings just after the removal; any other change ends the Undo
    after: Option<(Config, DomainVocabulary)>,
}

impl Undo {
    pub fn new(label: String, before: (Config, DomainVocabulary), at: Instant) -> Self {
        Self {
            label,
            before,
            at,
            after: None,
        }
    }

    /// Whether Undo still applies: within [`UNDO_DURATION`], and with no
    /// change after the removal. The first call notes the settings after it.
    pub fn still(&mut self, config: &Config, vocabulary: &DomainVocabulary, now: Instant) -> bool {
        if now.saturating_duration_since(self.at) > UNDO_DURATION {
            return false;
        }
        match &self.after {
            None => {
                self.after = Some((config.clone(), vocabulary.clone()));
                true
            },
            Some((after_config, after_vocabulary)) => {
                after_config == config && after_vocabulary == vocabulary
            },
        }
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
        let poll = Duration::from_millis(150);
        assert_eq!(
            debounce.wait(last + RESTART_DEBOUNCE / 2, poll),
            Some(RESTART_DEBOUNCE / 2)
        );
        assert!(debounce.fire(last + RESTART_DEBOUNCE, Daemon::Idle));
        assert!(!debounce.pending());
        assert!(
            !debounce.fire(last + RESTART_DEBOUNCE * 2, Daemon::Idle),
            "once"
        );
    }

    fn files(dir: &tempfile::TempDir) -> Files {
        Files::at(
            dir.path().join("config.toml"),
            dir.path().join("vocabulary.json"),
            None,
            None,
        )
    }

    #[test]
    fn each_click_writes_the_file_and_the_burst_restarts_once() {
        let dir = tempfile::tempdir().unwrap();
        let mut files = files(&dir);
        let start = Instant::now();
        let mut saved = Config::defaults().unwrap();
        let mut config = saved.clone();
        let mut debounce = Debounce::default();
        let mut restarts = 0;
        for click in 0..5_u64 {
            let now = start + Duration::from_millis(150 * click);
            config.audio.silence_stop_secs += 1;
            let (base, written) = files.write_config(&saved, &config).unwrap();
            let on_disk = Config::load_from_file(files.config_path()).unwrap();
            assert_eq!(
                on_disk.audio.silence_stop_secs,
                click + 1,
                "written at once"
            );
            assert!(daemon_reads_a_change(&base, &written));
            saved = written;
            debounce.change(now);
            restarts += usize::from(debounce.fire(now, Daemon::Idle));
        }
        let quiet = start + Duration::from_millis(600) + RESTART_DEBOUNCE;
        restarts += usize::from(debounce.fire(quiet, Daemon::Idle));
        assert_eq!(restarts, 1);
    }

    #[test]
    fn settings_that_fail_their_check_are_not_written() {
        let dir = tempfile::tempdir().unwrap();
        let mut files = files(&dir);
        let saved = Config::defaults().unwrap();
        files.write_config(&saved, &saved).unwrap();
        let before = std::fs::read_to_string(files.config_path()).unwrap();
        let mut config = saved.clone();
        config.transcription.backend = TranscriptionBackend::OpenAiCompatible;
        assert!(files.write_config(&saved, &config).is_err());
        assert_eq!(
            std::fs::read_to_string(files.config_path()).unwrap(),
            before
        );
    }

    #[test]
    fn an_outside_edit_survives_the_next_window_change() {
        let dir = tempfile::tempdir().unwrap();
        let mut files = files(&dir);
        let saved = Config::defaults().unwrap();
        files.write_config(&saved, &saved).unwrap();
        let mut outside = saved.clone();
        outside.llm.ollama_model = "a-much-longer-model-name".into();
        std::fs::write(files.config_path(), outside.to_toml().unwrap()).unwrap();

        let mut ours = saved.clone();
        ours.tray.enabled = false;
        let (base, written) = files.write_config(&saved, &ours).unwrap();
        assert_eq!(base.llm.ollama_model, "a-much-longer-model-name");
        let on_disk = Config::load_from_file(files.config_path()).unwrap();
        assert_eq!(on_disk, written);
        assert!(!on_disk.tray.enabled, "the window's change");
        assert_eq!(on_disk.llm.ollama_model, "a-much-longer-model-name");
    }

    #[test]
    fn a_file_that_does_not_load_is_kept_before_it_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let mut files = Files::at(
            dir.path().join("config.toml"),
            dir.path().join("vocabulary.json"),
            Some("Invalid config file".into()),
            None,
        );
        std::fs::write(files.config_path(), "not = [toml").unwrap();
        let defaults = Config::defaults().unwrap();
        let backup = files.replace_config(&defaults).unwrap().unwrap();
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), "not = [toml");
        assert!(backup.ends_with("config.toml.bak"));
        assert_eq!(
            Config::load_from_file(files.config_path()).unwrap(),
            defaults
        );
        assert_eq!(files.config_error, None);
    }

    #[test]
    fn undo_lasts_until_the_next_change_or_ten_seconds() {
        let start = Instant::now();
        let before = Config::defaults().unwrap();
        let vocabulary = DomainVocabulary::default();
        let mut after = before.clone();
        after.insertion.clipboard_apps.clear();
        let mut undo = Undo::new(
            "Removed kitty".into(),
            (before.clone(), vocabulary.clone()),
            start,
        );
        assert!(undo.still(&after, &vocabulary, start), "notes the removal");
        assert!(undo.still(&after, &vocabulary, start + Duration::from_secs(5)));
        assert!(!undo.still(&after, &vocabulary, start + UNDO_DURATION * 2));
        let mut undo = Undo::new("Removed kitty".into(), (before, vocabulary.clone()), start);
        undo.still(&after, &vocabulary, start);
        let mut later = after.clone();
        later.tray.enabled = false;
        assert!(
            !undo.still(&later, &vocabulary, start),
            "a later change ends it"
        );
    }

    #[test]
    fn a_close_commits_the_field_then_asks_then_waits_for_the_restart() {
        let state = CloseState::default();
        assert_eq!(close_step(state), CloseStep::Close);
        let focused = CloseState {
            focused: true,
            unsaved: true,
            ..state
        };
        assert_eq!(
            close_step(focused),
            CloseStep::CommitField,
            "first the field"
        );
        let unsaved = CloseState {
            unsaved: true,
            restart_owed: true,
            ..state
        };
        assert_eq!(close_step(unsaved), CloseStep::Warn);
        let warned = CloseState {
            warned: true,
            ..unsaved
        };
        assert_eq!(
            close_step(warned),
            CloseStep::WaitForRestart,
            "the restart still runs"
        );
        let again = CloseState {
            closing: true,
            ..warned
        };
        assert_eq!(close_step(again), CloseStep::Close, "a second close goes");
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
    fn a_restart_that_waits_for_a_take_draws_at_the_poll_rate() {
        let start = Instant::now();
        let poll = Duration::from_millis(150);
        let mut debounce = Debounce::default();
        assert_eq!(debounce.wait(start, poll), None);
        debounce.change(start);
        let late = start + RESTART_DEBOUNCE * 3;
        assert!(!debounce.fire(late, Daemon::Busy));
        assert_eq!(debounce.wait(late, poll), Some(poll), "not every frame");
    }

    #[test]
    fn a_window_change_lands_on_a_file_that_changed_outside() {
        let base = Config::defaults().unwrap();
        let mut ours = base.clone();
        ours.tray.enabled = false;
        ours.insertion.clipboard_apps.push("kitty".into());
        let mut theirs = base.clone();
        theirs.llm.ollama_model = "qwen3".into();
        theirs.hotkey.languages.insert("F13".into(), "de".into());
        let merged = rebase(&base, &ours, &theirs).unwrap();
        assert!(!merged.tray.enabled, "the window's change");
        assert_eq!(merged.insertion.clipboard_apps, ["kitty"]);
        assert_eq!(merged.llm.ollama_model, "qwen3", "the outside change");
        assert_eq!(merged.hotkey.languages["F13"], "de");
    }

    #[test]
    fn a_word_the_learn_key_added_survives_a_word_the_window_removed() {
        use crate::text_processing::DomainVocabulary;
        let mut base = DomainVocabulary::default();
        base.add_technical_term("kubernetes".into(), "Kubernetes".into());
        base.add_technical_term("neovim".into(), "Neovim".into());
        let mut ours = base.clone();
        ours.technical_terms.remove("neovim");
        let mut theirs = base.clone();
        theirs.add_technical_term("postgresql".into(), "PostgreSQL".into());
        let merged = rebase(&base, &ours, &theirs).unwrap();
        let mut words: Vec<_> = merged.technical_terms.values().cloned().collect();
        words.sort();
        assert_eq!(words, ["Kubernetes", "PostgreSQL"]);
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
        assert_eq!(
            Daemon::of(None, true),
            Daemon::Busy,
            "a restarting daemon is down for a moment, not stopped"
        );
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
