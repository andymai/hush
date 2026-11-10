//! Daemon side of the socket. Each connection carries one command and gets
//! one reply. Commands that change state are forwarded to the session loop;
//! the reply names the state that request leads to. A print stop replies
//! when the take ends, and a watch replies once for each state change.

use super::protocol::{
    DaemonCommand, DaemonResponse, DaemonState, ModelState, TakeResult, FILE_TIMEOUT, PRINT_TIMEOUT,
};
use crate::text_processing::{learn, Trigger};
use crate::transcription::language::{self, Language};
use anyhow::{Context, Result};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;
use tracing::{debug, warn};

/// What the session loop consumes, from the hotkey thread and from IPC alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionCommand {
    /// Start a take; the language, when set, comes from `--lang` or a
    /// language chord and applies to this take only
    StartRecording(Trigger, Option<Language>),
    /// Record a spoken instruction for Command Mode
    StartCommand,
    StopRecording,
    CancelRecording,
    /// Type the last transcript again
    PasteLast,
    /// Learn the current selection (from an action chord)
    Learn,
    /// The vocabulary file changed
    ReloadVocabulary,
    /// The tray chose the language of later takes; `None` follows the config
    ChooseLanguage(Option<Language>),
    /// A file waits in [`SharedState::next_file`]
    TranscribeFile,
    Quit,
}

enum Phase {
    Idle,
    Starting,
    Recording(Instant),
    Transcribing,
    Inserting,
    Stopping,
}

/// The client of a print stop. The take it ends types nothing and sends its
/// text here instead.
pub struct TakeWaiter(oneshot::Sender<TakeResult>);

impl TakeWaiter {
    pub fn finish(self, result: TakeResult) {
        let _ = self.0.send(result);
    }
}

/// The languages that decide the language of a take.
#[derive(Debug, Default, Clone, Copy)]
struct Languages {
    /// `transcription.language`
    config: Option<Language>,
    /// The tray choice
    chosen: Option<Language>,
    /// The language of the take in progress
    take: Option<Language>,
}

/// A file that a client asked the session to transcribe.
pub struct FileJob {
    pub path: PathBuf,
    pub reply: TakeWaiter,
}

/// Current phase, model state, languages, and the last transcript, written
/// by the session and read by IPC.
pub struct SharedState {
    phase: Mutex<Phase>,
    model: Mutex<Option<ModelState>>,
    languages: Mutex<Languages>,
    last_text: Mutex<Option<String>>,
    waiter: Mutex<Option<oneshot::Sender<TakeResult>>>,
    files: Mutex<VecDeque<FileJob>>,
    changes: watch::Sender<()>,
}

impl Default for SharedState {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedState {
    pub fn new() -> Self {
        Self {
            phase: Mutex::new(Phase::Idle),
            model: Mutex::new(None),
            languages: Mutex::new(Languages::default()),
            last_text: Mutex::new(None),
            waiter: Mutex::new(None),
            files: Mutex::new(VecDeque::new()),
            changes: watch::channel(()).0,
        }
    }

    fn set(&self, phase: Phase) {
        *self.phase.lock() = phase;
        self.changes.send_replace(());
    }

    /// Fires after each phase change.
    pub fn subscribe(&self) -> watch::Receiver<()> {
        self.changes.subscribe()
    }

    /// Make the next stop deliver its take here. A second print stop
    /// replaces the first, whose client then gets an error.
    pub fn wait_for_take(&self) -> oneshot::Receiver<TakeResult> {
        let (tx, rx) = oneshot::channel();
        *self.waiter.lock() = Some(tx);
        rx
    }

    /// Called by the session when it handles a stop.
    pub fn take_waiter(&self) -> Option<TakeWaiter> {
        self.waiter
            .lock()
            .take()
            .filter(|tx| !tx.is_closed())
            .map(TakeWaiter)
    }

