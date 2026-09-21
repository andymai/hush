//! Client side of the socket, used by `hush toggle`, `hush stop`, and status.

use super::paths;
use super::protocol::{DaemonCommand, DaemonResponse, DaemonState};
use anyhow::{anyhow, Context, Result};
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

/// Send one command to the running daemon.
pub async fn send(command: DaemonCommand) -> Result<DaemonResponse> {
    send_to(&paths::socket_path(), command).await
}

pub async fn send_to(path: &Path, command: DaemonCommand) -> Result<DaemonResponse> {
    let stream = UnixStream::connect(path)
        .await
        .map_err(|_| anyhow!("Hush daemon is not running. Start it with: hush daemon start"))?;
    let (reader, mut writer) = stream.into_split();

    let mut payload = serde_json::to_string(&command)?;
    payload.push('\n');
    writer
        .write_all(payload.as_bytes())
        .await
        .context("Failed to send command to the daemon")?;

    let mut line = String::new();
    BufReader::new(reader)
        .read_line(&mut line)
        .await
        .context("Failed to read the daemon's reply")?;
    serde_json::from_str(line.trim())
        .with_context(|| format!("Invalid daemon reply: {}", line.trim()))
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
