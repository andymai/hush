//! `hush daemon start|stop|restart|status` and the one-shot clients
//! `hush toggle|start|stop|cancel`.

use super::utils::print_line;
use crate::cli::exit::{Exit, ExitStatus};
use crate::config::Config;
use crate::ipc::protocol::PRINT_TIMEOUT;
use crate::ipc::{
    self, client, lock, paths, DaemonCommand, DaemonResponse, DaemonState, TakeResult,
};
use crate::text_processing::styles::{self, StyleConfig, StyleName};
use crate::transcription::Language;
use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::collections::BTreeMap;
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

/// `hush cancel`, `hush paste-last`, `hush learn`
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

/// `hush start [--lang CODE] [--style NAME]` and `hush toggle [--lang CODE]
/// [--style NAME]`.
pub async fn handle_start(toggle: bool, lang: Option<&str>, style: Option<&str>) -> Result<()> {
    let language = lang.map(Language::from_code).transpose()?;
    let code = language.map(|language| language.code().to_string());
    let style = match style {
        Some(name) => {
            let user = Config::load().ok().map(|config| config.styles);
            Some(style_for_start(name, user.as_ref())?)
        },
        None => None,
    };
    let style_name = style.map(|name| name.to_string());
    let command = if toggle {
        DaemonCommand::Toggle {
            language: code,
            style: style_name,
        }
    } else {
        DaemonCommand::Start {
            language: code,
            style: style_name,
        }
    };
    let response = client::send(command).await?;
    if !response.ok {
        return Err(anyhow!(rejection(response)));
    }
    if ignored_language(language, &response) {
        return Err(anyhow!(OLD_DAEMON_LANGUAGE));
    }
    if ignored_style(style, &response) {
        return Err(anyhow!(OLD_DAEMON_STYLE));
    }
    println!("{}", state_line(response.state));
    Ok(())
}

/// The style that `--style` names. A name that the config does not know is
/// a usage error. Without a readable config, the daemon decides.
fn style_for_start(name: &str, user: Option<&BTreeMap<String, StyleConfig>>) -> Result<StyleName> {
    let parsed =
        StyleName::from_name(name).map_err(|e| Exit::new(ExitStatus::Usage, e.to_string()))?;
    match user {
        Some(user) => styles::known(user, name, "for --style")
            .map(|style| style.name)
            .map_err(|e| Exit::new(ExitStatus::Usage, e.to_string()).into()),
        None => Ok(parsed),
    }
}

/// `hush rewrite STYLE`
pub async fn handle_rewrite(style: &str) -> Result<()> {
    let user = Config::load().ok().map(|config| config.styles);
    let name = style_for_rewrite(style, user.as_ref())?;
    let response = client::send(DaemonCommand::Rewrite {
        style: name.to_string(),
    })
    .await?;
    if !response.ok {
        let error = rejection(response);
        if error.starts_with("Invalid command") {
            return Err(anyhow!(OLD_DAEMON_REWRITE));
        }
        return Err(anyhow!(error));
    }
    println!(
        "{}",
        response
            .message
            .unwrap_or_else(|| state_line(response.state))
    );
    Ok(())
}

/// The style that `hush rewrite` names. A style that the config does not
/// know, or one that cannot rewrite, is a usage error. Without a readable
/// config, the daemon decides.
fn style_for_rewrite(
    name: &str,
    user: Option<&BTreeMap<String, StyleConfig>>,
) -> Result<StyleName> {
    let parsed =
        StyleName::from_name(name).map_err(|e| Exit::new(ExitStatus::Usage, e.to_string()))?;
    match user {
        Some(user) => styles::rewriter(user, name, "for hush rewrite")
            .map(|style| style.name)
            .map_err(|e| Exit::new(ExitStatus::Usage, e.to_string()).into()),
        None => Ok(parsed),
    }
}

/// A daemon from before `--lang` starts the take in its own language, and
/// its reply names no language.
fn ignored_language(asked: Option<Language>, response: &DaemonResponse) -> bool {
    asked.is_some() && response.state == Some(DaemonState::Starting) && response.language.is_none()
}

