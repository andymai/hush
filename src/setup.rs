//! The first run: which steps of the setup are done, skipped, or still to
//! do. `$XDG_STATE_HOME/hush/setup.toml` keeps them, so the setup resumes at
//! the first step that is still to do.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Step {
    Access,
    Model,
    Hotkey,
    TryIt,
    Tune,
    Autostart,
    Privacy,
}

impl Step {
    pub const ALL: [Step; 7] = [
        Step::Access,
        Step::Model,
        Step::Hotkey,
        Step::TryIt,
        Step::Tune,
        Step::Autostart,
        Step::Privacy,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Step::Access => "Keyboard access",
            Step::Model => "Speech model",
            Step::Hotkey => "Dictation key",
            Step::TryIt => "Try it",
            Step::Tune => "Hardware tune",
            Step::Autostart => "Start at login",
            Step::Privacy => "Privacy",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Progress {
    #[default]
    Pending,
    Done,
    Skipped,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SetupState {
    #[serde(default)]
    pub steps: BTreeMap<Step, Progress>,
    /// The daemon told this install about the setup once
    #[serde(default)]
    pub notified: bool,
}

/// What Hush finds on the machine, for an install without `setup.toml`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Facts {
    pub config_file: bool,
    /// A downloaded model, a model file of its own, or a cloud backend
    pub model: bool,
    pub history: bool,
    pub access: bool,
    pub tuned: bool,
    pub autostart: bool,
}

impl SetupState {
    pub fn progress(&self, step: Step) -> Progress {
        self.steps.get(&step).copied().unwrap_or_default()
    }

    pub fn set(&mut self, step: Step, progress: Progress) {
        self.steps.insert(step, progress);
    }

    /// The first step that is still to do.
    pub fn next(&self) -> Option<Step> {
        Step::ALL
            .into_iter()
            .find(|step| self.progress(*step) == Progress::Pending)
    }

    /// A new install starts with every step to do. An install that Hush
    /// already ran on (a config file, a model, or a history) takes each step
    /// from what is there, so the setup asks only for what is missing.
    pub fn derive(facts: &Facts) -> Self {
        let mut state = Self::default();
        if !(facts.config_file || facts.model || facts.history) {
            return state;
        }
        let done = |yes: bool| {
            if yes {
                Progress::Done
            } else {
                Progress::Pending
            }
        };
        state.set(Step::Access, done(facts.access));
        state.set(Step::Model, done(facts.model));
        state.set(Step::Hotkey, done(facts.config_file));
        state.set(Step::TryIt, done(facts.history));
        state.set(Step::Tune, done(facts.tuned));
        state.set(Step::Autostart, done(facts.autostart));
        state.set(Step::Privacy, done(facts.config_file));
        state
    }

    /// `None` when the file is absent or does not parse.
    pub fn load(path: &Path) -> Option<Self> {
        toml::from_str(&std::fs::read_to_string(path).ok()?).ok()
    }

    /// Write the file whole, so a crash leaves the old one or the new one.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("Failed to create {}", dir.display()))?;
        }
        let text = toml::to_string(self).context("Failed to write the setup state")?;
        let partial = path.with_extension("toml.partial");
        std::fs::write(&partial, text)
            .with_context(|| format!("Failed to write {}", partial.display()))?;
        std::fs::rename(&partial, path)
            .with_context(|| format!("Failed to replace {}", path.display()))
    }
}

/// `$XDG_STATE_HOME/hush/setup.toml`
pub fn path() -> PathBuf {
    crate::config::paths::state_dir().join("setup.toml")
}

/// Holds the lock that keeps one setup window open; the lock goes with it.
pub struct SetupLock {
    _file: File,
}

/// `None` when another setup window holds the lock.
pub fn lock() -> Result<Option<SetupLock>> {
    let dir = crate::ipc::paths::runtime_dir();
    std::fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    lock_at(&dir.join("setup.lock"))
}

fn lock_at(path: &Path) -> Result<Option<SetupLock>> {
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .with_context(|| format!("Failed to open {}", path.display()))?;
    // SAFETY: flock takes a file descriptor that `file` owns for the call.
    let taken = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0;
    Ok(taken.then_some(SetupLock { _file: file }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_install_does_every_step() {
        let state = SetupState::derive(&Facts::default());
        assert_eq!(state.next(), Some(Step::Access));
        assert!(Step::ALL
            .iter()
            .all(|step| state.progress(*step) == Progress::Pending));
    }

    #[test]
    fn an_existing_install_asks_only_for_what_is_missing() {
        let state = SetupState::derive(&Facts {
            config_file: true,
            model: true,
            history: true,
            access: true,
            tuned: false,
            autostart: true,
        });
        assert_eq!(state.next(), Some(Step::Tune));
        assert_eq!(state.progress(Step::Access), Progress::Done);
        assert_eq!(state.progress(Step::Privacy), Progress::Done);

        let imported = SetupState::derive(&Facts {
            config_file: true,
            ..Facts::default()
        });
        assert_eq!(
            imported.next(),
            Some(Step::Access),
            "a config that `config import` wrote on a new machine"
        );
        assert_eq!(imported.progress(Step::Hotkey), Progress::Done);
    }

    #[test]
    fn the_state_survives_a_round_trip_and_resumes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hush").join("setup.toml");
        assert_eq!(SetupState::load(&path), None);
        let mut state = SetupState::default();
        state.set(Step::Access, Progress::Done);
        state.set(Step::Model, Progress::Skipped);
        state.save(&path).unwrap();
        let loaded = SetupState::load(&path).unwrap();
        assert_eq!(loaded, state);
        assert_eq!(loaded.next(), Some(Step::Hotkey));
        std::fs::write(&path, "steps = 7").unwrap();
        assert_eq!(SetupState::load(&path), None, "a broken file starts over");
    }

    #[test]
    fn one_setup_window_holds_the_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("setup.lock");
        let first = lock_at(&path).unwrap();
        assert!(first.is_some());
        assert!(lock_at(&path).unwrap().is_none(), "a second window");
        drop(first);
        assert!(lock_at(&path).unwrap().is_some(), "after the first closed");
    }
}
