//! `hush daemon start|stop|restart|status` and the one-shot clients
//! `hush toggle|start|stop|cancel`.

use super::utils::print_line;
use crate::cli::exit::{Exit, ExitStatus};
use crate::ipc::protocol::PRINT_TIMEOUT;
use crate::ipc::{
    self, client, lock, paths, DaemonCommand, DaemonResponse, DaemonState, TakeResult,
};
use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::Duration;

/// What `hush listen` and `hush daemon start` accept.
#[derive(Debug, Clone)]
pub struct SessionOptions {
    pub editing_mode: String,
    pub no_processing: bool,
    pub no_button: bool,
}

pub async fn handle_daemon_start(options: SessionOptions, foreground: bool) -> Result<()> {
    if let Some(state) = client::status().await {
        println!("Hush daemon is already running ({}).", state);
        return Ok(());
    }
    crate::config::paths::move_legacy_models();
    if foreground {
        super::listen::handle_listen(options).await
    } else {
        spawn_detached(&options).await
    }
}

/// Re-run this binary in the foreground, detached from the terminal, and wait
/// for its socket to appear.
async fn spawn_detached(options: &SessionOptions) -> Result<()> {
    let exe = std::env::current_exe().context("Failed to locate the hush binary")?;
    let mut command = Command::new(exe);
    command.args(["daemon", "start", "--foreground"]);
    command.args(["--editing-mode", &options.editing_mode]);
    if options.no_processing {
        command.arg("--no-processing");
    }
    if options.no_button {
        command.arg("--no-button");
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);

    let mut child = command
        .spawn()
        .context("Failed to start the daemon process")?;
    let pid = child.id();

    // Model loading can take a few seconds on the first start.
    for _ in 0..80 {
        if let Some(state) = client::status().await {
            println!("Hush daemon started (PID {}), {}.", pid, state);
            println!("Use `hush toggle` or the hotkey to dictate, `hush daemon stop` to quit.");
            return Ok(());
        }
        if let Some(status) = child.try_wait()? {
            return Err(anyhow!(
                "The daemon exited during startup ({}). Run `hush daemon start --foreground` to see why.",
                status
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Err(anyhow!(
        "The daemon (PID {}) did not answer within 20 s. Run `hush daemon start --foreground` to see why.",
        pid
    ))
}

pub async fn handle_daemon_stop() -> Result<()> {
    match client::send(DaemonCommand::Quit).await {
        Ok(_) => {
            for _ in 0..40 {
                if !client::is_running().await {
                    println!("Hush daemon stopped.");
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            terminate_by_pid()
        },
        Err(_) => match lock::read_pid(&paths::pid_path()) {
            Some(pid) if lock::process_alive(pid) => terminate_by_pid(),
            _ => {
                println!("Hush daemon is not running.");
                Ok(())
            },
        },
    }
}

fn terminate_by_pid() -> Result<()> {
    let pid = lock::read_pid(&paths::pid_path())
        .ok_or_else(|| anyhow!("The daemon did not stop and left no PID file"))?;
    // SAFETY: sending SIGTERM to a PID we recorded ourselves.
    if unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) } == 0 {
        println!("Sent SIGTERM to the daemon (PID {}).", pid);
        Ok(())
    } else {
        Err(anyhow!("Failed to signal the daemon (PID {})", pid))
    }
}

pub async fn handle_daemon_restart(options: SessionOptions) -> Result<()> {
    if client::is_running().await {
        handle_daemon_stop().await?;
    }
    spawn_detached(&options).await
}

pub async fn handle_daemon_status() -> Result<()> {
    match client::status().await {
        Some(state) => println!("Hush daemon is running: {}", state),
        None => println!("Hush daemon is not running. Start it with: hush daemon start"),
    }
    Ok(())
}

/// `hush toggle`, `hush start`, `hush cancel`, `hush paste-last`, `hush learn`
pub async fn handle_client_command(command: DaemonCommand) -> Result<()> {
    let response = client::send(command).await?;
    if !response.ok {
        return Err(anyhow!(rejection(response)));
    }
    match response.message {
        Some(message) => println!("{}", message),
        None => println!("{}", state_line(response.state)),
    }
    Ok(())
}

fn rejection(response: DaemonResponse) -> String {
    response
        .error
        .unwrap_or_else(|| "The daemon rejected the command".to_string())
}

fn state_line(state: Option<DaemonState>) -> String {
    match state {
        Some(DaemonState::Starting) => "Starting the microphone.".to_string(),
        Some(DaemonState::Recording { .. }) => "Recording.".to_string(),
        Some(DaemonState::Transcribing) => "Transcribing.".to_string(),
        Some(DaemonState::Idle) => "Idle.".to_string(),
        Some(state) => format!("{}.", state),
        None => "Done.".to_string(),
    }
}

const NO_SPEECH: &str = "Hush heard no speech.";
const OLD_DAEMON: &str =
    "The running daemon is older than this client and typed the text. Restart it with: hush daemon restart";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StopStatus {
    Ok,
    NoSpeech,
    Error,
}

/// What `hush stop --json` prints. Every key is present; absent values are null.
#[derive(Debug, PartialEq, Serialize)]
pub struct StopReport {
    pub status: StopStatus,
    pub text: Option<String>,
    pub chars: Option<usize>,
    pub message: Option<String>,
    #[serde(skip)]
    pub exit: ExitStatus,
}

impl StopReport {
    fn ok(text: Option<String>, message: Option<String>) -> Self {
        Self {
            status: StopStatus::Ok,
            chars: text.as_ref().map(|text| text.chars().count()),
            text,
            message,
            exit: ExitStatus::Success,
        }
    }

    fn no_speech() -> Self {
        Self {
            status: StopStatus::NoSpeech,
            text: None,
            chars: None,
            message: Some(NO_SPEECH.to_string()),
            exit: ExitStatus::NoSpeech,
        }
    }

    fn error(exit: ExitStatus, message: impl Into<String>) -> Self {
        Self {
            status: StopStatus::Error,
            text: None,
            chars: None,
            message: Some(message.into()),
            exit,
        }
    }

    pub fn from_reply(reply: Result<DaemonResponse>, print: bool) -> Self {
        let response = match reply {
            Ok(response) => response,
            Err(e) => return Self::error(ExitStatus::of_error(&e), format!("{:#}", e)),
        };
        if !response.ok {
            return Self::error(ExitStatus::Error, rejection(response));
        }
        if !print {
            return Self::ok(None, Some(state_line(response.state)));
        }
        match response.take {
            Some(TakeResult::Ok { text }) => Self::ok(Some(text), None),
            Some(TakeResult::NoSpeech) => Self::no_speech(),
            Some(TakeResult::Error { message }) => Self::error(ExitStatus::Error, message),
            None => Self::error(ExitStatus::Error, OLD_DAEMON),
        }
    }
}

/// `hush stop [--print] [--json]`
pub async fn handle_stop(print: bool, json: bool) -> Result<()> {
    let command = DaemonCommand::Stop { print };
    let reply = if print {
        tokio::time::timeout(PRINT_TIMEOUT, client::send(command))
            .await
            .unwrap_or_else(|_| {
                Err(anyhow!(
                    "Hush did not finish the take within {} s",
                    PRINT_TIMEOUT.as_secs()
                ))
            })
    } else {
        client::send(command).await
    };
    let report = StopReport::from_reply(reply, print);
    if json {
        print_line(&serde_json::to_string(&report)?)?;
        return Exit::quiet_result(report.exit);
    }
    match (report.exit, report.text, report.message) {
        (ExitStatus::Success, Some(text), _) => print_line(&text),
        (ExitStatus::Success, None, message) => print_line(&message.unwrap_or_default()),
        (exit, _, message) => Err(Exit::new(exit, message.unwrap_or_default()).into()),
    }
}

/// Shared by the session: runtime directory, PID lock, and IPC server.
pub struct DaemonHandles {
    pub shared: std::sync::Arc<ipc::SharedState>,
    _lock: lock::PidLock,
    _server: ipc::server::ServerGuard,
}

/// Claim the single-daemon slot and open the socket.
pub async fn claim(control: std::sync::mpsc::Sender<ipc::SessionCommand>) -> Result<DaemonHandles> {
    paths::ensure_runtime_dir()?;
    let lock = lock::PidLock::acquire(&paths::pid_path())?;
    let shared = std::sync::Arc::new(ipc::SharedState::new());
    let server = ipc::server::start(
        &paths::socket_path(),
        std::sync::Arc::clone(&shared),
        control,
    )
    .await?;
    Ok(DaemonHandles {
        shared,
        _lock: lock,
        _server: server,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::client::NotRunning;
    use crate::ipc::protocol::NO_TAKE;

    fn printed(take: TakeResult) -> StopReport {
        StopReport::from_reply(
            Ok(DaemonResponse::ok(DaemonState::Idle).with_take(take)),
            true,
        )
    }

    #[test]
    fn a_printed_take_reports_its_text_and_length() {
        let report = printed(TakeResult::Ok {
            text: "Héllo, world.".to_string(),
        });
        assert_eq!(report.exit, ExitStatus::Success);
        assert_eq!(
            serde_json::to_string(&report).unwrap(),
            r#"{"status":"ok","text":"Héllo, world.","chars":13,"message":null}"#
        );
    }

    #[test]
    fn silence_and_failures_have_their_own_status() {
        let report = printed(TakeResult::NoSpeech);
        assert_eq!(report.exit, ExitStatus::NoSpeech);
        assert_eq!(
            serde_json::to_string(&report).unwrap(),
            r#"{"status":"no_speech","text":null,"chars":null,"message":"Hush heard no speech."}"#
        );

        let report = printed(TakeResult::Error {
            message: "Transcription failed".to_string(),
        });
        assert_eq!(report.exit, ExitStatus::Error);
        assert_eq!(
            serde_json::to_string(&report).unwrap(),
            r#"{"status":"error","text":null,"chars":null,"message":"Transcription failed"}"#
        );
    }

    #[test]
    fn a_print_stop_without_a_take_is_an_error() {
        let report = printed(TakeResult::Error {
            message: NO_TAKE.to_string(),
        });
        assert_eq!(report.status, StopStatus::Error);
        assert_eq!(report.exit, ExitStatus::Error);
        assert_eq!(report.message.as_deref(), Some(NO_TAKE));

        let rejected = StopReport::from_reply(Ok(DaemonResponse::error("busy")), true);
        assert_eq!(rejected.exit, ExitStatus::Error);
        assert_eq!(rejected.message.as_deref(), Some("busy"));
    }

    #[test]
    fn an_older_daemon_reply_is_an_error() {
        let report =
            StopReport::from_reply(Ok(DaemonResponse::ok(DaemonState::Transcribing)), true);
        assert_eq!(report.exit, ExitStatus::Error);
        assert_eq!(report.message.as_deref(), Some(OLD_DAEMON));
    }

    #[test]
    fn no_daemon_maps_to_its_exit_code() {
        let report = StopReport::from_reply(Err(NotRunning.into()), true);
        assert_eq!(report.status, StopStatus::Error);
        assert_eq!(report.exit, ExitStatus::NoDaemon);
    }

    #[test]
    fn a_plain_stop_reports_the_next_state() {
        let report =
            StopReport::from_reply(Ok(DaemonResponse::ok(DaemonState::Transcribing)), false);
        assert_eq!(report.exit, ExitStatus::Success);
        assert_eq!(
            serde_json::to_string(&report).unwrap(),
            r#"{"status":"ok","text":null,"chars":null,"message":"Transcribing."}"#
        );
    }
}