    /// Queue a file for the session; the receiver gets its text.
    pub fn queue_file(&self, path: PathBuf) -> oneshot::Receiver<TakeResult> {
        let (tx, rx) = oneshot::channel();
        self.files.lock().push_back(FileJob {
            path,
            reply: TakeWaiter(tx),
        });
        rx
    }

    /// The next queued file whose client still waits.
    pub fn next_file(&self) -> Option<FileJob> {
        let mut files = self.files.lock();
        while let Some(job) = files.pop_front() {
            if !job.reply.0.is_closed() {
                return Some(job);
            }
        }
        None
    }

    pub fn set_last_text(&self, text: String) {
        *self.last_text.lock() = Some(text);
    }

    pub fn last_text(&self) -> Option<String> {
        self.last_text.lock().clone()
    }

    /// The take, if any, is over: its language no longer applies.
    pub fn set_idle(&self) {
        self.languages.lock().take = None;
        self.set(Phase::Idle);
    }

    /// The stream started and waits for its first audio frame.
    pub fn set_starting(&self) {
        self.set(Phase::Starting);
    }

    /// `since` is the time of the first audio frame.
    pub fn set_recording(&self, since: Instant) {
        self.set(Phase::Recording(since));
    }

    pub fn set_transcribing(&self) {
        self.set(Phase::Transcribing);
    }

    pub fn set_inserting(&self) {
        self.set(Phase::Inserting);
    }

    pub fn set_stopping(&self) {
        self.set(Phase::Stopping);
    }

    /// A take is in progress, whether or not its first frame arrived.
    pub fn in_take(&self) -> bool {
        matches!(*self.phase.lock(), Phase::Starting | Phase::Recording(_))
    }

    /// No take, transcription, or result processing is in progress.
    pub fn is_idle(&self) -> bool {
        matches!(*self.phase.lock(), Phase::Idle)
    }

    /// Watchers hear only a change.
    pub fn set_model_state(&self, state: ModelState) {
        let changed = self.model.lock().replace(state) != Some(state);
        if changed {
            self.changes.send_replace(());
        }
    }

    pub fn model_state(&self) -> Option<ModelState> {
        *self.model.lock()
    }

    pub fn set_config_language(&self, language: Language) {
        self.languages.lock().config = Some(language);
    }

    /// Watchers hear the change.
    pub fn set_chosen_language(&self, choice: Option<Language>) {
        self.languages.lock().chosen = choice;
        self.changes.send_replace(());
    }

    pub fn chosen_language(&self) -> Option<Language> {
        self.languages.lock().chosen
    }

    /// The language of a take that starts now, with `once` from `--lang`
    /// or a language chord. `None` until the session sets the config
    /// language.
    pub fn language_for(&self, once: Option<Language>) -> Option<Language> {
        let languages = *self.languages.lock();
        languages
            .config
            .map(|config| language::for_take(once, languages.chosen, config))
    }

    /// The language of the take that just started; [`Self::set_idle`]
    /// clears it.
    pub fn set_take_language(&self, language: Language) {
        self.languages.lock().take = Some(language);
    }

    /// The language of the take in progress, else of the next take.
    pub fn active_language(&self) -> Option<Language> {
        let take = self.languages.lock().take;
        take.or_else(|| self.language_for(None))
    }

    /// The reply to a status or watch request.
    pub fn status(&self) -> DaemonResponse {
        DaemonResponse::ok(self.snapshot())
            .with_model_state(self.model_state())
            .with_language(self.active_language().map(|l| l.code().to_string()))
    }

