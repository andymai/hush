//! The restore token of the RemoteDesktop portal. With persist mode 2, the
//! portal gives a token at each start, and the next start that brings the
//! token shows no consent dialog. The token grants keyboard access, so only
//! the user can read its file.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

const FILE: &str = "remote-desktop-token";

#[derive(Debug, Clone)]
pub struct TokenStore {
    path: PathBuf,
}

impl TokenStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// In the app's data folder: `~/.var/app/<id>/data/hush` in Flatpak.
    pub fn default_path() -> PathBuf {
        crate::config::paths::data_dir().join(FILE)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Option<String> {
        let token = fs::read_to_string(&self.path).ok()?;
        let token = token.trim();
        (!token.is_empty()).then(|| token.to_string())
    }

    /// Writes a new file with mode 0600 and moves it over the old one, so
    /// no reader sees half a token, and the token is never readable by
    /// another user, also for a moment.
    pub fn save(&self, token: &str) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let temporary = self.path.with_extension("new");
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&temporary)?;
        file.write_all(token.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temporary, &self.path)
    }

    pub fn clear(&self) -> io::Result<()> {
        match fs::remove_file(&self.path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn a_token_survives_a_restart_in_a_private_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = TokenStore::new(dir.path().join("data/hush").join(FILE));
        assert_eq!(store.load(), None);
        store.save("token-1").unwrap();
        assert_eq!(store.load().as_deref(), Some("token-1"));
        let mode = fs::metadata(store.path()).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "only the user reads the token");

        store.save("token-2").unwrap();
        assert_eq!(
            TokenStore::new(store.path()).load().as_deref(),
            Some("token-2"),
            "a new store, as after a restart, reads the last token"
        );
        assert!(!store.path().with_extension("new").exists());
    }

    #[test]
    fn an_empty_or_cleared_token_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let store = TokenStore::new(dir.path().join(FILE));
        fs::write(store.path(), " \n").unwrap();
        assert_eq!(store.load(), None);
        store.save("token").unwrap();
        store.clear().unwrap();
        assert_eq!(store.load(), None);
        store.clear().unwrap();
    }
}
