//! API keys live in the environment or in `~/.config/hush/.env`, never in
//! config.toml, so the config file is safe to share.

use super::paths;
use parking_lot::Mutex;
use std::collections::{BTreeSet, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

static LOADED: Mutex<BTreeSet<OsString>> = Mutex::new(BTreeSet::new());

/// `~/.config/hush/.env`
pub fn env_file() -> PathBuf {
    paths::config_dir().join(".env")
}

/// Load `~/.config/hush/.env` (installed daemons) and `./.env` (source
/// checkouts) into the environment. A variable that is already set wins.
pub fn load_env_files() {
    load_with(|| {
        let _ = dotenvy::from_path(env_file());
        let _ = dotenvy::dotenv();
    });
}

/// The variables that [`load_env_files`] added, so that a child process
/// can go without them.
pub fn loaded_from_env_files() -> Vec<OsString> {
    LOADED.lock().iter().cloned().collect()
}

// dotenvy never overrides a set variable, so each name that is new after the
// load came from a file and not from the user's session.
pub(crate) fn load_with(load: impl FnOnce()) {
    let before: HashSet<OsString> = std::env::vars_os().map(|(name, _)| name).collect();
    load();
    LOADED.lock().extend(
        std::env::vars_os()
            .map(|(name, _)| name)
            .filter(|name| !before.contains(name)),
    );
}

/// The key in `var`, from the environment or else from
/// `~/.config/hush/.env`, when it is set and not blank. The file is read
/// each time, so a key added while a window is open is found.
pub fn api_key(var: &str) -> Option<String> {
    api_key_in(&env_file(), var)
}

/// [`api_key`] with `env_file` in place of `~/.config/hush/.env`.
pub fn api_key_in(env_file: &Path, var: &str) -> Option<String> {
    let from_env = std::env::var(var).ok().filter(|key| !key.trim().is_empty());
    from_env.or_else(|| from_file(env_file, var))
}

fn from_file(path: &Path, var: &str) -> Option<String> {
    dotenvy::from_path_iter(path)
        .ok()?
        .filter_map(Result::ok)
        .find(|(name, _)| name == var)
        .map(|(_, value)| value)
        .filter(|key| !key.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_read_from_an_env_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".env");
        std::fs::write(
            &path,
            "# keys\nGROQ_API_KEY=gsk_test\nOPENAI_API_KEY=\"  \"\n",
        )
        .unwrap();
        assert_eq!(
            from_file(&path, "GROQ_API_KEY").as_deref(),
            Some("gsk_test")
        );
        assert_eq!(from_file(&path, "OPENAI_API_KEY"), None, "blank is unset");
        assert_eq!(from_file(&path, "ANTHROPIC_API_KEY"), None);
        assert_eq!(from_file(&dir.path().join("missing"), "GROQ_API_KEY"), None);
    }

    #[test]
    fn only_variables_from_a_file_are_recorded() {
        std::env::set_var("HUSH_TEST_KEYS_SESSION", "from-session");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".env");
        std::fs::write(
            &path,
            "HUSH_TEST_KEYS_FILE=secret\nHUSH_TEST_KEYS_SESSION=from-file\n",
        )
        .unwrap();
        load_with(|| {
            let _ = dotenvy::from_path(&path);
        });
        assert_eq!(
            std::env::var("HUSH_TEST_KEYS_FILE").as_deref(),
            Ok("secret")
        );
        assert_eq!(
            std::env::var("HUSH_TEST_KEYS_SESSION").as_deref(),
            Ok("from-session"),
            "the session wins"
        );
        let loaded = loaded_from_env_files();
        assert!(loaded.contains(&OsString::from("HUSH_TEST_KEYS_FILE")));
        assert!(!loaded.contains(&OsString::from("HUSH_TEST_KEYS_SESSION")));
    }
}