    pub fn snapshot(&self) -> DaemonState {
        match *self.phase.lock() {
            Phase::Idle => DaemonState::Idle,
            Phase::Starting => DaemonState::Starting,
            Phase::Recording(since) => DaemonState::Recording {
                elapsed_ms: since.elapsed().as_millis() as u64,
            },
            Phase::Transcribing => DaemonState::Transcribing,
            Phase::Inserting => DaemonState::Inserting,
            Phase::Stopping => DaemonState::Stopping,
        }
    }
}

/// Removes the socket file and stops accepting when dropped.
pub struct ServerGuard {
    path: PathBuf,
    task: JoinHandle<()>,
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        self.task.abort();
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Bind the socket and serve until the guard is dropped.
pub async fn start(
    path: &Path,
    shared: Arc<SharedState>,
    control: mpsc::Sender<SessionCommand>,
) -> Result<ServerGuard> {
    // The PID lock already proved no daemon is alive, so a leftover socket is stale.
    if path.exists() {
        std::fs::remove_file(path)
            .with_context(|| format!("Failed to remove stale socket {}", path.display()))?;
    }
    let listener =
        UnixListener::bind(path).with_context(|| format!("Failed to bind {}", path.display()))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("Failed to restrict {}", path.display()))?;
    debug!("IPC listening on {}", path.display());

    let task = tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let shared = Arc::clone(&shared);
                    let control = control.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_client(stream, &shared, &control).await {
                            warn!("IPC client error: {}", e);
                        }
                    });
                },
                Err(e) => {
                    warn!("IPC accept failed: {}", e);
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                },
            }
        }
    });

    Ok(ServerGuard {
        path: path.to_path_buf(),
        task,
    })
}

async fn handle_client(
    stream: UnixStream,
    shared: &SharedState,
    control: &mpsc::Sender<SessionCommand>,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .await
        .context("Failed to read IPC command")?;
    let response = match serde_json::from_str::<DaemonCommand>(line.trim()) {
        Ok(DaemonCommand::Watch) => return watch(reader, writer, shared).await,
        Ok(DaemonCommand::Stop { print: true }) => {
            stop_and_print(shared, control, PRINT_TIMEOUT).await
        },
        Ok(DaemonCommand::Transcribe { path }) => {
            transcribe_file(shared, control, path, FILE_TIMEOUT).await
        },
        Ok(command) => respond(command, shared, control),
        Err(e) => DaemonResponse::error(format!("Invalid command: {}", e)),
    };
    write_response(&mut writer, &response).await
}

async fn write_response(writer: &mut OwnedWriteHalf, response: &DaemonResponse) -> Result<()> {
    let mut payload = serde_json::to_string(response)?;
    payload.push('\n');
    writer
        .write_all(payload.as_bytes())
        .await
        .context("Failed to write IPC response")
}

/// Send the state now and after each change, until the client hangs up.
async fn watch(
    mut reader: BufReader<OwnedReadHalf>,
    mut writer: OwnedWriteHalf,
    shared: &SharedState,
) -> Result<()> {
    let mut changes = shared.subscribe();
    let mut scratch = [0u8; 64];
    loop {
        write_response(&mut writer, &shared.status()).await?;
        tokio::select! {
            changed = changes.changed() => {
                if changed.is_err() {
                    return Ok(());
                }
            },
            read = reader.read(&mut scratch) => {
                if matches!(read, Ok(0) | Err(_)) {
                    return Ok(());
                }
            },
        }
    }
}

/// Stop the take and reply with its text when the session finishes it. The
/// session decides whether a take records, because a start sent just before
/// can still wait in the command channel.
pub async fn stop_and_print(
    shared: &SharedState,
    control: &mpsc::Sender<SessionCommand>,
    timeout: Duration,
) -> DaemonResponse {
    let result = shared.wait_for_take();
    if control.send(SessionCommand::StopRecording).is_err() {
        return DaemonResponse::error("The daemon session is no longer running");
    }
    match tokio::time::timeout(timeout, result).await {
        Ok(Ok(take)) => DaemonResponse::ok(shared.snapshot()).with_take(take),
        Ok(Err(_)) => DaemonResponse::error("The take ended without a result"),
        Err(_) => DaemonResponse::error(format!(
            "Hush did not finish the take within {} s",
            timeout.as_secs()
        )),
    }
}

