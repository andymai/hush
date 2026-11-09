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
    send(DaemonCommand::Status).await.ok()?.state
}

/// `status` for code that is not on an async runtime, or is on one it must
/// not block: a plain socket with a short timeout.
pub fn status_blocking(timeout: Duration) -> Option<DaemonState> {
    use std::io::{BufRead, BufReader, Write};
    let mut stream = std::os::unix::net::UnixStream::connect(paths::socket_path()).ok()?;
    stream.set_read_timeout(Some(timeout)).ok()?;
    stream.set_write_timeout(Some(timeout)).ok()?;
    let mut payload = serde_json::to_string(&DaemonCommand::Status).ok()?;
    payload.push('\n');
    stream.write_all(payload.as_bytes()).ok()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).ok()?;
    serde_json::from_str::<DaemonResponse>(line.trim())
        .ok()?
        .state
}

pub async fn is_running() -> bool {
    status().await.is_some()
}