/// A daemon from before `--style` starts the take without the style, and
/// its reply names no style.
fn ignored_style(asked: Option<StyleName>, response: &DaemonResponse) -> bool {
    asked.is_some() && response.state == Some(DaemonState::Starting) && response.style.is_none()
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
const OLD_DAEMON_LANGUAGE: &str =
    "The running daemon is older than this client and records in its own language. Restart it with: hush daemon restart";
const OLD_DAEMON_REWRITE: &str =
    "The running daemon is older than this client and cannot rewrite. Restart it with: hush daemon restart";
const OLD_DAEMON_STYLE: &str =
    "The running daemon is older than this client and records without the style. Restart it with: hush daemon restart";

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
    fn an_older_daemon_that_ignores_the_language_is_found() {
        let german = Language::parse("de");
        let old = DaemonResponse::ok(DaemonState::Starting);
        assert!(ignored_language(german, &old));
        assert!(!ignored_language(None, &old), "no language was asked for");

        let new = DaemonResponse::ok(DaemonState::Starting).with_language(Some("de".into()));
        assert!(!ignored_language(german, &new));
        let stopped = DaemonResponse::ok(DaemonState::Transcribing);
        assert!(
            !ignored_language(german, &stopped),
            "a toggle that stops a take starts none"
        );
    }

    #[test]
    fn an_unknown_style_is_a_usage_error() {
        let user: BTreeMap<String, StyleConfig> =
            [("casual".to_string(), StyleConfig::default())].into();
        assert_eq!(
            style_for_start("verbatim", Some(&user)).unwrap(),
            styles::VERBATIM
        );
        assert_eq!(
            style_for_start("casual", Some(&user)).unwrap().as_str(),
            "casual"
        );
        let error = style_for_start("nosuch", Some(&user)).unwrap_err();
        assert_eq!(ExitStatus::of_error(&error), ExitStatus::Usage);
        assert!(error.to_string().contains("'nosuch'"), "{error}");
        assert_eq!(
            style_for_start("nosuch", None).unwrap().as_str(),
            "nosuch",
            "without a config, the daemon decides"
        );
        let error = style_for_start("No Such", None).unwrap_err();
        assert_eq!(ExitStatus::of_error(&error), ExitStatus::Usage);
    }

    #[test]
    fn a_rewrite_needs_a_style_that_can_rewrite() {
        let user: BTreeMap<String, StyleConfig> = [
            (
                "formal".to_string(),
                StyleConfig {
                    prompt: "Formal English.".to_string(),
                    polish: true,
                },
            ),
            (
                "raw".to_string(),
                StyleConfig {
                    prompt: "Formal English.".to_string(),
                    polish: false,
                },
            ),
        ]
        .into();
        assert_eq!(
            style_for_rewrite("formal", Some(&user)).unwrap().as_str(),
            "formal"
        );
        for name in ["verbatim", "raw", "nosuch", "No Such"] {
            let error = style_for_rewrite(name, Some(&user)).unwrap_err();
            assert_eq!(ExitStatus::of_error(&error), ExitStatus::Usage, "{name}");
        }
        assert!(style_for_rewrite("verbatim", Some(&user))
            .unwrap_err()
            .to_string()
            .contains("turns polish off"));
        assert_eq!(
            style_for_rewrite("formal", None).unwrap().as_str(),
            "formal",
            "without a config, the daemon decides"
        );
    }

    #[test]
    fn an_older_daemon_that_ignores_the_style_is_found() {
        let verbatim = Some(styles::VERBATIM);
        let old = DaemonResponse::ok(DaemonState::Starting).with_language(Some("en".into()));
        assert!(ignored_style(verbatim, &old));
        assert!(!ignored_style(None, &old), "no style was asked for");
        let new = DaemonResponse::ok(DaemonState::Starting).with_style(Some("verbatim".into()));
        assert!(!ignored_style(verbatim, &new));
        let stopped = DaemonResponse::ok(DaemonState::Transcribing);
        assert!(
            !ignored_style(verbatim, &stopped),
            "a toggle that stops a take starts none"
        );
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
