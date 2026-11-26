use crate::audio::take::StartDelay;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

/// How long `hush stop --print` waits for the final text.
pub const PRINT_TIMEOUT: Duration = Duration::from_secs(60);

/// How long the daemon gets to transcribe a file for `hush transcribe`.
pub const FILE_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// The reply to a print stop while no take records.
pub const NO_TAKE: &str = "No recording is in progress. Start one with: hush start";

/// Sent by a client, one JSON object per line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum DaemonCommand {
    /// Start recording if idle, otherwise stop and transcribe. `language`
    /// and `style` apply to the take it starts. A daemon from before a field
    /// ignores it, and its reply then does not carry that field.
    Toggle {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        language: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        style: Option<String>,
    },
    /// Start recording, in `language` and `style` when set
    Start {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        language: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        style: Option<String>,
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
    /// Keep the connection open and send the state at once and on each
    /// change. `meter` adds the microphone level while a take records, and
    /// `events` adds errors. A daemon from before these fields sends neither.
    Watch {
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        meter: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        events: bool,
    },
    Quit,
    /// Keep the audio of the next take, from any trigger, in a file that only
    /// the user can read, for `hush tune`
    KeepNext,
    /// Type the last transcript again
    PasteLast,
    /// Add words to the vocabulary; without `text`, the current selection
    Learn {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    /// Transcribe a WAV or FLAC file with the daemon's model and reply with
    /// the text; `path` is absolute
    Transcribe {
        path: PathBuf,
    },
    /// Rewrite the selected text with the prompt of `style`
    Rewrite {
        style: String,
    },
    /// In portal mode, the shortcuts and the keys the desktop binds them
    /// to; with `configure`, open the desktop's shortcut dialog first
    Shortcuts {
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        configure: bool,
    },
}

/// A GlobalShortcuts portal shortcut as the desktop reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortalShortcut {
    pub id: String,
    pub description: String,
    /// What the desktop shows, such as `Meta+Space`, or empty for none.
    pub trigger: String,
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
        /// Hands-free: the take records without the key held
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        locked: bool,
        /// A Command Mode take, whose text is an instruction for the selection
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        command: bool,
        /// Once the recording cap is near: the time until it stops the take
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stops_in_ms: Option<u64>,
    },
    Transcribing,
    Inserting,
    Stopping,
}

impl DaemonState {
    /// A take that records with the key held, `elapsed_ms` after its first
    /// audio frame.
    pub fn recording(elapsed_ms: u64) -> Self {
        DaemonState::Recording {
            elapsed_ms,
            locked: false,
            command: false,
            stops_in_ms: None,
        }
    }

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
            DaemonState::Recording {
                elapsed_ms,
                locked,
                command,
                ..
            } => {
                let kind = match (command, locked) {
                    (true, _) => " a command",
                    (false, true) => " hands-free",
                    (false, false) => "",
                };
                write!(
                    f,
                    "recording{} ({:.1} s)",
                    kind,
                    *elapsed_ms as f64 / 1000.0
                )
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

/// Whether the daemon shows the recording overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlayStatus {
    Running,
    /// The session has neither a layer-shell compositor nor an X11 display
    Unavailable,
    /// The overlay crashed again and again, or Hush was updated under it
    Failed,
    /// `overlay.enabled` is off
    Off,
}

/// Something that happened, sent to a watch that asked for events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum WatchEvent {
    /// A take or its transcription failed
    Error { message: String },
}

/// The microphone level of the take that records, from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Level {
    pub level: f32,
}

