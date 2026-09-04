//! Daemon side of the socket. Each connection carries one command and gets
//! one reply. Commands that change state are forwarded to the session loop;
//! the reply names the state that request leads to.

use super::protocol::{DaemonCommand, DaemonResponse, DaemonState};
use crate::text_processing::learn;
use anyhow::{Context, Result};
use parking_lot::Mutex;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::task::JoinHandle;
use tracing::{debug, warn};

/// What the session loop consumes, from the hotkey thread and from IPC alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionCommand {
    StartRecording,
    StopRecording,
    CancelRecording,
    /// Type the last transcript again
    PasteLast,
    /// Learn the current selection (from an action chord)
    Learn,
    /// The vocabulary file changed
    ReloadVocabulary,
    Quit,
}

enum Phase {
    Idle,
    Recording(Instant),
    Transcribing,
    Inserting,
    Stopping,
}

/// Current phase and the last transcript, written by the session and read
/// by IPC.
pub struct SharedState {
    phase: Mutex<Phase>,
    last_text: Mutex<Option<String>>,
}

impl Default for SharedState {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedState {
    pub fn new() -> Self {
        Self {
            phase: Mutex::new(Phase::Idle),
            last_text: Mutex::new(None),
        }
    }

    pub fn set_last_text(&self, text: String) {
        *self.last_text.lock() = Some(text);
    }

    pub fn last_text(&self) -> Option<String> {
        self.last_text.lock().clone()
    }

    pub fn set_idle(&self) {
        *self.phase.lock() = Phase::Idle;
    }

    pub fn set_recording(&self) {
        *self.phase.lock() = Phase::Recording(Instant::now());
    }

    pub fn set_transcribing(&self) {
        *self.phase.lock() = Phase::Transcribing;
    }

    pub fn set_inserting(&self) {
        *self.phase.lock() = Phase::Inserting;
    }

    pub fn set_stopping(&self) {
        *self.phase.lock() = Phase::Stopping;
    }

    pub fn is_recording(&self) -> bool {
        matches!(*self.phase.lock(), Phase::Recording(_))
    }

    pub fn snapshot(&self) -> DaemonState {
        match *self.phase.lock() {
            Phase::Idle => DaemonState::Idle,
            Phase::Recording(since) => DaemonState::Recording {
                elapsed_ms: since.elapsed().as_millis() as u64,
            },
            Phase::Transcribing => DaemonState::Transcribing,
            Phase::Inserting => DaemonState::Inserting,
            Phase::Stopping => DaemonState::Stopping,
        }
    }
}

/// Removes the socket file and stops accepting when dropped.
pub struct ServerGuard {
    path: PathBuf,
    task: JoinHandle<()>,
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        self.task.abort();
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Bind the socket and serve until the guard is dropped.
pub async fn start(
    path: &Path,
    shared: Arc<SharedState>,
    control: mpsc::Sender<SessionCommand>,
) -> Result<ServerGuard> {
    // The PID lock already proved no daemon is alive, so a leftover socket is stale.
    if path.exists() {
        std::fs::remove_file(path)
            .with_context(|| format!("Failed to remove stale socket {}", path.display()))?;
    }
    let listener =
        UnixListener::bind(path).with_context(|| format!("Failed to bind {}", path.display()))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("Failed to restrict {}", path.display()))?;
    debug!("IPC listening on {}", path.display());

    let task = tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let shared = Arc::clone(&shared);
                    let control = control.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_client(stream, &shared, &control).await {
                            warn!("IPC client error: {}", e);
                        }
                    });
                },
                Err(e) => {
                    warn!("IPC accept failed: {}", e);
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                },
            }
        }
    });

    Ok(ServerGuard {
        path: path.to_path_buf(),
        task,
    })
}

async fn handle_client(
    stream: UnixStream,
    shared: &SharedState,
    control: &mpsc::Sender<SessionCommand>,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut line = String::new();
    BufReader::new(reader)
        .read_line(&mut line)
        .await
        .context("Failed to read IPC command")?;
    let response = match serde_json::from_str::<DaemonCommand>(line.trim()) {
        Ok(command) => respond(command, shared, control),
        Err(e) => DaemonResponse::error(format!("Invalid command: {}", e)),
    };
    let mut payload = serde_json::to_string(&response)?;
    payload.push('\n');
    writer
        .write_all(payload.as_bytes())
        .await
        .context("Failed to write IPC response")?;
    Ok(())
}

