use serde::{Deserialize, Serialize};
use std::fmt;

/// Sent by a client, one JSON object per line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum DaemonCommand {
    /// Start recording if idle, otherwise stop and transcribe
    Toggle,
    Start,
    Stop,
    /// Stop recording and discard the audio
    Cancel,
    Status,
    Quit,
    /// Type the last transcript again
    PasteLast,
    /// Add words to the vocabulary; without `text`, the current selection
    Learn {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
}

/// What the daemon is doing right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum DaemonState {
    Idle,
    Recording { elapsed_ms: u64 },
    Transcribing,
    Inserting,
    Stopping,
}

impl fmt::Display for DaemonState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DaemonState::Idle => f.write_str("idle"),
            DaemonState::Recording { elapsed_ms } => {
                write!(f, "recording ({:.1} s)", *elapsed_ms as f64 / 1000.0)
            },
            DaemonState::Transcribing => f.write_str("transcribing"),
            DaemonState::Inserting => f.write_str("inserting"),
            DaemonState::Stopping => f.write_str("stopping"),
        }
    }
}

/// One reply per command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonResponse {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<DaemonState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// A line for the user, such as the term that was learned
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl DaemonResponse {
    pub fn ok(state: DaemonState) -> Self {
        Self {
            ok: true,
            state: Some(state),
            error: None,
            message: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            state: None,
            error: Some(message.into()),
            message: None,
        }
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_serialize_as_tagged_objects() {
        assert_eq!(
            serde_json::to_string(&DaemonCommand::Toggle).unwrap(),
            r#"{"command":"toggle"}"#
        );
        let parsed: DaemonCommand = serde_json::from_str(r#"{"command":"cancel"}"#).unwrap();
        assert_eq!(parsed, DaemonCommand::Cancel);
        assert_eq!(
            serde_json::to_string(&DaemonCommand::PasteLast).unwrap(),
            r#"{"command":"paste_last"}"#
        );
        assert_eq!(
            serde_json::to_string(&DaemonCommand::Learn { text: None }).unwrap(),
            r#"{"command":"learn"}"#
        );
        let learn: DaemonCommand =
            serde_json::from_str(r#"{"command":"learn","text":"Kubernetes"}"#).unwrap();
        assert_eq!(
            learn,
            DaemonCommand::Learn {
                text: Some("Kubernetes".to_string())
            }
        );
    }

    #[test]
    fn messages_ride_along_with_ok_responses() {
        let json =
            serde_json::to_string(&DaemonResponse::ok(DaemonState::Idle).with_message("Learned"))
                .unwrap();
        assert_eq!(
            json,
            r#"{"ok":true,"state":{"state":"idle"},"message":"Learned"}"#
        );
        let back: DaemonResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.message.as_deref(), Some("Learned"));
    }

    #[test]
    fn states_round_trip_with_payloads() {
        for state in [
            DaemonState::Idle,
            DaemonState::Recording { elapsed_ms: 1500 },
            DaemonState::Transcribing,
            DaemonState::Inserting,
            DaemonState::Stopping,
        ] {
            let json = serde_json::to_string(&DaemonResponse::ok(state)).unwrap();
            let back: DaemonResponse = serde_json::from_str(&json).unwrap();
            assert_eq!(back.state, Some(state));
            assert!(back.ok);
        }
        assert_eq!(
            serde_json::to_string(&DaemonState::Recording { elapsed_ms: 20 }).unwrap(),
            r#"{"state":"recording","elapsed_ms":20}"#
        );
        assert_eq!(
            DaemonState::Recording { elapsed_ms: 1250 }.to_string(),
            "recording (1.2 s)"
        );
    }

    #[test]
    fn error_responses_carry_no_state() {
        let json = serde_json::to_string(&DaemonResponse::error("nope")).unwrap();
        assert_eq!(json, r#"{"ok":false,"error":"nope"}"#);
    }
}
