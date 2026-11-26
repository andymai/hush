//! `hush status --follow --json`: one Waybar custom-module line at once and
//! after each state change, for as long as the reader keeps the pipe open.

use crate::ipc::protocol::WatchLine;
use crate::ipc::{client, paths, DaemonCommand, DaemonResponse, DaemonState};
use anyhow::{Context, Result};
use serde::Serialize;
use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;

/// How often to ask a daemon that cannot push its state.
const POLL: Duration = Duration::from_millis(200);
/// How often to look for a daemon after it stops.
const RETRY: Duration = Duration::from_secs(2);

/// The JSON a Waybar custom module reads with `"return-type": "json"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WaybarLine {
    pub text: &'static str,
    pub alt: &'static str,
    pub class: &'static str,
    pub tooltip: String,
}

impl WaybarLine {
    /// `None` means no daemon runs. `style` is the style that the take in
    /// progress asked for.
    pub fn new(state: Option<DaemonState>, style: Option<&str>) -> Self {
        let (name, text, tooltip) = match state {
            None => ("stopped", "⊘", "Hush is not running".to_string()),
            Some(state) => {
                let (text, tooltip) = match state {
                    DaemonState::Idle => ("○", "Hush is idle".to_string()),
                    DaemonState::Starting => ("◔", "Hush is starting the microphone".to_string()),
                    DaemonState::Recording {
                        locked, command, ..
                    } => ("●", recording_tooltip(locked, command, style)),
                    DaemonState::Transcribing => ("◑", "Hush is transcribing".to_string()),
                    DaemonState::Inserting => ("◕", "Hush is typing the text".to_string()),
                    DaemonState::Stopping => ("◌", "Hush is stopping".to_string()),
                };
                (state.name(), text, tooltip)
            },
        };
        Self {
            text,
            alt: name,
            class: name,
            tooltip,
        }
    }
}

fn recording_tooltip(locked: bool, command: bool, style: Option<&str>) -> String {
    let mut tooltip = match (command, locked) {
        (true, _) => "Hush is recording a command",
        (false, true) => "Hush is recording hands-free",
        (false, false) => "Hush is recording",
    }
    .to_string();
    if let Some(style) = style {
        tooltip.push_str(&format!(" in the {} style", style));
    }
    tooltip
}

/// Lets a line through only when the state name changes, so the time in a
/// recording does not repeat the line.
#[derive(Debug, Default)]
pub struct ChangeFilter {
    last: Option<WaybarLine>,
}

impl ChangeFilter {
    pub fn update(
        &mut self,
        state: Option<DaemonState>,
        style: Option<&str>,
    ) -> Option<WaybarLine> {
        let line = WaybarLine::new(state, style);
        if self.last.as_ref() == Some(&line) {
            return None;
        }
        self.last = Some(line.clone());
        Some(line)
    }
}

/// Runs until the reader closes the pipe, then exits quietly.
pub async fn handle_status_follow() -> Result<()> {
    match follow(&paths::socket_path(), &mut io::stdout()).await {
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        result => result.context("Failed to write the status"),
    }
}

async fn follow(socket: &Path, out: &mut impl Write) -> io::Result<()> {
    let mut filter = ChangeFilter::default();
    loop {
        if let Ok(mut watcher) = client::Watcher::connect(socket).await {
            while let Ok(Some(line)) = watcher.next().await {
                let WatchLine::State(response) = line else {
                    continue;
                };
                if !response.ok {
                    poll(socket, out, &mut filter).await?;
                    break;
                }
                emit(out, &mut filter, Some(response.as_ref()))?;
            }
        }
        emit(out, &mut filter, None)?;
        tokio::time::sleep(RETRY).await;
    }
}

/// For a daemon that does not know the watch command.
async fn poll(socket: &Path, out: &mut impl Write, filter: &mut ChangeFilter) -> io::Result<()> {
    while let Ok(response) = client::send_to(socket, DaemonCommand::Status).await {
        emit(out, filter, Some(&response))?;
        tokio::time::sleep(POLL).await;
    }
    Ok(())
}

