//! API keys live in the environment or in `~/.config/hush/.env`, never in
//! config.toml, so the config file is safe to share.

use super::paths;
use std::path::Path;

/// Load `~/.config/hush/.env` (installed daemons) and `./.env` (source
/// checkouts) into the environment. A variable that is already set wins.
pub fn load_env_files() {
    let _ = dotenvy::from_path(paths::config_dir().join(".env"));
    let _ = dotenvy::dotenv();
}

/// The key in `var`, from the environment or else from
/// `~/.config/hush/.env`, when it is set and not blank. The file is read
/// each time, so a key added while a window is open is found.
pub fn api_key(var: &str) -> Option<String> {
    let from_env = std::env::var(var).ok().filter(|key| !key.trim().is_empty());
    from_env.or_else(|| from_file(&paths::config_dir().join(".env"), var))
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
}
