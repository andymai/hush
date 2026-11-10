//! Client side of the socket, used by `hush toggle`, `hush stop`, and status.

use super::paths;
use super::protocol::{DaemonCommand, DaemonResponse, DaemonState};
use anyhow::{Context, Result};
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::UnixStream;

/// Nothing listens on the daemon socket.
#[derive(Debug, thiserror::Error)]
#[error("Hush daemon is not running. Start it with: hush daemon start")]
pub struct NotRunning;

/// Send one command to the running daemon.
pub async fn send(command: DaemonCommand) -> Result<DaemonResponse> {
    send_to(&paths::socket_path(), command).await
}

pub async fn send_to(path: &Path, command: DaemonCommand) -> Result<DaemonResponse> {
    let (reader, _writer) = connect(path, &command).await?;
    let mut line = String::new();
    BufReader::new(reader)
        .read_line(&mut line)
        .await
        .context("Failed to read the daemon's reply")?;
    serde_json::from_str(line.trim())
        .with_context(|| format!("Invalid daemon reply: {}", line.trim()))
}

async fn connect(path: &Path, command: &DaemonCommand) -> Result<(OwnedReadHalf, OwnedWriteHalf)> {
    let stream = UnixStream::connect(path).await.map_err(|_| NotRunning)?;
    let (reader, mut writer) = stream.into_split();
    let mut payload = serde_json::to_string(command)?;
    payload.push('\n');
    writer
        .write_all(payload.as_bytes())
        .await
        .context("Failed to send command to the daemon")?;
    Ok((reader, writer))
}

/// The daemon's state at once and after each change.
pub struct Watcher {
    lines: Lines<BufReader<OwnedReadHalf>>,
    _writer: OwnedWriteHalf,
}

impl Watcher {
    pub async fn connect(path: &Path) -> Result<Self> {
        let (reader, writer) = connect(path, &DaemonCommand::Watch).await?;
        Ok(Self {
            lines: BufReader::new(reader).lines(),
            _writer: writer,
        })
    }

    /// `None` when the daemon closes the connection.
    pub async fn next(&mut self) -> Result<Option<DaemonResponse>> {
        let Some(line) = self
            .lines
            .next_line()
            .await
            .context("Failed to read the daemon's state")?
        else {
            return Ok(None);
        };
        serde_json::from_str(line.trim())
            .map(Some)
            .with_context(|| format!("Invalid daemon reply: {}", line.trim()))
    }
}

/// The daemon's state when it answers, `None` when nothing is listening.
pub async fn status() -> Option<DaemonState> {
    status_reply().await?.state
}

/// The daemon's whole status reply, `None` when nothing is listening.
pub async fn status_reply() -> Option<DaemonResponse> {
    send(DaemonCommand::Status).await.ok()
}

/// `status` for code that is not on an async runtime, or is on one it must
/// not block: a plain socket with a short timeout.
pub fn status_blocking(timeout: Duration) -> Option<DaemonState> {
    match probe_at(&paths::socket_path(), timeout) {
        Probe::State(state) => Some(state),
        Probe::NotRunning | Probe::NoAnswer => None,
    }
}

/// What a status question over the socket found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    /// Nothing listens on the socket
    NotRunning,
    State(DaemonState),
    /// Something listens but gave no state in time, as a daemon that is busy
    /// or starts up
    NoAnswer,
}

/// Ask the daemon for its state, waiting at most `timeout`.
pub fn probe_blocking(timeout: Duration) -> Probe {
    probe_at(&paths::socket_path(), timeout)
}

/// [`probe_blocking`] on the socket at `path`.
pub fn probe_at(path: &Path, timeout: Duration) -> Probe {
    reply_at(path, timeout).0
}

/// The daemon's status reply with its probe, such as to read whether it
/// runs in safe mode. The reply is `None` unless the probe has a state.
pub fn reply_blocking(timeout: Duration) -> (Probe, Option<DaemonResponse>) {
    reply_at(&paths::socket_path(), timeout)
}

/// [`reply_blocking`] on the socket at `path`.
pub fn reply_at(path: &Path, timeout: Duration) -> (Probe, Option<DaemonResponse>) {
    use std::io::{BufRead, BufReader, ErrorKind, Write};
    let mut stream = match std::os::unix::net::UnixStream::connect(path) {
        Ok(stream) => stream,
        Err(e) if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::ConnectionRefused) => {
            return (Probe::NotRunning, None)
        },
        Err(_) => return (Probe::NoAnswer, None),
    };
    let mut answer = || -> Option<DaemonResponse> {
        stream.set_read_timeout(Some(timeout)).ok()?;
        stream.set_write_timeout(Some(timeout)).ok()?;
        let mut payload = serde_json::to_string(&DaemonCommand::Status).ok()?;
        payload.push('\n');
        stream.write_all(payload.as_bytes()).ok()?;
        let mut line = String::new();
        BufReader::new(&stream).read_line(&mut line).ok()?;
        serde_json::from_str::<DaemonResponse>(line.trim()).ok()
    };
    match answer() {
        Some(reply) => match reply.state {
            Some(state) => (Probe::State(state), Some(reply)),
            None => (Probe::NoAnswer, None),
        },
        None => (Probe::NoAnswer, None),
    }
}

pub async fn is_running() -> bool {
    status().await.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, Write};

    const TIMEOUT: Duration = Duration::from_millis(100);

    #[test]
    fn no_socket_is_no_daemon() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            probe_at(&dir.path().join("hush.sock"), TIMEOUT),
            Probe::NotRunning
        );
    }

    #[test]
    fn a_socket_that_does_not_answer_is_no_answer_not_a_stopped_daemon() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hush.sock");
        let _listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        assert_eq!(probe_at(&path, TIMEOUT), Probe::NoAnswer);
    }

    #[test]
    fn a_reply_carries_the_safe_mode_of_the_daemon() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hush.sock");
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            std::io::BufReader::new(&stream)
                .read_line(&mut line)
                .unwrap();
            stream
                .write_all(
                    b"{\"ok\":true,\"state\":{\"state\":\"idle\"},\"safe_mode\":\"bad toml\"}\n",
                )
                .unwrap();
        });
        let (probe, reply) = reply_at(&path, Duration::from_secs(2));
        assert_eq!(probe, Probe::State(DaemonState::Idle));
        assert_eq!(reply.unwrap().safe_mode.as_deref(), Some("bad toml"));
        server.join().unwrap();
    }

    #[test]
    fn a_daemon_that_answers_gives_its_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hush.sock");
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            std::io::BufReader::new(&stream)
                .read_line(&mut line)
                .unwrap();
            stream
                .write_all(b"{\"ok\":true,\"state\":{\"state\":\"transcribing\"}}\n")
                .unwrap();
        });
        assert_eq!(
            probe_at(&path, Duration::from_secs(2)),
            Probe::State(DaemonState::Transcribing)
        );
        server.join().unwrap();
    }
}
