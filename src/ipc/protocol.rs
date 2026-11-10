use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

/// How long `hush stop --print` waits for the final text.
pub const PRINT_TIMEOUT: Duration = Duration::from_secs(60);

/// The reply to a print stop while no take records.
pub const NO_TAKE: &str = "No recording is in progress. Start one with: hush start";

/// Sent by a client, one JSON object per line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum DaemonCommand {
    /// Start recording if idle, otherwise stop and transcribe. `language`
    /// applies to the take it starts. A daemon from before the field ignores
    /// it, and its reply then carries no `language`.
    Toggle {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        language: Option<String>,
    },
    /// Start recording, in `language` when set
    Start {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        language: Option<String>,
    },
    /// Stop recording and transcribe; with `print`, reply with the final text
    /// and type nothing
    Stop {
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        print: bool,
    },
    /// Stop recording and discard the audio
    Cancel,
    Status,
    /// Keep the connection open and send the state at once and on each change
    Watch,
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
    /// A take waits for the first audio from the microphone
    Starting,
    Recording {
        elapsed_ms: u64,
    },
    Transcribing,
    Inserting,
    Stopping,
}

impl DaemonState {
    /// The lowercase state name that scripts read.
    pub fn name(&self) -> &'static str {
        match self {
            DaemonState::Idle => "idle",
            DaemonState::Starting => "starting",
            DaemonState::Recording { .. } => "recording",
            DaemonState::Transcribing => "transcribing",
            DaemonState::Inserting => "inserting",
            DaemonState::Stopping => "stopping",
        }
    }
}

impl fmt::Display for DaemonState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DaemonState::Idle => f.write_str("idle"),
            DaemonState::Starting => f.write_str("starting"),
            DaemonState::Recording { elapsed_ms } => {
                write!(f, "recording ({:.1} s)", *elapsed_ms as f64 / 1000.0)
            },
            DaemonState::Transcribing => f.write_str("transcribing"),
            DaemonState::Inserting => f.write_str("inserting"),
            DaemonState::Stopping => f.write_str("stopping"),
        }
    }
}

/// Whether the daemon holds the local model in memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelState {
    Loaded,
    /// The next take started a load, and the take waits for it
    Loading,
    /// Freed after idle minutes; the next take loads it again
    Unloaded,
    /// An HTTP backend runs the model, so the daemon holds none
    #[serde(rename = "none")]
    Remote,
}

impl ModelState {
    /// The lowercase name that scripts read.
    pub fn name(&self) -> &'static str {
        match self {
            ModelState::Loaded => "loaded",
            ModelState::Loading => "loading",
            ModelState::Unloaded => "unloaded",
            ModelState::Remote => "none",
        }
    }
}

impl fmt::Display for ModelState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelState::Loaded => f.write_str("loaded"),
            ModelState::Loading => f.write_str("loading"),
            ModelState::Unloaded => f.write_str("unloaded (the next take loads it)"),
            ModelState::Remote => f.write_str("none (the transcription service runs it)"),
        }
    }
}

/// How a take ended, for a client that asked for its text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TakeResult {
    Ok { text: String },
    NoSpeech,
    Error { message: String },
}