/// `None` means no daemon runs.
fn emit(
    out: &mut impl Write,
    filter: &mut ChangeFilter,
    response: Option<&DaemonResponse>,
) -> io::Result<()> {
    let state = response.and_then(|response| response.state);
    let style = response.and_then(|response| response.style.as_deref());
    if let Some(line) = filter.update(state, style) {
        serde_json::to_writer(&mut *out, &line)?;
        writeln!(out)?;
        out.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::{server, SharedState};
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::Instant;

    /// A pipe whose reader leaves after `limit` lines.
    #[derive(Clone, Default)]
    struct Pipe {
        buf: Arc<Mutex<Vec<u8>>>,
        limit: usize,
    }

    impl Pipe {
        fn lines(&self) -> Vec<String> {
            String::from_utf8(self.buf.lock().unwrap().clone())
                .unwrap()
                .lines()
                .map(str::to_string)
                .collect()
        }

        async fn wait_for(&self, count: usize) {
            while self.lines().len() < count {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }
    }

    impl Write for Pipe {
        fn write(&mut self, data: &[u8]) -> io::Result<usize> {
            let mut buf = self.buf.lock().unwrap();
            if buf.iter().filter(|byte| **byte == b'\n').count() >= self.limit {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            buf.extend_from_slice(data);
            Ok(data.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn follow_streams_changes_until_the_pipe_closes() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("hush.sock");
        let shared = Arc::new(SharedState::new());
        let (tx, _rx) = mpsc::channel();
        let _guard = server::start(&socket, Arc::clone(&shared), tx)
            .await
            .unwrap();
        let pipe = Pipe {
            limit: 2,
            ..Pipe::default()
        };
        let mut out = pipe.clone();
        let changes = async {
            pipe.wait_for(1).await;
            shared.set_recording(Instant::now());
            pipe.wait_for(2).await;
            shared.set_transcribing();
        };
        let (result, ()) = tokio::join!(follow(&socket, &mut out), changes);
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::BrokenPipe);
        let lines = pipe.lines();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains(r#""class":"idle""#));
        assert!(lines[1].contains(r#""class":"recording""#));
    }

    #[test]
    fn each_state_has_a_waybar_line() {
        let cases = [
            (
                None,
                r#"{"text":"⊘","alt":"stopped","class":"stopped","tooltip":"Hush is not running"}"#,
            ),
            (
                Some(DaemonState::Idle),
                r#"{"text":"○","alt":"idle","class":"idle","tooltip":"Hush is idle"}"#,
            ),
            (
                Some(DaemonState::Starting),
                r#"{"text":"◔","alt":"starting","class":"starting","tooltip":"Hush is starting the microphone"}"#,
            ),
            (
                Some(DaemonState::recording(900)),
                r#"{"text":"●","alt":"recording","class":"recording","tooltip":"Hush is recording"}"#,
            ),
            (
                Some(DaemonState::Transcribing),
                r#"{"text":"◑","alt":"transcribing","class":"transcribing","tooltip":"Hush is transcribing"}"#,
            ),
            (
                Some(DaemonState::Inserting),
                r#"{"text":"◕","alt":"inserting","class":"inserting","tooltip":"Hush is typing the text"}"#,
            ),
            (
                Some(DaemonState::Stopping),
                r#"{"text":"◌","alt":"stopping","class":"stopping","tooltip":"Hush is stopping"}"#,
            ),
        ];
        for (state, json) in cases {
            assert_eq!(
                serde_json::to_string(&WaybarLine::new(state, None)).unwrap(),
                json
            );
        }
    }

    #[test]
    fn the_filter_passes_only_changes() {
        let mut filter = ChangeFilter::default();
        let names: Vec<Option<&str>> = [
            Some(DaemonState::Idle),
            Some(DaemonState::Idle),
            Some(DaemonState::recording(10)),
            Some(DaemonState::recording(900)),
            Some(DaemonState::Transcribing),
            None,
            None,
            Some(DaemonState::Idle),
        ]
        .into_iter()
        .map(|state| filter.update(state, None).map(|line| line.class))
        .collect();
        assert_eq!(
            names,
            [
                Some("idle"),
                None,
                Some("recording"),
                None,
                Some("transcribing"),
                Some("stopped"),
                None,
                Some("idle")
            ]
        );
    }

    #[test]
    fn emit_writes_one_json_line_per_change() {
        let mut out = Vec::new();
        let mut filter = ChangeFilter::default();
        emit(&mut out, &mut filter, None).unwrap();
        emit(&mut out, &mut filter, None).unwrap();
        emit(
            &mut out,
            &mut filter,
            Some(&DaemonResponse::ok(DaemonState::Idle)),
        )
        .unwrap();
        let text = String::from_utf8(out).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains(r#""class":"stopped""#));
        assert!(lines[1].contains(r#""class":"idle""#));
    }

    #[test]
    fn the_tooltip_names_the_lock_the_command_and_the_style() {
        let locked = DaemonState::Recording {
            elapsed_ms: 10,
            locked: true,
            command: false,
            stops_in_ms: None,
        };
        assert_eq!(
            WaybarLine::new(Some(locked), Some("casual")).tooltip,
            "Hush is recording hands-free in the casual style"
        );
        let command = DaemonState::Recording {
            elapsed_ms: 10,
            locked: false,
            command: true,
            stops_in_ms: None,
        };
        assert_eq!(
            WaybarLine::new(Some(command), None).tooltip,
            "Hush is recording a command"
        );

        let mut filter = ChangeFilter::default();
        assert!(filter
            .update(Some(DaemonState::recording(10)), None)
            .is_some());
        assert!(
            filter.update(Some(locked), None).is_some(),
            "a lock in the middle of a take changes the line"
        );
        assert!(filter.update(Some(locked), None).is_none());
    }
}
