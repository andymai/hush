//! Daemon side of the socket. Each connection carries one command and gets
//! one reply. Commands that change state are forwarded to the session loop;
//! the reply names the state that request leads to. A print stop replies
//! when the take ends, and a watch replies once for each state change.

use super::protocol::{DaemonCommand, DaemonResponse, DaemonState, TakeResult, PRINT_TIMEOUT};
use crate::text_processing::{learn, Trigger};
use anyhow::{Context, Result};
use parking_lot::Mutex;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;
use tracing::{debug, warn};

/// What the session loop consumes, from the hotkey thread and from IPC alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionCommand {
    StartRecording(Trigger),
    /// Record a spoken instruction for Command Mode
    StartCommand,
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
    Starting,
    Recording(Instant),
    Transcribing,
    Inserting,
    Stopping,
}

/// The client of a print stop. The take it ends types nothing and sends its
/// text here instead.
pub struct TakeWaiter(oneshot::Sender<TakeResult>);

impl TakeWaiter {
    pub fn finish(self, result: TakeResult) {
        let _ = self.0.send(result);
    }
}

/// Current phase and the last transcript, written by the session and read
/// by IPC.
pub struct SharedState {
    phase: Mutex<Phase>,
    last_text: Mutex<Option<String>>,
    waiter: Mutex<Option<oneshot::Sender<TakeResult>>>,
    changes: watch::Sender<()>,
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
            waiter: Mutex::new(None),
            changes: watch::channel(()).0,
        }
    }

    fn set(&self, phase: Phase) {
        *self.phase.lock() = phase;
        self.changes.send_replace(());
    }

    /// Fires after each phase change.
    pub fn subscribe(&self) -> watch::Receiver<()> {
        self.changes.subscribe()
    }

    /// Make the next stop deliver its take here. A second print stop
    /// replaces the first, whose client then gets an error.
    pub fn wait_for_take(&self) -> oneshot::Receiver<TakeResult> {
        let (tx, rx) = oneshot::channel();
        *self.waiter.lock() = Some(tx);
        rx
    }

    /// Called by the session when it handles a stop.
    pub fn take_waiter(&self) -> Option<TakeWaiter> {
        self.waiter
            .lock()
            .take()
            .filter(|tx| !tx.is_closed())
            .map(TakeWaiter)
    }

    pub fn set_last_text(&self, text: String) {
        *self.last_text.lock() = Some(text);
    }

    pub fn last_text(&self) -> Option<String> {
        self.last_text.lock().clone()
    }

    pub fn set_idle(&self) {
        self.set(Phase::Idle);
    }

    /// The stream started and waits for its first audio frame.
    pub fn set_starting(&self) {
        self.set(Phase::Starting);
    }

    /// `since` is the time of the first audio frame.
    pub fn set_recording(&self, since: Instant) {
        self.set(Phase::Recording(since));
    }

    pub fn set_transcribing(&self) {
        self.set(Phase::Transcribing);
    }

    pub fn set_inserting(&self) {
        self.set(Phase::Inserting);
    }

    pub fn set_stopping(&self) {
        self.set(Phase::Stopping);
    }

    /// A take is in progress, whether or not its first frame arrived.
    pub fn in_take(&self) -> bool {
        matches!(*self.phase.lock(), Phase::Starting | Phase::Recording(_))
    }

    pub fn snapshot(&self) -> DaemonState {
        match *self.phase.lock() {
            Phase::Idle => DaemonState::Idle,
            Phase::Starting => DaemonState::Starting,
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
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .await
        .context("Failed to read IPC command")?;
    let response = match serde_json::from_str::<DaemonCommand>(line.trim()) {
        Ok(DaemonCommand::Watch) => return watch(reader, writer, shared).await,
        Ok(DaemonCommand::Stop { print: true }) => {
            stop_and_print(shared, control, PRINT_TIMEOUT).await
        },
        Ok(command) => respond(command, shared, control),
        Err(e) => DaemonResponse::error(format!("Invalid command: {}", e)),
    };
    write_response(&mut writer, &response).await
}

async fn write_response(writer: &mut OwnedWriteHalf, response: &DaemonResponse) -> Result<()> {
    let mut payload = serde_json::to_string(response)?;
    payload.push('\n');
    writer
        .write_all(payload.as_bytes())
        .await
        .context("Failed to write IPC response")
}

/// Send the state now and after each change, until the client hangs up.
async fn watch(
    mut reader: BufReader<OwnedReadHalf>,
    mut writer: OwnedWriteHalf,
    shared: &SharedState,
) -> Result<()> {
    let mut changes = shared.subscribe();
    let mut scratch = [0u8; 64];
    loop {
        write_response(&mut writer, &DaemonResponse::ok(shared.snapshot())).await?;
        tokio::select! {
            changed = changes.changed() => {
                if changed.is_err() {
                    return Ok(());
                }
            },
            read = reader.read(&mut scratch) => {
                if matches!(read, Ok(0) | Err(_)) {
                    return Ok(());
                }
            },
        }
    }
}

/// Stop the take and reply with its text when the session finishes it. The
/// session decides whether a take records, because a start sent just before
/// can still wait in the command channel.
pub async fn stop_and_print(
    shared: &SharedState,
    control: &mpsc::Sender<SessionCommand>,
    timeout: Duration,
) -> DaemonResponse {
    let result = shared.wait_for_take();
    if control.send(SessionCommand::StopRecording).is_err() {
        return DaemonResponse::error("The daemon session is no longer running");
    }
    match tokio::time::timeout(timeout, result).await {
        Ok(Ok(take)) => DaemonResponse::ok(shared.snapshot()).with_take(take),
        Ok(Err(_)) => DaemonResponse::error("The take ended without a result"),
        Err(_) => DaemonResponse::error(format!(
            "Hush did not finish the take within {} s",
            timeout.as_secs()
        )),
    }
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
    // A stop before the first audio frame discards the take.
    let after_stop = match shared.snapshot() {
        DaemonState::Starting => DaemonState::Idle,
        DaemonState::Recording { .. } => DaemonState::Transcribing,
        other => other,
    };
    match command {
        DaemonCommand::Status | DaemonCommand::Watch => DaemonResponse::ok(shared.snapshot()),
        DaemonCommand::Toggle => {
            if shared.in_take() {
                forward(SessionCommand::StopRecording, after_stop)
            } else {
                forward(
                    SessionCommand::StartRecording(Trigger::Ipc),
                    DaemonState::Starting,
                )
            }
        },
        DaemonCommand::Start => forward(
            SessionCommand::StartRecording(Trigger::Ipc),
            DaemonState::Starting,
        ),
        DaemonCommand::Stop { .. } => forward(SessionCommand::StopRecording, after_stop),
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
    fn toggle_follows_the_take() {
        let shared = SharedState::new();
        let (tx, rx) = mpsc::channel();
        assert_eq!(
            respond(DaemonCommand::Toggle, &shared, &tx).state,
            Some(DaemonState::Starting)
        );
        assert_eq!(
            rx.try_recv(),
            Ok(SessionCommand::StartRecording(Trigger::Ipc))
        );

        shared.set_starting();
        assert_eq!(
            respond(DaemonCommand::Toggle, &shared, &tx).state,
            Some(DaemonState::Idle),
            "a stop before the first frame discards the take"
        );
        assert_eq!(rx.try_recv(), Ok(SessionCommand::StopRecording));

        shared.set_recording(Instant::now());
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

    #[test]
    fn a_plain_stop_without_a_take_keeps_the_state() {
        let shared = SharedState::new();
        let (tx, _rx) = mpsc::channel();
        assert_eq!(
            respond(DaemonCommand::Stop { print: false }, &shared, &tx).state,
            Some(DaemonState::Idle)
        );
    }

    #[tokio::test]
    async fn print_stop_gives_up_after_the_timeout() {
        let shared = SharedState::new();
        let (tx, rx) = mpsc::channel();
        let response = stop_and_print(&shared, &tx, Duration::from_millis(20)).await;
        assert_eq!(rx.try_recv(), Ok(SessionCommand::StopRecording));
        assert!(!response.ok);
        assert!(response.error.unwrap().contains("did not finish"));
        assert!(
            shared.take_waiter().is_none(),
            "the next stop types its take again"
        );
    }

    #[tokio::test]
    async fn print_stop_reports_a_dead_session() {
        let shared = SharedState::new();
        let (tx, rx) = mpsc::channel();
        drop(rx);
        let response = stop_and_print(&shared, &tx, Duration::from_secs(1)).await;
        assert!(!response.ok);
        assert!(response.error.unwrap().contains("no longer running"));
    }

    #[test]
    fn a_waiter_whose_client_left_is_dropped() {
        let shared = SharedState::new();
        drop(shared.wait_for_take());
        assert!(shared.take_waiter().is_none());
        let _rx = shared.wait_for_take();
        assert!(shared.take_waiter().is_some());
        assert!(shared.take_waiter().is_none(), "a waiter serves one take");
    }

    #[tokio::test]
    async fn print_stop_replies_with_the_take_over_the_socket() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hush.sock");
        let shared = Arc::new(SharedState::new());
        let (tx, rx) = mpsc::channel();
        let _guard = start(&path, Arc::clone(&shared), tx).await.unwrap();
        shared.set_recording(Instant::now());

        let session_shared = Arc::clone(&shared);
        let session = std::thread::spawn(move || {
            assert_eq!(rx.recv().unwrap(), SessionCommand::StopRecording);
            let waiter = session_shared.take_waiter().expect("a print stop waits");
            session_shared.set_idle();
            waiter.finish(TakeResult::Ok {
                text: "Ship it.".to_string(),
            });
        });

        let response = client::send_to(&path, DaemonCommand::Stop { print: true })
            .await
            .unwrap();
        session.join().unwrap();
        assert!(response.ok);
        assert_eq!(
            response.take,
            Some(TakeResult::Ok {
                text: "Ship it.".to_string()
            })
        );
    }

    #[tokio::test]
    async fn watch_sends_the_state_now_and_on_each_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hush.sock");
        let shared = Arc::new(SharedState::new());
        let (tx, _rx) = mpsc::channel();
        let guard = start(&path, Arc::clone(&shared), tx).await.unwrap();

        let mut watcher = client::Watcher::connect(&path).await.unwrap();
        let first = watcher.next().await.unwrap().unwrap();
        assert_eq!(first.state, Some(DaemonState::Idle));

        shared.set_starting();
        let next = watcher.next().await.unwrap().unwrap();
        assert_eq!(next.state, Some(DaemonState::Starting));
        drop(guard);
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
        assert_eq!(started.state, Some(DaemonState::Starting));
        assert_eq!(
            rx.recv().unwrap(),
            SessionCommand::StartRecording(Trigger::Ipc)
        );

        drop(guard);
        assert!(!path.exists());
        assert!(client::send_to(&path, DaemonCommand::Status).await.is_err());
    }
}
