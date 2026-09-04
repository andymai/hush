//! `hush daemon start|stop|restart|status` and the one-shot clients
//! `hush toggle|start|stop|cancel`.

use crate::ipc::{self, client, lock, paths, DaemonCommand, DaemonState};
use anyhow::{anyhow, Context, Result};
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

/// `hush toggle`, `hush start`, `hush stop`, `hush cancel`
pub async fn handle_client_command(command: DaemonCommand) -> Result<()> {
    let response = client::send(command).await?;
    if let (true, Some(message)) = (response.ok, &response.message) {
        println!("{}", message);
        return Ok(());
    }
    match (response.ok, response.state) {
        (true, Some(DaemonState::Recording { .. })) => println!("Recording."),
        (true, Some(DaemonState::Transcribing)) => println!("Transcribing."),
        (true, Some(DaemonState::Idle)) => println!("Idle."),
        (true, Some(state)) => println!("{}.", state),
        (true, None) => println!("Done."),
        (false, _) => {
            return Err(anyhow!(response
                .error
                .unwrap_or_else(|| "The daemon rejected the command".to_string())))
        },
    }
    Ok(())
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
