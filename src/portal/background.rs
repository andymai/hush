//! Starting the daemon at login through the Background portal. A sandboxed
//! app cannot write `~/.config/autostart`, so the portal writes the entry,
//! after the user agrees.

use super::PortalError;
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tracing::debug;

/// The command the entry runs, inside the sandbox.
pub const COMMAND: [&str; 4] = ["hush", "daemon", "start", "--foreground"];
const REASON: &str = "Hush starts at login, so the dictation key works at once.";
const MARKER: &str = "background-autostart";

#[async_trait]
pub trait BackgroundPortal: Send + Sync {
    /// Asks for background running with or without autostart. Returns
    /// whether the desktop starts Hush at login.
    async fn request(
        &self,
        autostart: bool,
        command: &[&str],
        reason: &str,
    ) -> Result<bool, PortalError>;
}

pub struct AshpdBackground;

#[async_trait]
impl BackgroundPortal for AshpdBackground {
    async fn request(
        &self,
        autostart: bool,
        command: &[&str],
        reason: &str,
    ) -> Result<bool, PortalError> {
        let error = |e| PortalError::from_ashpd("Background", e);
        let response = ashpd::desktop::background::Background::request()
            .reason(reason)
            .auto_start(autostart)
            .command(command.iter().copied())
            .send()
            .await
            .map_err(error)?
            .response()
            .map_err(error)?;
        Ok(response.auto_start())
    }
}

/// The portal does not report the state later, so Hush keeps a mark of the
/// last answer.
pub fn marker_path() -> PathBuf {
    crate::config::paths::data_dir().join(MARKER)
}

/// Whether the desktop starts Hush at login, as of the last request.
pub fn autostart_on() -> bool {
    marker_path().exists()
}

pub async fn set_autostart_with(
    portal: &dyn BackgroundPortal,
    marker: &Path,
    on: bool,
) -> Result<bool, PortalError> {
    let granted = portal.request(on, &COMMAND, REASON).await?;
    if granted {
        if let Some(dir) = marker.parent() {
            std::fs::create_dir_all(dir).map_err(|e| PortalError::Failed(e.to_string()))?;
        }
        std::fs::write(marker, "on\n").map_err(|e| PortalError::Failed(e.to_string()))?;
    } else if let Err(e) = std::fs::remove_file(marker) {
        debug!("No autostart mark to remove: {}", e);
    }
    Ok(granted)
}

/// Turns autostart on or off through the portal. Turning it on fails when
/// the user says no.
pub fn set_autostart(on: bool) -> anyhow::Result<()> {
    let marker = marker_path();
    let granted =
        super::block_on(async move { set_autostart_with(&AshpdBackground, &marker, on).await })?
            .map_err(|e| anyhow::anyhow!("{}", e))?;
    if on && !granted {
        return Err(anyhow::anyhow!(
            "the desktop did not allow Hush to start at login"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;

    struct Answer {
        grants: bool,
        asked: Mutex<Vec<(bool, Vec<String>)>>,
    }

    #[async_trait]
    impl BackgroundPortal for Answer {
        async fn request(
            &self,
            autostart: bool,
            command: &[&str],
            _reason: &str,
        ) -> Result<bool, PortalError> {
            self.asked
                .lock()
                .push((autostart, command.iter().map(|s| s.to_string()).collect()));
            Ok(self.grants && autostart)
        }
    }

    #[tokio::test]
    async fn the_portal_answer_sets_the_mark() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("hush").join(MARKER);
        let yes = Answer {
            grants: true,
            asked: Mutex::new(Vec::new()),
        };
        assert!(set_autostart_with(&yes, &marker, true).await.unwrap());
        assert!(marker.exists());
        assert_eq!(
            yes.asked.lock()[0],
            (
                true,
                COMMAND.iter().map(|s| s.to_string()).collect::<Vec<_>>()
            ),
            "the entry starts the daemon"
        );
        assert!(!set_autostart_with(&yes, &marker, false).await.unwrap());
        assert!(!marker.exists(), "off removes the mark");

        let no = Answer {
            grants: false,
            asked: Mutex::new(Vec::new()),
        };
        assert!(!set_autostart_with(&no, &marker, true).await.unwrap());
        assert!(!marker.exists(), "a refusal sets no mark");
    }
}
