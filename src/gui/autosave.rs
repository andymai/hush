//! Each change in the window saves at once. A change that the daemon reads
//! also restarts it, once, after the changes stop for a moment, and never
//! while a take records, transcribes, or types.

use crate::config::settings::{HistoryConfig, LlmSetting, TranscriptionBackend};
use crate::config::{paths, transfer, Config};
use crate::ipc::client::Probe;
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
    /// A daemon in a restart is down for a moment, and a daemon that gives
    /// no answer in time may be busy: neither is a reason to drop the
    /// restart that a change needs. While the configuration file does not
    /// load, a running daemon is held as busy: restarted, it would run on
    /// the defaults.
    pub fn of(probe: Probe, restarting: bool, file_loads: bool) -> Self {
        match probe {
            _ if restarting => Daemon::Busy,
            Probe::NotRunning => Daemon::Stopped,
            _ if !file_loads => Daemon::Busy,
            Probe::NoAnswer => Daemon::Busy,
            Probe::State(DaemonState::Idle) => Daemon::Idle,
            Probe::State(_) => Daemon::Busy,
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
        self.change_in(now, RESTART_DEBOUNCE);
    }

    /// Try the restart again after `delay`.
    pub fn change_in(&mut self, now: Instant, delay: Duration) {
        self.due = Some(now + delay);
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

/// The dotted keys whose values differ between `before` and `after`, such
/// as `llm.ollama_model`, to name what changed outside the window.
pub fn changed_keys<T: Serialize>(before: &T, after: &T) -> Vec<String> {
    let (Ok(before), Ok(after)) = (serde_json::to_value(before), serde_json::to_value(after))
    else {
        return Vec::new();
    };
    let mut keys = Vec::new();
    differences("", &before, &after, &mut keys);
    keys
}

fn differences(prefix: &str, before: &Value, after: &Value, keys: &mut Vec<String>) {
    if before == after {
        return;
    }
    match (before, after) {
        (Value::Object(before), Value::Object(after)) => {
            let mut names: Vec<&String> = before.keys().chain(after.keys()).collect();
            names.sort();
            names.dedup();
            for name in names {
                let key = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}.{}", prefix, name)
                };
                let missing = Value::Null;
                differences(
                    &key,
                    before.get(name).unwrap_or(&missing),
                    after.get(name).unwrap_or(&missing),
                    keys,
                );
            }
        },
        _ => keys.push(prefix.to_string()),
    }
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
    /// The default files, stamped before the window reads them, so a write
    /// between the stamp and the read shows as a change on disk.
    pub fn before_reading() -> Self {
        Self::at(paths::config_path(), DomainVocabulary::default_path())
    }

    pub(super) fn at(config_path: PathBuf, vocabulary_path: PathBuf) -> Self {
        Self {
            config_stamp: stamp(&config_path),
            vocabulary_stamp: stamp(&vocabulary_path),
            config_path,
            vocabulary_path,
            config_error: None,
            vocabulary_error: None,
        }
    }

    /// Read the configuration file again, as when it loads again after it
    /// did not, and drop the load error.
    pub fn reload_config(&mut self) -> Result<Config> {
        let read_from = stamp(&self.config_path);
        let config = if self.config_path.exists() {
            Config::load_from_file(&self.config_path)?
        } else {
            Config::defaults()?
        };
        self.config_stamp = read_from;
        self.config_error = None;
        Ok(config)
    }

    /// Read the vocabulary file again, as after a word was learned outside
    /// the window's own writes.
    pub fn reload_vocabulary(&mut self) -> Result<DomainVocabulary> {
        let read_from = stamp(&self.vocabulary_path);
        let vocabulary = if self.vocabulary_path.exists() {
            DomainVocabulary::load_from_file(&self.vocabulary_path)?
        } else {
            DomainVocabulary::default()
        };
        self.vocabulary_stamp = read_from;
        Ok(vocabulary)
    }

    /// The `[history]` table of a configuration file that does not load,
    /// when that table still reads as history settings, so a replacement
    /// deletes no kept audio and no transcripts.
    pub fn history_in_file(&self) -> Option<HistoryConfig> {
        let text = std::fs::read_to_string(&self.config_path).ok()?;
        let table: toml::Table = toml::from_str(&text).ok()?;
        let history = table.get("history")?.clone();
        let history: HistoryConfig = history.try_into().ok()?;
        let mut check = Config::defaults().ok()?;
        check.history = history.clone();
        check.validate().ok()?;
        Some(history)
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
        let read_from = stamp(&path);
        let (base, next, outside) = if path.exists() && read_from != self.config_stamp {
            let theirs = Config::load_from_file(&path)
                .context("The file changed on disk and does not load")?;
            let merged = rebase(saved, ours, &theirs)?;
            let outside = changed_keys(saved, &theirs);
            (theirs, merged, outside)
        } else {
            (saved.clone(), ours.clone(), Vec::new())
        };
        let text = match next.to_toml() {
            Ok(text) => text,
            Err(e) if !outside.is_empty() => {
                return Err(e).with_context(|| {
                    format!(
                        "The file changed outside the window ({}), and the window's change \
                         does not fit it",
                        outside.join(", ")
                    )
                })
            },
            Err(e) => return Err(e),
        };
        transfer::replace_file(&path, text.as_bytes())?;
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

/// Where a close of the window stands.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CloseFlow {
    /// A close is under way; the window closes once nothing holds it
    pub closing: bool,
    /// The user saw what a close loses; the next close discards it
    pub warned: bool,
    /// This frame commits the focused field for a close, so its save does
    /// not stop the close
    pub commit_frame: bool,
}

impl CloseFlow {
    /// A save of a change. A change by the user keeps the window open and
    /// makes the next close ask again; a change by the window itself, such
    /// as a model that finished its download, leaves the close as it is.
    pub fn changed(&mut self, by_user: bool) {
        if !by_user {
            return;
        }
        self.warned = false;
        if !self.commit_frame {
            self.closing = false;
        }
    }

    /// A save failed: the window stays open with the message, even after a
    /// second close.
    pub fn failed(&mut self) {
        self.closing = false;
        self.warned = false;
    }
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

/// A close asks for an owed restart at once, unless a broken file or a
/// silent daemon holds it: a held restart keeps its own pace, so a close
/// does not start a try each frame.
pub fn hurry_on_close(held_by_file: bool, held_tries: u32) -> bool {
    !held_by_file && held_tries == 0
}

/// The window owes the daemon a restart that can happen: a change waits and
/// no broken file holds it back, or a try is under way.
pub fn restart_owed(pending: bool, file_broken: bool, attempting: bool) -> bool {
    (pending && !file_broken) || attempting
}

/// A daemon in safe mode needs a restart once the configuration file loads
/// again, unless one is on its way.
pub fn leave_safe_mode(safe_mode: bool, config_loads: bool, restart_on_its_way: bool) -> bool {
    safe_mode && config_loads && !restart_on_its_way
}

/// After this many tries that the file or a silent daemon holds back, the
/// restart tries at the normal poll rate.
pub const BACKOFF_AFTER: u32 = 3;

/// The wait before the next restart try, and the redraw rate meanwhile:
/// quick at first, then at the normal poll rate.
pub fn retry_delay(held_tries: u32, fast: Duration, normal: Duration) -> Duration {
    if held_tries >= BACKOFF_AFTER {
        normal
    } else {
        fast
    }
}

/// How often the window reads the machine: quickly during a take, a
/// restart, or a key capture, also while a held restart backs off; at the
/// pace of the held restart while one waits; else at the normal rate.
pub fn poll_rate(
    active: bool,
    restart_pending: bool,
    held_tries: u32,
    fast: Duration,
    normal: Duration,
) -> Duration {
    if active {
        fast
    } else if restart_pending {
        retry_delay(held_tries, fast, normal)
    } else {
        normal
    }
}

/// How long Undo stays in the footer.
pub const UNDO_DURATION: Duration = Duration::from_secs(10);

/// The settings before the last removal, for Undo.
#[derive(Debug, Clone)]
pub struct Undo {
    pub label: String,
    before: (Config, DomainVocabulary),
    /// The settings right after the removal, before its save merged any
    /// outside change: the difference from `before` is the removal alone
    after: (Config, DomainVocabulary),
    at: Instant,
    /// The settings after the removal's save; any later change ends the Undo
    settled: Option<(Config, DomainVocabulary)>,
}

impl Undo {
    pub fn new(
        label: String,
        before: (Config, DomainVocabulary),
        after: (Config, DomainVocabulary),
        at: Instant,
    ) -> Self {
        Self {
            label,
            before,
            after,
            at,
            settled: None,
        }
    }

    /// The settings with the removal put back on top of `now`: what the
    /// removal changed goes back, and everything else in `now` stays.
    pub fn restore(&self, now: &(Config, DomainVocabulary)) -> Result<(Config, DomainVocabulary)> {
        let config = rebase(&self.after.0, &self.before.0, &now.0)?;
        config.validate()?;
        let vocabulary = rebase(&self.after.1, &self.before.1, &now.1)?;
        Ok((config, vocabulary))
    }

    /// Whether Undo still applies: within [`UNDO_DURATION`], and with no
    /// change after the removal's save. The first call notes the settings
    /// after that save.
    pub fn still(&mut self, config: &Config, vocabulary: &DomainVocabulary, now: Instant) -> bool {
        if now.saturating_duration_since(self.at) > UNDO_DURATION {
            return false;
        }
        match &self.settled {
            None => {
                self.settled = Some((config.clone(), vocabulary.clone()));
                true
            },
            Some((settled_config, settled_vocabulary)) => {
                settled_config == config && settled_vocabulary == vocabulary
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
        let mut files = files(&dir);
        files.config_error = Some("Invalid config file".into());
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
    fn undo_puts_back_the_removal_and_keeps_an_outside_change() {
        let start = Instant::now();
        let vocabulary = DomainVocabulary::default();
        let mut before = Config::defaults().unwrap();
        before.insertion.clipboard_apps = vec!["kitty".into(), "foot".into()];
        let mut after = before.clone();
        after.insertion.clipboard_apps.retain(|app| app != "kitty");
        let mut undo = Undo::new(
            "Removed kitty".into(),
            (before, vocabulary.clone()),
            (after.clone(), vocabulary.clone()),
            start,
        );
        // The removal's save found an outside change on disk and merged it.
        let mut merged = after.clone();
        merged.llm.ollama_model = "qwen3".into();
        assert!(undo.still(&merged, &vocabulary, start));

        let (config, _) = undo.restore(&(merged, vocabulary)).unwrap();
        assert_eq!(config.insertion.clipboard_apps, ["kitty", "foot"]);
        assert_eq!(config.llm.ollama_model, "qwen3", "the outside change stays");
    }

    #[test]
    fn undo_lasts_until_the_next_change_or_ten_seconds() {
        let start = Instant::now();
        let before = Config::defaults().unwrap();
        let vocabulary = DomainVocabulary::default();
        let mut after = before.clone();
        after.insertion.clipboard_apps.clear();
        let pair = |config: &Config| (config.clone(), vocabulary.clone());
        let mut undo = Undo::new("Removed kitty".into(), pair(&before), pair(&after), start);
        assert!(undo.still(&after, &vocabulary, start), "notes the save");
        assert!(undo.still(&after, &vocabulary, start + Duration::from_secs(5)));
        assert!(!undo.still(&after, &vocabulary, start + UNDO_DURATION * 2));
        let mut undo = Undo::new("Removed kitty".into(), pair(&before), pair(&after), start);
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
    fn a_download_that_finishes_keeps_a_close_that_waits() {
        let mut flow = CloseFlow {
            closing: true,
            ..CloseFlow::default()
        };
        flow.changed(false);
        assert!(flow.closing, "the window's own change");
        flow.changed(true);
        assert!(!flow.closing, "the user's change keeps the window open");
    }

    #[test]
    fn the_field_a_close_commits_does_not_stop_the_close() {
        let mut flow = CloseFlow {
            closing: true,
            commit_frame: true,
            ..CloseFlow::default()
        };
        flow.changed(true);
        assert!(flow.closing);
    }

    #[test]
    fn a_failed_save_after_the_second_close_keeps_the_window_open() {
        let mut flow = CloseFlow {
            closing: true,
            warned: true,
            commit_frame: true,
        };
        flow.failed();
        assert!(!flow.closing);
        assert!(!flow.warned, "the next close asks again");
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
    fn a_close_does_not_wait_for_a_restart_that_a_broken_file_holds() {
        assert!(hurry_on_close(false, 0));
        assert!(!hurry_on_close(true, 1), "a broken file holds it");
        assert!(!hurry_on_close(false, 2), "a silent daemon holds it");
        assert!(restart_owed(true, false, false));
        assert!(!restart_owed(true, true, false), "so the first close goes");
        assert!(
            restart_owed(false, true, true),
            "a try under way still counts"
        );
        let close = CloseState {
            unsaved: false,
            restart_owed: restart_owed(true, true, false),
            ..CloseState::default()
        };
        assert_eq!(close_step(close), CloseStep::Close);
    }

    #[test]
    fn a_daemon_in_safe_mode_restarts_once_the_file_loads() {
        assert!(leave_safe_mode(true, true, false));
        assert!(
            !leave_safe_mode(true, false, false),
            "the file is still broken"
        );
        assert!(!leave_safe_mode(false, true, false), "not in safe mode");
        assert!(
            !leave_safe_mode(true, true, true),
            "a restart is on its way"
        );
    }

    #[test]
    fn a_file_that_loads_again_drops_its_error() {
        let dir = tempfile::tempdir().unwrap();
        let mut files = files(&dir);
        files.config_error = Some("Invalid config file".into());
        let mut fixed = Config::defaults().unwrap();
        fixed.tray.enabled = false;
        std::fs::write(files.config_path(), fixed.to_toml().unwrap()).unwrap();
        assert_eq!(files.reload_config().unwrap(), fixed);
        assert_eq!(files.config_error, None);
        let mut ours = fixed.clone();
        ours.llm.ollama_model = "qwen3".into();
        let (base, _) = files.write_config(&fixed, &ours).unwrap();
        assert_eq!(base, fixed, "no merge after the reload");
    }

    #[test]
    fn a_take_keeps_the_fast_poll_while_a_held_restart_backs_off() {
        let fast = Duration::from_millis(150);
        let normal = Duration::from_millis(700);
        let held = BACKOFF_AFTER + 2;
        assert_eq!(
            poll_rate(true, true, held, fast, normal),
            fast,
            "a take runs"
        );
        assert_eq!(poll_rate(false, true, held, fast, normal), normal);
        assert_eq!(poll_rate(false, true, 0, fast, normal), fast);
        assert_eq!(poll_rate(false, false, 0, fast, normal), normal);
    }

    #[test]
    fn a_held_restart_backs_off_to_the_normal_poll() {
        let fast = Duration::from_millis(150);
        let normal = Duration::from_millis(700);
        assert_eq!(retry_delay(0, fast, normal), fast);
        assert_eq!(retry_delay(BACKOFF_AFTER - 1, fast, normal), fast);
        assert_eq!(retry_delay(BACKOFF_AFTER, fast, normal), normal);
        let start = Instant::now();
        let mut debounce = Debounce::default();
        debounce.change_in(start, normal);
        assert!(!debounce.fire(start + fast, Daemon::Idle));
        assert!(debounce.fire(start + normal, Daemon::Idle));
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
        let idle = Probe::State(DaemonState::Idle);
        assert_eq!(Daemon::of(Probe::NotRunning, false, true), Daemon::Stopped);
        assert_eq!(Daemon::of(idle, false, true), Daemon::Idle);
        assert_eq!(Daemon::of(idle, true, true), Daemon::Busy);
        assert_eq!(
            Daemon::of(Probe::NotRunning, true, true),
            Daemon::Busy,
            "a restarting daemon is down for a moment, not stopped"
        );
        assert_eq!(
            Daemon::of(Probe::NoAnswer, false, true),
            Daemon::Busy,
            "a slow answer keeps the restart"
        );
        assert_eq!(
            Daemon::of(idle, false, false),
            Daemon::Busy,
            "no restart onto a file that does not load"
        );
        assert_eq!(Daemon::of(Probe::NotRunning, false, false), Daemon::Stopped);
        for state in [
            DaemonState::Starting,
            DaemonState::Recording { elapsed_ms: 5 },
            DaemonState::Transcribing,
            DaemonState::Inserting,
            DaemonState::Stopping,
        ] {
            assert_eq!(
                Daemon::of(Probe::State(state), false, true),
                Daemon::Busy,
                "{state:?}"
            );
        }
    }

    #[test]
    fn an_outside_change_that_breaks_the_merge_is_named() {
        let dir = tempfile::tempdir().unwrap();
        let mut files = files(&dir);
        let saved = Config::defaults().unwrap();
        files.write_config(&saved, &saved).unwrap();
        let mut outside = saved.clone();
        outside.llm.provider = LlmSetting::OpenAiCompatible;
        outside.llm.server_url = "http://localhost:8080/v1".into();
        outside.llm.server_model = "llama3.2".into();
        std::fs::write(files.config_path(), outside.to_toml().unwrap()).unwrap();

        let mut ours = saved.clone();
        ours.llm.server_url = "localhost:8080".into();
        ours.validate().unwrap();
        let err = format!("{:#}", files.write_config(&saved, &ours).unwrap_err());
        assert!(err.contains("llm.provider"), "{err}");
        assert!(err.contains("changed outside the window"), "{err}");
        let on_disk = Config::load_from_file(files.config_path()).unwrap();
        assert_eq!(on_disk, outside, "nothing written");
    }

    #[test]
    fn changed_keys_are_dotted_and_sorted() {
        let before = Config::defaults().unwrap();
        let mut after = before.clone();
        after.llm.ollama_model = "qwen3".into();
        after.hotkey.languages.insert("F13".into(), "de".into());
        assert_eq!(
            changed_keys(&before, &after),
            ["hotkey.languages.F13", "llm.ollama_model"]
        );
        assert!(changed_keys(&before, &before).is_empty());
    }

    #[test]
    fn a_bad_file_still_gives_its_history_settings() {
        let dir = tempfile::tempdir().unwrap();
        let files = files(&dir);
        std::fs::write(
            files.config_path(),
            "[history]\nkeep_audio_days = 30\nmax_entries = 2000\n[hotkey]\ncombination = \"Ctrl+Bogus\"\n",
        )
        .unwrap();
        let history = files.history_in_file().unwrap();
        assert_eq!((history.keep_audio_days, history.max_entries), (30, 2000));
        std::fs::write(files.config_path(), "[history\nkeep_audio_days = 30\n").unwrap();
        assert_eq!(files.history_in_file(), None, "not TOML at all");
        std::fs::write(files.config_path(), "[history]\nkeep_audio_days = 99999\n").unwrap();
        assert_eq!(files.history_in_file(), None, "out of range");
    }

    #[test]
    fn a_reload_reads_the_learned_word_and_stamps_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut files = files(&dir);
        let saved = DomainVocabulary::default();
        files.write_vocabulary(&saved, &saved).unwrap();
        let mut outside = saved.clone();
        outside.add_technical_term("kubernetes".into(), "Kubernetes".into());
        std::fs::write(
            files.vocabulary_path(),
            serde_json::to_string_pretty(&outside).unwrap(),
        )
        .unwrap();
        let reloaded = files.reload_vocabulary().unwrap();
        assert_eq!(reloaded, outside);
        let mut ours = reloaded.clone();
        ours.add_technical_term("neovim".into(), "Neovim".into());
        let written = files.write_vocabulary(&reloaded, &ours).unwrap();
        assert_eq!(written, ours);
    }
}
