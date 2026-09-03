//! Filesystem locations for configuration, models, and runtime state.
//!
//! Every path follows the XDG base directory layout so packaged installs,
//! AppImages, and source builds all read and write the same places.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
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

/// Where downloaded Whisper models live.
///
/// Models used to be stored under the cache directory. The first call moves
/// that directory into the data directory; if the move fails (for example
/// across filesystems) the legacy location keeps being used so nothing has to
/// be downloaded again.
pub fn models_dir() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| resolve_models_dir(&data_dir().join("models"), &cache_dir().join("models")))
        .clone()
}

fn resolve_models_dir(current: &Path, legacy: &Path) -> PathBuf {
    if current.exists() || !legacy.is_dir() {
        return current.to_path_buf();
    }
    match migrate_dir(legacy, current) {
        Ok(()) => {
            info!(
                "Moved models from {} to {}",
                legacy.display(),
                current.display()
            );
            current.to_path_buf()
        },
        Err(e) => {
            warn!(
                "Could not move models from {} to {} ({}); using the legacy location",
                legacy.display(),
                current.display(),
                e
            );
            legacy.to_path_buf()
        },
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
    fn resolve_prefers_existing_current_dir() {
        let root = tempfile::tempdir().unwrap();
        let current = root.path().join("data/models");
        let legacy = root.path().join("cache/models");
        std::fs::create_dir_all(&current).unwrap();
        std::fs::create_dir_all(&legacy).unwrap();
        assert_eq!(resolve_models_dir(&current, &legacy), current);
        assert!(
            legacy.exists(),
            "an existing current dir never triggers a move"
        );
    }

    #[test]
    fn resolve_moves_legacy_dir_once() {
        let root = tempfile::tempdir().unwrap();
        let current = root.path().join("data/models");
        let legacy = root.path().join("cache/models");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("ggml-base.bin"), b"model").unwrap();

        assert_eq!(resolve_models_dir(&current, &legacy), current);
        assert!(current.join("ggml-base.bin").exists());
        assert!(!legacy.exists());
        assert_eq!(resolve_models_dir(&current, &legacy), current);
    }

    #[test]
    fn resolve_uses_current_when_nothing_exists() {
        let root = tempfile::tempdir().unwrap();
        let current = root.path().join("data/models");
        let legacy = root.path().join("cache/models");
        assert_eq!(resolve_models_dir(&current, &legacy), current);
        assert!(!current.exists(), "resolution never creates the directory");
    }
}
