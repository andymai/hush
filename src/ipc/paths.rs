//! Runtime files: `$XDG_RUNTIME_DIR/hush/`, or `/tmp/hush-<uid>` without a
//! session runtime directory.

use anyhow::{Context, Result};
use std::path::PathBuf;

pub fn runtime_dir() -> PathBuf {
    match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(dir) => PathBuf::from(dir).join("hush"),
        // SAFETY: getuid has no preconditions and cannot fail.
        None => PathBuf::from(format!("/tmp/hush-{}", unsafe { libc::getuid() })),
    }
}

pub fn socket_path() -> PathBuf {
    runtime_dir().join("hush.sock")
}

pub fn pid_path() -> PathBuf {
    runtime_dir().join("hush.pid")
}

/// Create the runtime directory, readable by the owner only.
pub fn ensure_runtime_dir() -> Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let dir = runtime_dir();
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create runtime directory {}", dir.display()))?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
        .with_context(|| format!("Failed to restrict {}", dir.display()))?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_and_pid_live_in_the_runtime_dir() {
        let dir = runtime_dir();
        assert!(socket_path().starts_with(&dir));
        assert!(pid_path().starts_with(&dir));
        assert!(dir.ends_with("hush") || dir.to_string_lossy().contains("/tmp/hush-"));
    }
}