/// Have the session transcribe a file, and reply with its text.
pub async fn transcribe_file(
    shared: &SharedState,
    control: &mpsc::Sender<SessionCommand>,
    path: PathBuf,
    timeout: Duration,
) -> DaemonResponse {
    if !path.is_absolute() {
        return DaemonResponse::error(format!("{} is not an absolute path", path.display()));
    }
    let result = shared.queue_file(path);
    if control.send(SessionCommand::TranscribeFile).is_err() {
        return DaemonResponse::error("The daemon session is no longer running");
    }
    match tokio::time::timeout(timeout, result).await {
        Ok(Ok(take)) => DaemonResponse::ok(shared.snapshot()).with_take(take),
        Ok(Err(_)) => DaemonResponse::error("The file ended without a result"),
        Err(_) => DaemonResponse::error(format!(
            "Hush did not transcribe the file within {} minutes",
            timeout.as_secs() / 60
        )),
    }
}

/// Map a command onto the session and describe the state it leads to.
pub fn respond(
    command: DaemonCommand,
    shared: &SharedState,
    control: &mpsc::Sender<SessionCommand>,
) -> DaemonResponse {
    let forward = |session: SessionCommand, next: DaemonState| match control.send(session) {
        Ok(()) => DaemonResponse::ok(next),
        Err(_) => DaemonResponse::error("The daemon session is no longer running"),
    };
    // The reply names the take's language, so a client can tell this
    // daemon from an older one that ignores `language`.
    let start = |language: Option<String>| {
        let once = match language.as_deref().map(Language::from_code).transpose() {
            Ok(once) => once,
            Err(e) => return DaemonResponse::error(e.to_string()),
        };
        let reply = forward(
            SessionCommand::StartRecording(Trigger::Ipc, once),
            DaemonState::Starting,
        );
        if !reply.ok {
            return reply;
        }
        let language = shared.language_for(once).or(once);
        reply.with_language(language.map(|l| l.code().to_string()))
    };
    // A stop before the first audio frame discards the take.
    let after_stop = match shared.snapshot() {
        DaemonState::Starting => DaemonState::Idle,
        DaemonState::Recording { .. } => DaemonState::Transcribing,
        other => other,
    };
    match command {
        DaemonCommand::Status | DaemonCommand::Watch => shared.status(),
        DaemonCommand::Toggle { language } => {
            if shared.in_take() {
                forward(SessionCommand::StopRecording, after_stop)
            } else {
                start(language)
            }
        },
        DaemonCommand::Start { language } => start(language),
        DaemonCommand::Stop { .. } => forward(SessionCommand::StopRecording, after_stop),
        DaemonCommand::Cancel => forward(SessionCommand::CancelRecording, DaemonState::Idle),
        DaemonCommand::Quit => forward(SessionCommand::Quit, DaemonState::Stopping),
        DaemonCommand::PasteLast => {
            if shared.last_text().is_none() {
                DaemonResponse::error("Nothing to paste yet: dictate something first")
            } else {
                forward(SessionCommand::PasteLast, DaemonState::Inserting)
            }
        },
        DaemonCommand::Learn { text } => match learn::learn(text.as_deref()) {
            Ok(term) => forward(SessionCommand::ReloadVocabulary, shared.snapshot())
                .with_message(format!("Learned '{}'", term)),
            Err(e) => DaemonResponse::error(e.to_string()),
        },
        DaemonCommand::Transcribe { .. } => {
            DaemonResponse::error("A transcription waits for its reply; send it over the socket")
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::client;

    #[test]
    fn toggle_follows_the_take() {
        let shared = SharedState::new();
        let (tx, rx) = mpsc::channel();
        let toggle = || DaemonCommand::Toggle { language: None };
        assert_eq!(
            respond(toggle(), &shared, &tx).state,
            Some(DaemonState::Starting)
        );
        assert_eq!(
            rx.try_recv(),
            Ok(SessionCommand::StartRecording(Trigger::Ipc, None))
        );

        shared.set_starting();
        assert_eq!(
            respond(toggle(), &shared, &tx).state,
            Some(DaemonState::Idle),
            "a stop before the first frame discards the take"
        );
        assert_eq!(rx.try_recv(), Ok(SessionCommand::StopRecording));

        shared.set_recording(Instant::now());
        assert_eq!(
            respond(toggle(), &shared, &tx).state,
            Some(DaemonState::Transcribing)
        );
        assert_eq!(rx.try_recv(), Ok(SessionCommand::StopRecording));

        assert!(matches!(
            respond(DaemonCommand::Status, &shared, &tx).state,
            Some(DaemonState::Recording { .. })
        ));
    }

    #[test]
    fn status_carries_the_model_state() {
        let shared = SharedState::new();
        let (tx, _rx) = mpsc::channel();
        assert_eq!(
            respond(DaemonCommand::Status, &shared, &tx).model_state,
            None
        );
        assert!(shared.is_idle());

        let mut changes = shared.subscribe();
        changes.mark_unchanged();
        shared.set_model_state(ModelState::Loaded);
        assert!(changes.has_changed().unwrap());
        changes.mark_unchanged();
        shared.set_model_state(ModelState::Loaded);
        assert!(!changes.has_changed().unwrap(), "only a change is sent");

        shared.set_model_state(ModelState::Unloaded);
        let reply = respond(DaemonCommand::Status, &shared, &tx);
        assert_eq!(reply.state, Some(DaemonState::Idle));
        assert_eq!(reply.model_state, Some(ModelState::Unloaded));

        shared.set_transcribing();
        assert!(!shared.is_idle());
    }

    #[test]
    fn paste_last_needs_a_transcript() {
        let shared = SharedState::new();
        let (tx, rx) = mpsc::channel();
        let response = respond(DaemonCommand::PasteLast, &shared, &tx);
        assert!(!response.ok);
        assert!(rx.try_recv().is_err());
        shared.set_last_text("hello".to_string());
        assert_eq!(
            respond(DaemonCommand::PasteLast, &shared, &tx).state,
            Some(DaemonState::Inserting)
        );
        assert_eq!(rx.try_recv(), Ok(SessionCommand::PasteLast));
    }

    #[test]
    fn dead_session_is_reported() {
        let shared = SharedState::new();
        let (tx, rx) = mpsc::channel::<SessionCommand>();
        drop(rx);
        let response = respond(DaemonCommand::Start { language: None }, &shared, &tx);
        assert!(!response.ok);
        assert!(response.error.unwrap().contains("no longer running"));
    }

    fn lang(code: &str) -> Language {
        Language::parse(code).unwrap()
    }

    #[test]
    fn a_start_in_a_language_reaches_the_session_and_names_it() {
        let shared = SharedState::new();
        shared.set_config_language(lang("en"));
        let (tx, rx) = mpsc::channel();
        let start = |code: &str| DaemonCommand::Start {
            language: Some(code.to_string()),
        };

        let reply = respond(start("de"), &shared, &tx);
        assert_eq!(reply.state, Some(DaemonState::Starting));
        assert_eq!(reply.language.as_deref(), Some("de"));
        assert_eq!(
            rx.try_recv(),
            Ok(SessionCommand::StartRecording(
                Trigger::Ipc,
                Some(lang("de"))
            ))
        );

        let reply = respond(DaemonCommand::Start { language: None }, &shared, &tx);
        assert_eq!(reply.language.as_deref(), Some("en"), "the config language");
        shared.set_chosen_language(Some(lang("fr")));
        let reply = respond(DaemonCommand::Toggle { language: None }, &shared, &tx);
        assert_eq!(reply.language.as_deref(), Some("fr"), "the tray choice");
        while rx.try_recv().is_ok() {}

        let reply = respond(start("klingon"), &shared, &tx);
        assert!(!reply.ok);
        assert!(reply.error.unwrap().contains("'klingon'"));
        assert!(rx.try_recv().is_err(), "nothing starts");

        shared.set_recording(Instant::now());
        let reply = respond(
            DaemonCommand::Toggle {
                language: Some("de".to_string()),
            },
            &shared,
            &tx,
        );
        assert_eq!(reply.state, Some(DaemonState::Transcribing));
        assert_eq!(reply.language, None, "a toggle that stops starts no take");
        assert_eq!(rx.try_recv(), Ok(SessionCommand::StopRecording));
    }

    #[test]
    fn status_names_the_language_of_the_take_or_the_next_one() {
        let shared = SharedState::new();
        assert_eq!(shared.status().language, None);
        shared.set_config_language(lang("en"));
        assert_eq!(shared.status().language.as_deref(), Some("en"));

        let mut changes = shared.subscribe();
        changes.mark_unchanged();
        shared.set_chosen_language(Some(Language::AUTO));
        assert!(
            changes.has_changed().unwrap(),
            "watchers hear a tray choice"
        );
        assert_eq!(shared.status().language.as_deref(), Some("auto"));
        assert_eq!(shared.chosen_language(), Some(Language::AUTO));

        shared.set_take_language(lang("de"));
        shared.set_recording(Instant::now());
        assert_eq!(shared.status().language.as_deref(), Some("de"));
        shared.set_transcribing();
        assert_eq!(shared.status().language.as_deref(), Some("de"));
        shared.set_idle();
        assert_eq!(shared.status().language.as_deref(), Some("auto"));
        assert_eq!(shared.language_for(Some(lang("fr"))), Some(lang("fr")));
    }

    #[test]
    fn a_plain_stop_without_a_take_keeps_the_state() {
        let shared = SharedState::new();
        let (tx, _rx) = mpsc::channel();
        assert_eq!(
            respond(DaemonCommand::Stop { print: false }, &shared, &tx).state,
            Some(DaemonState::Idle)
        );
    }

    #[tokio::test]
    async fn print_stop_gives_up_after_the_timeout() {
        let shared = SharedState::new();
        let (tx, rx) = mpsc::channel();
        let response = stop_and_print(&shared, &tx, Duration::from_millis(20)).await;
        assert_eq!(rx.try_recv(), Ok(SessionCommand::StopRecording));
        assert!(!response.ok);
        assert!(response.error.unwrap().contains("did not finish"));
        assert!(
            shared.take_waiter().is_none(),
            "the next stop types its take again"
        );
    }

    #[tokio::test]
    async fn print_stop_reports_a_dead_session() {
        let shared = SharedState::new();
        let (tx, rx) = mpsc::channel();
        drop(rx);
        let response = stop_and_print(&shared, &tx, Duration::from_secs(1)).await;
        assert!(!response.ok);
        assert!(response.error.unwrap().contains("no longer running"));
    }

    #[test]
    fn a_waiter_whose_client_left_is_dropped() {
        let shared = SharedState::new();
        drop(shared.wait_for_take());
        assert!(shared.take_waiter().is_none());
        let _rx = shared.wait_for_take();
        assert!(shared.take_waiter().is_some());
        assert!(shared.take_waiter().is_none(), "a waiter serves one take");
    }

    #[tokio::test]
    async fn print_stop_replies_with_the_take_over_the_socket() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hush.sock");
        let shared = Arc::new(SharedState::new());
        let (tx, rx) = mpsc::channel();
        let _guard = start(&path, Arc::clone(&shared), tx).await.unwrap();
        shared.set_recording(Instant::now());

        let session_shared = Arc::clone(&shared);
        let session = std::thread::spawn(move || {
            assert_eq!(rx.recv().unwrap(), SessionCommand::StopRecording);
            let waiter = session_shared.take_waiter().expect("a print stop waits");
            session_shared.set_idle();
            waiter.finish(TakeResult::Ok {
                text: "Ship it.".to_string(),
            });
        });

        let response = client::send_to(&path, DaemonCommand::Stop { print: true })
            .await
            .unwrap();
        session.join().unwrap();
        assert!(response.ok);
        assert_eq!(
            response.take,
            Some(TakeResult::Ok {
                text: "Ship it.".to_string()
            })
        );
    }

    #[tokio::test]
    async fn a_file_reaches_the_session_and_its_text_comes_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hush.sock");
        let shared = Arc::new(SharedState::new());
        let (tx, rx) = mpsc::channel();
        let _guard = start(&path, Arc::clone(&shared), tx).await.unwrap();

        let session_shared = Arc::clone(&shared);
        let session = std::thread::spawn(move || {
            assert_eq!(rx.recv().unwrap(), SessionCommand::TranscribeFile);
            let job = session_shared.next_file().expect("a file waits");
            assert_eq!(job.path, PathBuf::from("/tmp/take.flac"));
            job.reply.finish(TakeResult::Ok {
                text: "Recovered.".to_string(),
            });
            assert!(session_shared.next_file().is_none());
        });

        let command = DaemonCommand::Transcribe {
            path: PathBuf::from("/tmp/take.flac"),
        };
        let response = client::send_to(&path, command).await.unwrap();
        session.join().unwrap();
        assert!(response.ok);
        assert_eq!(
            response.take,
            Some(TakeResult::Ok {
                text: "Recovered.".to_string()
            })
        );
    }

    #[tokio::test]
    async fn a_relative_file_path_is_refused_and_a_gone_client_is_skipped() {
        let shared = SharedState::new();
        let (tx, rx) = mpsc::channel();
        let response =
            transcribe_file(&shared, &tx, PathBuf::from("take.flac"), FILE_TIMEOUT).await;
        assert!(!response.ok);
        assert!(rx.try_recv().is_err(), "nothing reaches the session");

        drop(shared.queue_file(PathBuf::from("/tmp/a.flac")));
        let _waiting = shared.queue_file(PathBuf::from("/tmp/b.flac"));
        assert_eq!(
            shared.next_file().map(|job| job.path),
            Some(PathBuf::from("/tmp/b.flac"))
        );
    }

    #[tokio::test]
    async fn watch_sends_the_state_now_and_on_each_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hush.sock");
        let shared = Arc::new(SharedState::new());
        let (tx, _rx) = mpsc::channel();
        let guard = start(&path, Arc::clone(&shared), tx).await.unwrap();

        let mut watcher = client::Watcher::connect(&path).await.unwrap();
        let first = watcher.next().await.unwrap().unwrap();
        assert_eq!(first.state, Some(DaemonState::Idle));

        shared.set_starting();
        let next = watcher.next().await.unwrap().unwrap();
        assert_eq!(next.state, Some(DaemonState::Starting));
        drop(guard);
    }

    #[tokio::test]
    async fn client_and_server_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hush.sock");
        let shared = Arc::new(SharedState::new());
        let (tx, rx) = mpsc::channel();
        let guard = start(&path, Arc::clone(&shared), tx).await.unwrap();

        let status = client::send_to(&path, DaemonCommand::Status).await.unwrap();
        assert_eq!(status.state, Some(DaemonState::Idle));

        let started = client::send_to(&path, DaemonCommand::Start { language: None })
            .await
            .unwrap();
        assert_eq!(started.state, Some(DaemonState::Starting));
        assert_eq!(
            rx.recv().unwrap(),
            SessionCommand::StartRecording(Trigger::Ipc, None)
        );

        drop(guard);
        assert!(!path.exists());
        assert!(client::send_to(&path, DaemonCommand::Status).await.is_err());
    }
}
