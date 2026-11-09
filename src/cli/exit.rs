//! Exit codes for scripts. `toggle`, `start`, `stop`, `cancel`, `status`, and
//! `paste-last` use them; clap exits with [`ExitStatus::Usage`] on a usage
//! error before a command runs.

use crate::ipc::client::NotRunning;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitStatus {
    Success,
    Error,
    Usage,
    /// No daemon answers on the socket
    NoDaemon,
    /// The take ended without speech
    NoSpeech,
}

impl ExitStatus {
    pub const fn code(self) -> u8 {
        match self {
            ExitStatus::Success => 0,
            ExitStatus::Error => 1,
            ExitStatus::Usage => 2,
            ExitStatus::NoDaemon => 3,
            ExitStatus::NoSpeech => 4,
        }
    }

    pub fn of(result: &anyhow::Result<()>) -> Self {
        match result {
            Ok(()) => ExitStatus::Success,
            Err(error) => Self::of_error(error),
        }
    }

    pub fn of_error(error: &anyhow::Error) -> Self {
        if let Some(exit) = error.downcast_ref::<Exit>() {
            exit.status
        } else if error.downcast_ref::<NotRunning>().is_some() {
            ExitStatus::NoDaemon
        } else {
            ExitStatus::Error
        }
    }
}

/// Ends a command with `status`. Without a message, the command already
/// printed its result, and `main` prints nothing more.
#[derive(Debug)]
pub struct Exit {
    pub status: ExitStatus,
    pub message: Option<String>,
}

impl Exit {
    pub fn new(status: ExitStatus, message: impl Into<String>) -> Self {
        Self {
            status,
            message: Some(message.into()),
        }
    }

    pub fn quiet(status: ExitStatus) -> Self {
        Self {
            status,
            message: None,
        }
    }

    /// `Ok` for success, else the error that carries `status`.
    pub fn quiet_result(status: ExitStatus) -> anyhow::Result<()> {
        match status {
            ExitStatus::Success => Ok(()),
            status => Err(Self::quiet(status).into()),
        }
    }
}

impl fmt::Display for Exit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.message {
            Some(message) => f.write_str(message),
            None => write!(f, "exit code {}", self.status.code()),
        }
    }
}

impl std::error::Error for Exit {}

/// What `main` prints on stderr for a failed command, if anything.
pub fn error_report(error: &anyhow::Error) -> Option<String> {
    match error.downcast_ref::<Exit>() {
        Some(Exit { message: None, .. }) => None,
        _ => Some(format!("Error: {:?}", error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{anyhow, Context};

    #[test]
    fn codes_are_fixed() {
        let codes: Vec<u8> = [
            ExitStatus::Success,
            ExitStatus::Error,
            ExitStatus::Usage,
            ExitStatus::NoDaemon,
            ExitStatus::NoSpeech,
        ]
        .into_iter()
        .map(ExitStatus::code)
        .collect();
        assert_eq!(codes, [0, 1, 2, 3, 4]);
    }

    #[test]
    fn errors_map_to_their_status() {
        assert_eq!(ExitStatus::of(&Ok(())), ExitStatus::Success);
        assert_eq!(ExitStatus::of(&Err(anyhow!("boom"))), ExitStatus::Error);
        assert_eq!(
            ExitStatus::of(&Err(NotRunning.into())),
            ExitStatus::NoDaemon
        );
        let wrapped = Err::<(), _>(NotRunning).context("Failed to stop");
        assert_eq!(ExitStatus::of(&wrapped), ExitStatus::NoDaemon);
        assert_eq!(
            ExitStatus::of(&Err(Exit::new(ExitStatus::NoSpeech, "silence").into())),
            ExitStatus::NoSpeech
        );
        assert_eq!(
            ExitStatus::of(&Exit::quiet_result(ExitStatus::NoDaemon)),
            ExitStatus::NoDaemon
        );
        assert!(Exit::quiet_result(ExitStatus::Success).is_ok());
    }

    #[test]
    fn quiet_exits_print_nothing() {
        assert_eq!(error_report(&Exit::quiet(ExitStatus::Error).into()), None);
        let report = error_report(&Exit::new(ExitStatus::Error, "No recording").into());
        assert!(report.unwrap().starts_with("Error: No recording"));
        let report = error_report(&NotRunning.into());
        assert!(report
            .unwrap()
            .starts_with("Error: Hush daemon is not running"));
    }

    #[tokio::test]
    async fn a_missing_socket_means_no_daemon() {
        let dir = tempfile::tempdir().unwrap();
        let error = crate::ipc::client::send_to(
            &dir.path().join("hush.sock"),
            crate::ipc::DaemonCommand::Status,
        )
        .await
        .unwrap_err();
        assert_eq!(ExitStatus::of_error(&error), ExitStatus::NoDaemon);
    }

    #[test]
    fn clap_usage_errors_use_the_usage_code() {
        use clap::Parser;
        let error = match crate::cli::Cli::try_parse_from(["hush", "stop", "--bogus"]) {
            Ok(_) => panic!("an unknown flag must fail"),
            Err(error) => error,
        };
        assert_eq!(error.exit_code(), i32::from(ExitStatus::Usage.code()));
        assert!(
            crate::cli::Cli::try_parse_from(["hush", "status", "--follow"]).is_err(),
            "--follow needs --json"
        );
    }
}
