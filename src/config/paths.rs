//! Filesystem locations for configuration, models, and runtime state.
//!
//! Every path follows the XDG base directory layout so packaged installs,
//! AppImages, and source builds all read and write the same places.

use std::path::{Path, PathBuf};
use tracing::{info, warn};

const APP: &str = "hush";

/// Environment variable that overrides the configuration file location.
pub const CONFIG_ENV: &str = "HUSH_CONFIG";

/// `$XDG_CONFIG_HOME/hush`
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join(APP)
}

/// Whether `$HUSH_CONFIG` (or `--config-file`, which sets it) names the config file.
pub fn config_path_is_explicit() -> bool {
    std::env::var_os(CONFIG_ENV).is_some()
}

/// The configuration file: `$HUSH_CONFIG` when set, else `config_dir()/config.toml`.
pub fn config_path() -> PathBuf {
    std::env::var_os(CONFIG_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| config_dir().join("config.toml"))
}

/// `$XDG_DATA_HOME/hush`
pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from(".local/share"))
        .join(APP)
}

/// `$XDG_CACHE_HOME/hush`
pub fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join(APP)
}

/// `$XDG_STATE_HOME/hush`
pub fn state_dir() -> PathBuf {
    dirs::state_dir()
        .unwrap_or_else(|| PathBuf::from(".local/state"))
        .join(APP)
}

/// Where downloaded Whisper models live: `$XDG_DATA_HOME/hush/models`, or
/// the legacy `$XDG_CACHE_HOME/hush/models` while only that folder exists.
/// It never moves files; [`move_legacy_models`] does.
pub fn models_dir() -> PathBuf {
    choose_models_dir(&data_dir().join("models"), &cache_dir().join("models"))
}

/// Move models from the legacy cache folder into the data folder. The daemon
/// and `hush models` call it once at start. When the move fails (for example
/// across filesystems), the models stay, and [`models_dir`] keeps the legacy
/// folder, so nothing must be downloaded again.
pub fn move_legacy_models() {
    move_models(&data_dir().join("models"), &cache_dir().join("models"));
}

fn choose_models_dir(current: &Path, legacy: &Path) -> PathBuf {
    if current.exists() || !legacy.is_dir() {
        current.to_path_buf()
    } else {
        legacy.to_path_buf()
    }
}

fn move_models(current: &Path, legacy: &Path) {
    if current.exists() || !legacy.is_dir() {
        return;
    }
    match migrate_dir(legacy, current) {
        Ok(()) => info!(
            "Moved models from {} to {}",
            legacy.display(),
            current.display()
        ),
        Err(e) => warn!(
            "Could not move models from {} to {} ({}); using the legacy location",
            legacy.display(),
            current.display(),
            e
        ),
    }
}

fn migrate_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(from, to)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_path_honors_override() {
        crate::test_support::in_temp_home!(config_path_honors_override);
        let dir = tempfile::tempdir().unwrap();
        let custom = dir.path().join("custom.toml");
        std::env::set_var(CONFIG_ENV, &custom);
        assert_eq!(config_path(), custom);
        assert!(config_path_is_explicit());
        let err = crate::Config::load().unwrap_err();
        assert!(err.to_string().contains("not found"), "{err}");
        std::env::remove_var(CONFIG_ENV);
        assert!(!config_path_is_explicit());
        assert!(config_path().ends_with("hush/config.toml"));
    }

    #[test]
    fn an_existing_current_dir_wins_and_is_never_replaced() {
        let root = tempfile::tempdir().unwrap();
        let current = root.path().join("data/models");
        let legacy = root.path().join("cache/models");
        std::fs::create_dir_all(&current).unwrap();
        std::fs::create_dir_all(&legacy).unwrap();
        assert_eq!(choose_models_dir(&current, &legacy), current);
        move_models(&current, &legacy);
        assert!(
            legacy.exists(),
            "an existing current dir never triggers a move"
        );
    }

    #[test]
    fn a_read_uses_the_legacy_dir_and_never_moves_it() {
        let root = tempfile::tempdir().unwrap();
        let current = root.path().join("data/models");
        let legacy = root.path().join("cache/models");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("ggml-base.bin"), b"model").unwrap();

        assert_eq!(choose_models_dir(&current, &legacy), legacy);
        assert!(legacy.join("ggml-base.bin").exists());
        assert!(!current.exists());
    }

    #[test]
    fn the_move_takes_the_legacy_dir_once() {
        let root = tempfile::tempdir().unwrap();
        let current = root.path().join("data/models");
        let legacy = root.path().join("cache/models");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("ggml-base.bin"), b"model").unwrap();

        move_models(&current, &legacy);
        assert!(current.join("ggml-base.bin").exists());
        assert!(!legacy.exists());
        assert_eq!(choose_models_dir(&current, &legacy), current);
        move_models(&current, &legacy);
        assert!(current.join("ggml-base.bin").exists());
    }

    #[test]
    fn a_failed_move_keeps_the_legacy_dir() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("data"), b"not a folder").unwrap();
        let current = root.path().join("data/models");
        let legacy = root.path().join("cache/models");
        std::fs::create_dir_all(&legacy).unwrap();

        move_models(&current, &legacy);
        assert!(legacy.exists());
        assert_eq!(choose_models_dir(&current, &legacy), legacy);
    }

    #[test]
    fn nothing_on_disk_means_the_current_dir_and_no_new_folder() {
        let root = tempfile::tempdir().unwrap();
        let current = root.path().join("data/models");
        let legacy = root.path().join("cache/models");
        assert_eq!(choose_models_dir(&current, &legacy), current);
        move_models(&current, &legacy);
        assert!(!current.exists(), "nothing to move creates nothing");
    }
}