/// One line of a watch: the state, and on request the level and events.
/// Each kind has a key that the others lack (`level`, `event`, `ok`), so
/// the order of the variants decides nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WatchLine {
    Level(Level),
    Event(WatchEvent),
    State(Box<DaemonResponse>),
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
    /// The style that a start asked for, for the take it began
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// In a status reply: why the daemon runs in safe mode, which writes
    /// and deletes nothing in the history or the kept audio
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safe_mode: Option<String>,
    /// In a status reply: how long recent takes waited for the first audio
    /// from the microphone, `None` before the first take
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_delay: Option<StartDelay>,
    /// In a shortcuts reply: the portal shortcuts
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcuts: Option<Vec<PortalShortcut>>,
    /// In a status or watch reply: the daemon's version, which a client
    /// compares with its own after an upgrade
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// In a status or watch reply: whether the overlay shows
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay: Option<OverlayStatus>,
    /// In a status or watch reply: the next take's audio will be kept, so a
    /// restart now would lose the request
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub keep_next: bool,
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
            style: None,
            safe_mode: None,
            start_delay: None,
            shortcuts: None,
            version: None,
            overlay: None,
            keep_next: false,
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
            style: None,
            safe_mode: None,
            start_delay: None,
            shortcuts: None,
            version: None,
            overlay: None,
            keep_next: false,
        }
    }

    pub fn with_keep_next(mut self, keep_next: bool) -> Self {
        self.keep_next = keep_next;
        self
    }

    pub fn with_overlay(mut self, overlay: Option<OverlayStatus>) -> Self {
        self.overlay = overlay;
        self
    }

    pub fn with_version(mut self, version: &str) -> Self {
        self.version = Some(version.to_string());
        self
    }

    pub fn with_shortcuts(mut self, shortcuts: Vec<PortalShortcut>) -> Self {
        self.shortcuts = Some(shortcuts);
        self
    }

    pub fn with_safe_mode(mut self, safe_mode: Option<String>) -> Self {
        self.safe_mode = safe_mode;
        self
    }

    pub fn with_start_delay(mut self, start_delay: Option<StartDelay>) -> Self {
        self.start_delay = start_delay;
        self
    }

    pub fn with_language(mut self, language: Option<String>) -> Self {
        self.language = language;
        self
    }

    pub fn with_style(mut self, style: Option<String>) -> Self {
        self.style = style;
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
            serde_json::to_string(&DaemonCommand::Toggle {
                language: None,
                style: None
            })
            .unwrap(),
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
        let transcribe = DaemonCommand::Transcribe {
            path: PathBuf::from("/tmp/take.flac"),
        };
        let json = serde_json::to_string(&transcribe).unwrap();
        assert_eq!(json, r#"{"command":"transcribe","path":"/tmp/take.flac"}"#);
        assert_eq!(
            serde_json::from_str::<DaemonCommand>(&json).unwrap(),
            transcribe
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
            serde_json::to_string(&DaemonCommand::Watch {
                meter: false,
                events: false
            })
            .unwrap(),
            r#"{"command":"watch"}"#
        );
    }

    #[test]
    fn a_watch_asks_for_the_meter_and_events_and_an_older_daemon_ignores_them() {
        let plain: DaemonCommand = serde_json::from_str(r#"{"command":"watch"}"#).unwrap();
        assert_eq!(
            plain,
            DaemonCommand::Watch {
                meter: false,
                events: false
            },
            "an older client"
        );
        let both = DaemonCommand::Watch {
            meter: true,
            events: true,
        };
        let json = serde_json::to_string(&both).unwrap();
        assert_eq!(json, r#"{"command":"watch","meter":true,"events":true}"#);
        assert_eq!(serde_json::from_str::<DaemonCommand>(&json).unwrap(), both);

        #[derive(Debug, PartialEq, Deserialize)]
        #[serde(tag = "command", rename_all = "snake_case")]
        enum OldCommand {
            Watch,
        }
        let old: OldCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(old, OldCommand::Watch, "an older daemon");
    }

    #[test]
    fn watch_lines_tell_the_state_the_level_and_events_apart() {
        let lines = [
            WatchLine::State(Box::new(DaemonResponse::ok(DaemonState::recording(40)))),
            WatchLine::Level(Level { level: 0.5 }),
            WatchLine::Event(WatchEvent::Error {
                message: "The microphone did not start".into(),
            }),
        ];
        let json: Vec<String> = lines
            .iter()
            .map(|line| serde_json::to_string(line).unwrap())
            .collect();
        assert_eq!(
            json,
            [
                r#"{"ok":true,"state":{"state":"recording","elapsed_ms":40}}"#,
                r#"{"level":0.5}"#,
                r#"{"event":"error","message":"The microphone did not start"}"#,
            ]
        );
        for (line, json) in lines.iter().zip(&json) {
            assert_eq!(&serde_json::from_str::<WatchLine>(json).unwrap(), line);
        }
        let failure: WatchLine = serde_json::from_str(r#"{"ok":false,"error":"nope"}"#).unwrap();
        assert_eq!(
            failure,
            WatchLine::State(Box::new(DaemonResponse::error("nope")))
        );
    }

    #[test]
    fn start_and_toggle_keep_the_old_form_and_add_a_language() {
        for (command, old, new) in [
            (
                DaemonCommand::Start {
                    language: None,
                    style: None,
                },
                r#"{"command":"start"}"#,
                r#"{"command":"start","language":"de"}"#,
            ),
            (
                DaemonCommand::Toggle {
                    language: None,
                    style: None,
                },
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
                    style: None,
                },
                _ => DaemonCommand::Toggle {
                    language: Some("de".to_string()),
                    style: None,
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
    fn a_rewrite_names_its_style() {
        let rewrite = DaemonCommand::Rewrite {
            style: "formal".to_string(),
        };
        let json = r#"{"command":"rewrite","style":"formal"}"#;
        assert_eq!(serde_json::to_string(&rewrite).unwrap(), json);
        assert_eq!(
            serde_json::from_str::<DaemonCommand>(json).unwrap(),
            rewrite
        );
        assert!(
            serde_json::from_str::<DaemonCommand>(r#"{"command":"rewrite"}"#).is_err(),
            "a rewrite needs a style"
        );
    }

    #[test]
    fn start_and_toggle_add_a_style() {
        let start = DaemonCommand::Start {
            language: None,
            style: Some("verbatim".to_string()),
        };
        let json = r#"{"command":"start","style":"verbatim"}"#;
        assert_eq!(serde_json::to_string(&start).unwrap(), json);
        assert_eq!(serde_json::from_str::<DaemonCommand>(json).unwrap(), start);
        let toggle = DaemonCommand::Toggle {
            language: Some("de".to_string()),
            style: Some("casual".to_string()),
        };
        let json = r#"{"command":"toggle","language":"de","style":"casual"}"#;
        assert_eq!(serde_json::to_string(&toggle).unwrap(), json);
        assert_eq!(serde_json::from_str::<DaemonCommand>(json).unwrap(), toggle);

        #[derive(Debug, PartialEq, Deserialize)]
        #[serde(tag = "command", rename_all = "snake_case")]
        enum OldCommand {
            Start {
                #[serde(default)]
                language: Option<String>,
            },
        }
        let old: OldCommand =
            serde_json::from_str(r#"{"command":"start","style":"verbatim"}"#).unwrap();
        assert_eq!(old, OldCommand::Start { language: None }, "an older daemon");

        let reply = DaemonResponse::ok(DaemonState::Starting).with_style(Some("verbatim".into()));
        let json = serde_json::to_string(&reply).unwrap();
        assert_eq!(
            json,
            r#"{"ok":true,"state":{"state":"starting"},"style":"verbatim"}"#
        );
        assert_eq!(
            serde_json::from_str::<DaemonResponse>(&json).unwrap(),
            reply
        );
        let old: DaemonResponse =
            serde_json::from_str(r#"{"ok":true,"state":{"state":"starting"}}"#).unwrap();
        assert_eq!(old.style, None, "the reply of an older daemon");
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
            DaemonState::recording(5),
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
            DaemonState::recording(1500),
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
            serde_json::to_string(&DaemonState::recording(20)).unwrap(),
            r#"{"state":"recording","elapsed_ms":20}"#
        );
        assert_eq!(
            DaemonState::recording(1250).to_string(),
            "recording (1.2 s)"
        );
    }

    #[test]
    fn a_recording_names_its_lock_command_and_cap_only_when_set() {
        let locked = DaemonState::Recording {
            elapsed_ms: 2000,
            locked: true,
            command: false,
            stops_in_ms: Some(45_000),
        };
        let json = serde_json::to_string(&locked).unwrap();
        assert_eq!(
            json,
            r#"{"state":"recording","elapsed_ms":2000,"locked":true,"stops_in_ms":45000}"#
        );
        assert_eq!(serde_json::from_str::<DaemonState>(&json).unwrap(), locked);
        assert_eq!(
            serde_json::from_str::<DaemonState>(r#"{"state":"recording","elapsed_ms":7}"#).unwrap(),
            DaemonState::recording(7),
            "an older daemon"
        );
        assert_eq!(locked.to_string(), "recording hands-free (2.0 s)");
        let command = DaemonState::Recording {
            elapsed_ms: 500,
            locked: false,
            command: true,
            stops_in_ms: None,
        };
        assert_eq!(command.to_string(), "recording a command (0.5 s)");
    }

    #[test]
    fn keep_next_is_a_plain_command_and_a_flag_only_when_set() {
        assert_eq!(
            serde_json::to_string(&DaemonCommand::KeepNext).unwrap(),
            r#"{"command":"keep_next"}"#
        );
        let idle = DaemonResponse::ok(DaemonState::Idle);
        assert!(!serde_json::to_string(&idle).unwrap().contains("keep_next"));
        let waiting = idle.with_keep_next(true);
        let json = serde_json::to_string(&waiting).unwrap();
        assert!(json.ends_with(r#""keep_next":true}"#), "{json}");
        assert!(
            serde_json::from_str::<DaemonResponse>(&json)
                .unwrap()
                .keep_next
        );
    }

    #[test]
    fn status_replies_carry_the_version_only_when_set() {
        let reply = DaemonResponse::ok(DaemonState::Idle).with_version("1.2.0");
        let json = serde_json::to_string(&reply).unwrap();
        assert_eq!(
            json,
            r#"{"ok":true,"state":{"state":"idle"},"version":"1.2.0"}"#
        );
        let old: DaemonResponse =
            serde_json::from_str(r#"{"ok":true,"state":{"state":"idle"}}"#).unwrap();
        assert_eq!(old.version, None, "an older daemon");
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
    fn status_replies_carry_the_start_delay_and_old_ones_parse() {
        let delay = StartDelay {
            last_ms: 40,
            worst_ms: 650,
            takes: 3,
        };
        let reply = DaemonResponse::ok(DaemonState::Idle).with_start_delay(Some(delay));
        let json = serde_json::to_string(&reply).unwrap();
        assert_eq!(
            json,
            r#"{"ok":true,"state":{"state":"idle"},"start_delay":{"last_ms":40,"worst_ms":650,"takes":3}}"#
        );
        assert_eq!(
            serde_json::from_str::<DaemonResponse>(&json).unwrap(),
            reply
        );
        let old: DaemonResponse =
            serde_json::from_str(r#"{"ok":true,"state":{"state":"idle"}}"#).unwrap();
        assert_eq!(old.start_delay, None, "an older daemon measures nothing");

        #[derive(Debug, Deserialize)]
        struct OldReply {
            ok: bool,
        }
        let older: OldReply = serde_json::from_str(&json).unwrap();
        assert!(older.ok, "an older client ignores the new key");
    }

    #[test]
    fn error_responses_carry_no_state() {
        let json = serde_json::to_string(&DaemonResponse::error("nope")).unwrap();
        assert_eq!(json, r#"{"ok":false,"error":"nope"}"#);
    }
}
