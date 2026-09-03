//! One daemon per user, enforced with a PID file in the runtime directory.

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct PidLock {
    path: PathBuf,
    pid: u32,
}

impl PidLock {
    /// Write our PID, refusing when the recorded PID still belongs to a live process.
    pub fn acquire(path: &Path) -> Result<Self> {
        if let Some(pid) = read_pid(path) {
            if process_alive(pid) && pid != std::process::id() {
                return Err(anyhow!(
                    "Another hush daemon is already running (PID {}). Stop it with: hush daemon stop",
                    pid
                ));
            }
        }
        let pid = std::process::id();
        std::fs::write(path, pid.to_string())
            .with_context(|| format!("Failed to write PID file {}", path.display()))?;
        Ok(Self {
            path: path.to_path_buf(),
            pid,
        })
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }
}

impl Drop for PidLock {
    fn drop(&mut self) {
        if read_pid(&self.path) == Some(self.pid) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

pub fn read_pid(path: &Path) -> Option<u32> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// A process we may not signal (EPERM) still exists.
pub fn process_alive(pid: u32) -> bool {
    // SAFETY: kill with signal 0 only checks for the process's existence.
    if unsafe { libc::kill(pid as libc::pid_t, 0) } == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquire_writes_and_drop_removes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hush.pid");
        {
            let lock = PidLock::acquire(&path).unwrap();
            assert_eq!(read_pid(&path), Some(lock.pid()));
        }
        assert!(!path.exists());
    }

    #[test]
    fn stale_pid_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hush.pid");
        std::fs::write(&path, "999999999").unwrap();
        let lock = PidLock::acquire(&path).unwrap();
        assert_eq!(read_pid(&path), Some(lock.pid()));
    }

    #[test]
    fn live_foreign_pid_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hush.pid");
        // PID 1 always exists.
        std::fs::write(&path, "1").unwrap();
        let err = PidLock::acquire(&path).unwrap_err();
        assert!(err.to_string().contains("already running"));
    }
}