impl TakeResult {
    /// `NoSpeech` when processing left only whitespace.
    pub fn from_text(text: String) -> Self {
        if text.trim().is_empty() {
            TakeResult::NoSpeech
        } else {
            TakeResult::Ok { text }
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
    /// The result of the take that a print stop ended
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub take: Option<TakeResult>,
    /// Where the model is, in a status or watch reply
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_state: Option<ModelState>,
    /// The language of the take a start began, or in a status or watch
    /// reply, of the take in progress or the next one
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

impl DaemonResponse {
    pub fn ok(state: DaemonState) -> Self {
        Self {
            ok: true,
            state: Some(state),
            error: None,
            message: None,
            take: None,
            model_state: None,
            language: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            state: None,
            error: Some(message.into()),
            message: None,
            take: None,
            model_state: None,
            language: None,
        }
    }

    pub fn with_language(mut self, language: Option<String>) -> Self {
        self.language = language;
        self
    }

    pub fn with_model_state(mut self, model_state: Option<ModelState>) -> Self {
        self.model_state = model_state;
        self
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn with_take(mut self, take: TakeResult) -> Self {
        self.take = Some(take);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_serialize_as_tagged_objects() {
        assert_eq!(
            serde_json::to_string(&DaemonCommand::Toggle { language: None }).unwrap(),
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
    fn stop_keeps_the_old_form_and_adds_print() {
        assert_eq!(
            serde_json::to_string(&DaemonCommand::Stop { print: false }).unwrap(),
            r#"{"command":"stop"}"#
        );
        assert_eq!(
            serde_json::to_string(&DaemonCommand::Stop { print: true }).unwrap(),
            r#"{"command":"stop","print":true}"#
        );
        let old: DaemonCommand = serde_json::from_str(r#"{"command":"stop"}"#).unwrap();
        assert_eq!(old, DaemonCommand::Stop { print: false });
        let new: DaemonCommand =
            serde_json::from_str(r#"{"command":"stop","print":true}"#).unwrap();
        assert_eq!(new, DaemonCommand::Stop { print: true });
        assert_eq!(
            serde_json::to_string(&DaemonCommand::Watch).unwrap(),
            r#"{"command":"watch"}"#
        );
    }

    #[test]
    fn start_and_toggle_keep_the_old_form_and_add_a_language() {
        for (command, old, new) in [
            (
                DaemonCommand::Start { language: None },
                r#"{"command":"start"}"#,
                r#"{"command":"start","language":"de"}"#,
            ),
            (
                DaemonCommand::Toggle { language: None },
                r#"{"command":"toggle"}"#,
                r#"{"command":"toggle","language":"de"}"#,
            ),
        ] {
            assert_eq!(serde_json::to_string(&command).unwrap(), old);
            assert_eq!(
                serde_json::from_str::<DaemonCommand>(old).unwrap(),
                command,
                "an older client sends no language"
            );
            let with_language = match command {
                DaemonCommand::Start { .. } => DaemonCommand::Start {
                    language: Some("de".to_string()),
                },
                _ => DaemonCommand::Toggle {
                    language: Some("de".to_string()),
                },
            };
            assert_eq!(serde_json::to_string(&with_language).unwrap(), new);
            assert_eq!(
                serde_json::from_str::<DaemonCommand>(new).unwrap(),
                with_language
            );
        }
    }

    #[test]
    fn an_older_daemon_reads_a_start_in_a_language_as_a_plain_start() {
        #[derive(Debug, PartialEq, Deserialize)]
        #[serde(tag = "command", rename_all = "snake_case")]
        enum OldCommand {
            Toggle,
            Start,
        }
        let start: OldCommand =
            serde_json::from_str(r#"{"command":"start","language":"de"}"#).unwrap();
        assert_eq!(start, OldCommand::Start);
        let toggle: OldCommand =
            serde_json::from_str(r#"{"command":"toggle","language":"de"}"#).unwrap();
        assert_eq!(toggle, OldCommand::Toggle);

        let old: DaemonResponse =
            serde_json::from_str(r#"{"ok":true,"state":{"state":"starting"}}"#).unwrap();
        assert_eq!(old.language, None, "the reply of an older daemon");
        let reply = DaemonResponse::ok(DaemonState::Starting).with_language(Some("de".into()));
        let json = serde_json::to_string(&reply).unwrap();
        assert_eq!(
            json,
            r#"{"ok":true,"state":{"state":"starting"},"language":"de"}"#
        );
        assert_eq!(
            serde_json::from_str::<DaemonResponse>(&json).unwrap(),
            reply
        );
    }

    #[test]
    fn an_older_daemon_reads_a_print_stop_as_a_plain_stop() {
        #[derive(Debug, PartialEq, Deserialize)]
        #[serde(tag = "command", rename_all = "snake_case")]
        enum OldCommand {
            Stop,
        }
        let parsed: OldCommand =
            serde_json::from_str(r#"{"command":"stop","print":true}"#).unwrap();
        assert_eq!(parsed, OldCommand::Stop);
    }

    #[test]
    fn take_results_carry_a_status_tag() {
        let cases = [
            (
                TakeResult::Ok {
                    text: "Hello there.".to_string(),
                },
                r#"{"status":"ok","text":"Hello there."}"#,
            ),
            (TakeResult::NoSpeech, r#"{"status":"no_speech"}"#),
            (
                TakeResult::Error {
                    message: "Transcription failed".to_string(),
                },
                r#"{"status":"error","message":"Transcription failed"}"#,
            ),
        ];
        for (take, json) in cases {
            assert_eq!(serde_json::to_string(&take).unwrap(), json);
            assert_eq!(serde_json::from_str::<TakeResult>(json).unwrap(), take);
        }
        let reply = DaemonResponse::ok(DaemonState::Idle).with_take(TakeResult::NoSpeech);
        assert_eq!(
            serde_json::to_string(&reply).unwrap(),
            r#"{"ok":true,"state":{"state":"idle"},"take":{"status":"no_speech"}}"#
        );
        let old: DaemonResponse =
            serde_json::from_str(r#"{"ok":true,"state":{"state":"transcribing"}}"#).unwrap();
        assert_eq!(old.take, None);
    }

    #[test]
    fn blank_text_is_no_speech() {
        assert_eq!(
            TakeResult::from_text(" \n".to_string()),
            TakeResult::NoSpeech
        );
        assert_eq!(
            TakeResult::from_text("ok".to_string()),
            TakeResult::Ok {
                text: "ok".to_string()
            }
        );
    }

    #[test]
    fn state_names_are_lowercase_words() {
        let names: Vec<&str> = [
            DaemonState::Idle,
            DaemonState::Starting,
            DaemonState::Recording { elapsed_ms: 5 },
            DaemonState::Transcribing,
            DaemonState::Inserting,
            DaemonState::Stopping,
        ]
        .iter()
        .map(DaemonState::name)
        .collect();
        assert_eq!(
            names,
            [
                "idle",
                "starting",
                "recording",
                "transcribing",
                "inserting",
                "stopping"
            ]
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
            DaemonState::Starting,
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
    fn status_replies_name_the_model_state() {
        let reply =
            DaemonResponse::ok(DaemonState::Idle).with_model_state(Some(ModelState::Unloaded));
        assert_eq!(
            serde_json::to_string(&reply).unwrap(),
            r#"{"ok":true,"state":{"state":"idle"},"model_state":"unloaded"}"#
        );
        for state in [
            ModelState::Loaded,
            ModelState::Loading,
            ModelState::Unloaded,
            ModelState::Remote,
        ] {
            let json = serde_json::to_string(&state).unwrap();
            assert_eq!(json, format!("\"{}\"", state.name()));
            assert_eq!(serde_json::from_str::<ModelState>(&json).unwrap(), state);
        }
        assert_eq!(ModelState::Remote.name(), "none");
        let old: DaemonResponse =
            serde_json::from_str(r#"{"ok":true,"state":{"state":"idle"}}"#).unwrap();
        assert_eq!(
            old.model_state, None,
            "an older daemon sends no model state"
        );
    }

    #[test]
    fn error_responses_carry_no_state() {
        let json = serde_json::to_string(&DaemonResponse::error("nope")).unwrap();
        assert_eq!(json, r#"{"ok":false,"error":"nope"}"#);
    }
}