/// Map a command onto the session and describe the state it leads to.
pub fn respond(
    command: DaemonCommand,
    shared: &SharedState,
    control: &mpsc::Sender<SessionCommand>,
) -> DaemonResponse {
    let forward = |session: SessionCommand, next: DaemonState| match control.send(session) {
        Ok(()) => DaemonResponse::ok(next),
        Err(_) => DaemonResponse::error("The daemon session is no longer running"),
    };
    match command {
        DaemonCommand::Status => DaemonResponse::ok(shared.snapshot()),
        DaemonCommand::Toggle => {
            if shared.is_recording() {
                forward(SessionCommand::StopRecording, DaemonState::Transcribing)
            } else {
                forward(
                    SessionCommand::StartRecording,
                    DaemonState::Recording { elapsed_ms: 0 },
                )
            }
        },
        DaemonCommand::Start => forward(
            SessionCommand::StartRecording,
            DaemonState::Recording { elapsed_ms: 0 },
        ),
        DaemonCommand::Stop => forward(SessionCommand::StopRecording, DaemonState::Transcribing),
        DaemonCommand::Cancel => forward(SessionCommand::CancelRecording, DaemonState::Idle),
        DaemonCommand::Quit => forward(SessionCommand::Quit, DaemonState::Stopping),
        DaemonCommand::PasteLast => {
            if shared.last_text().is_none() {
                DaemonResponse::error("Nothing to paste yet: dictate something first")
            } else {
                forward(SessionCommand::PasteLast, DaemonState::Inserting)
            }
        },
        DaemonCommand::Learn { text } => match learn::learn(text.as_deref()) {
            Ok(term) => forward(SessionCommand::ReloadVocabulary, shared.snapshot())
                .with_message(format!("Learned '{}'", term)),
            Err(e) => DaemonResponse::error(e.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::client;

    #[test]
    fn toggle_follows_the_recording_flag() {
        let shared = SharedState::new();
        let (tx, rx) = mpsc::channel();
        assert_eq!(
            respond(DaemonCommand::Toggle, &shared, &tx).state,
            Some(DaemonState::Recording { elapsed_ms: 0 })
        );
        assert_eq!(rx.try_recv(), Ok(SessionCommand::StartRecording));

        shared.set_recording();
        assert_eq!(
            respond(DaemonCommand::Toggle, &shared, &tx).state,
            Some(DaemonState::Transcribing)
        );
        assert_eq!(rx.try_recv(), Ok(SessionCommand::StopRecording));

        assert!(matches!(
            respond(DaemonCommand::Status, &shared, &tx).state,
            Some(DaemonState::Recording { .. })
        ));
    }

    #[test]
    fn paste_last_needs_a_transcript() {
        let shared = SharedState::new();
        let (tx, rx) = mpsc::channel();
        let response = respond(DaemonCommand::PasteLast, &shared, &tx);
        assert!(!response.ok);
        assert!(rx.try_recv().is_err());
        shared.set_last_text("hello".to_string());
        assert_eq!(
            respond(DaemonCommand::PasteLast, &shared, &tx).state,
            Some(DaemonState::Inserting)
        );
        assert_eq!(rx.try_recv(), Ok(SessionCommand::PasteLast));
    }

    #[test]
    fn dead_session_is_reported() {
        let shared = SharedState::new();
        let (tx, rx) = mpsc::channel::<SessionCommand>();
        drop(rx);
        let response = respond(DaemonCommand::Start, &shared, &tx);
        assert!(!response.ok);
        assert!(response.error.unwrap().contains("no longer running"));
    }

    #[tokio::test]
    async fn client_and_server_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hush.sock");
        let shared = Arc::new(SharedState::new());
        let (tx, rx) = mpsc::channel();
        let guard = start(&path, Arc::clone(&shared), tx).await.unwrap();

        let status = client::send_to(&path, DaemonCommand::Status).await.unwrap();
        assert_eq!(status.state, Some(DaemonState::Idle));

        let started = client::send_to(&path, DaemonCommand::Start).await.unwrap();
        assert_eq!(
            started.state,
            Some(DaemonState::Recording { elapsed_ms: 0 })
        );
        assert_eq!(rx.recv().unwrap(), SessionCommand::StartRecording);

        drop(guard);
        assert!(!path.exists());
        assert!(client::send_to(&path, DaemonCommand::Status).await.is_err());
    }
}
