//! Daemon side of the socket. Each connection carries one command and gets
//! one reply. Commands that change state are forwarded to the session loop;
//! the reply names the state that request leads to. A print stop replies
//! when the take ends, and a watch replies once for each state change.

use super::protocol::{
    DaemonCommand, DaemonResponse, DaemonState, Level, ModelState, OverlayStatus, TakeResult,
    WatchEvent, FILE_TIMEOUT, PRINT_TIMEOUT,
};
use crate::audio::take::{StartDelay, StartDelays};
use crate::portal::shortcuts::ShortcutsHandle;
use crate::text_processing::styles::{self, StyleName};
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
use tokio::sync::{broadcast, oneshot, watch};
use tokio::task::JoinHandle;
use tracing::{debug, warn};

/// What one take asks for beyond the settings, from `--lang`, `--translate`,
/// and `--style` or from a chord. It applies to that take only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Once {
    pub language: Option<Language>,
    /// The take comes out in English
    pub translate: bool,
    pub style: Option<StyleName>,
}

/// What the session loop consumes, from the hotkey thread and from IPC alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionCommand {
    /// Start a take
    StartRecording(Trigger, Once),
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
    /// Rewrite the selected text with the prompt of the style
    Rewrite(StyleName),
    /// Free the local model while no take runs
    UnloadModel,
    /// Load the local model when it is unloaded
    LoadModel,
    Quit,
}

/// What a take carries from its start into its recording. A phase outside
/// the take carries none, so nothing here outlives the take.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct TakeFlags {
    locked: bool,
    command: bool,
    style: Option<StyleName>,
    stops_at: Option<Instant>,
}

enum Phase {
    Idle,
    Starting(TakeFlags),
    Recording(Instant, TakeFlags),
    Transcribing,
    Inserting,
    Stopping,
}

/// How long a request to keep the next take waits for that take.
pub const KEEP_NEXT_FOR: Duration = Duration::from_secs(120);

/// How many events a slow watch can fall behind before it misses some.
const EVENT_BACKLOG: usize = 16;

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

/// Current phase, model state, languages, styles, the last transcript, and
/// the start delays of recent takes, written by the session and read by IPC.
pub struct SharedState {
    phase: Mutex<Phase>,
    model: Mutex<Option<ModelState>>,
    languages: Mutex<Languages>,
    styles: Mutex<Vec<StyleName>>,
    rewriters: Mutex<Vec<StyleName>>,
    last_text: Mutex<Option<String>>,
    waiter: Mutex<Option<oneshot::Sender<TakeResult>>>,
    files: Mutex<VecDeque<FileJob>>,
    changes: watch::Sender<()>,
    /// The microphone level of the take that records. A watch reads only the
    /// newest value, so a slow reader gets fewer lines, never a backlog.
    level: watch::Sender<f32>,
    events: broadcast::Sender<WatchEvent>,
    overlay: Mutex<Option<OverlayStatus>>,
    /// Until when the next take's audio is kept
    keep_next: Mutex<Option<Instant>>,
    safe_mode: Mutex<Option<String>>,
    start_delays: Mutex<StartDelays>,
    /// The GlobalShortcuts session, in portal mode.
    shortcuts: Mutex<Option<ShortcutsHandle>>,
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
            styles: Mutex::new(vec![styles::VERBATIM]),
            rewriters: Mutex::new(Vec::new()),
            last_text: Mutex::new(None),
            waiter: Mutex::new(None),
            files: Mutex::new(VecDeque::new()),
            changes: watch::channel(()).0,
            level: watch::channel(0.0).0,
            events: broadcast::channel(EVENT_BACKLOG).0,
            overlay: Mutex::new(None),
            keep_next: Mutex::new(None),
            safe_mode: Mutex::new(None),
            start_delays: Mutex::new(StartDelays::default()),
            shortcuts: Mutex::new(None),
        }
    }

    /// The portal's shortcuts, for `hush shortcuts` and the settings window.
    pub fn set_shortcuts(&self, handle: Option<ShortcutsHandle>) {
        *self.shortcuts.lock() = handle;
    }

    fn shortcuts(&self) -> Option<ShortcutsHandle> {
        self.shortcuts.lock().clone()
    }

    /// The daemon runs in safe mode for `reason`; status replies say so.
    pub fn set_safe_mode(&self, reason: Option<String>) {
        *self.safe_mode.lock() = reason;
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

    /// The stream started and waits for its first audio frame. `command`
    /// marks a Command Mode take, and `style` is the style that its start
    /// asked for.
    pub fn set_starting(&self, command: bool, style: Option<StyleName>) {
        self.set(Phase::Starting(TakeFlags {
            command,
            style,
            ..TakeFlags::default()
        }));
    }

    /// The first audio frame of a take arrived `delay` after its stream
    /// started.
    pub fn record_start_delay(&self, delay: Duration) {
        self.start_delays.lock().record(delay);
    }

    pub fn start_delay(&self) -> Option<StartDelay> {
        self.start_delays.lock().summary()
    }

    /// `since` is the time of the first audio frame. The take keeps the
    /// flags that it got while it started.
    pub fn set_recording(&self, since: Instant) {
        let mut phase = self.phase.lock();
        let flags = match *phase {
            Phase::Starting(flags) | Phase::Recording(_, flags) => flags,
            _ => TakeFlags::default(),
        };
        *phase = Phase::Recording(since, flags);
        drop(phase);
        self.changes.send_replace(());
    }

    /// The take in progress records hands-free.
    pub fn set_locked(&self) {
        self.update_take(|flags| flags.locked = true);
    }

    /// The recording cap stops the take in progress at `at`.
    pub fn set_stops_at(&self, at: Instant) {
        self.update_take(|flags| flags.stops_at = Some(at));
    }

    /// Watchers hear a change of the flags; outside a take, nothing changes.
    fn update_take(&self, change: impl FnOnce(&mut TakeFlags)) {
        let mut phase = self.phase.lock();
        let (Phase::Starting(flags) | Phase::Recording(_, flags)) = &mut *phase else {
            return;
        };
        let before = *flags;
        change(flags);
        let changed = *flags != before;
        drop(phase);
        if changed {
            self.changes.send_replace(());
        }
    }

    /// Keep the next take's audio, if it starts within [`KEEP_NEXT_FOR`].
    /// Safe mode writes no audio, so it refuses.
    pub fn request_keep_next(&self, now: Instant) -> std::result::Result<(), &'static str> {
        if self.safe_mode.lock().is_some() {
            return Err("Hush runs in safe mode, which keeps no audio");
        }
        *self.keep_next.lock() = Some(now + KEEP_NEXT_FOR);
        self.changes.send_replace(());
        Ok(())
    }

    /// The take that ends now keeps its audio: true once per request, and
    /// only before the request expires.
    pub fn take_keep_next(&self, now: Instant) -> bool {
        let Some(until) = self.keep_next.lock().take() else {
            return false;
        };
        self.changes.send_replace(());
        now < until
    }

    fn keep_next_pending(&self) -> bool {
        self.keep_next
            .lock()
            .is_some_and(|until| Instant::now() < until)
    }

    /// Watchers hear only a change.
    pub fn set_overlay(&self, status: OverlayStatus) {
        let changed = self.overlay.lock().replace(status) != Some(status);
        if changed {
            self.changes.send_replace(());
        }
    }

    /// The microphone level of the take that records, from 0 to 1.
    pub fn publish_level(&self, level: f32) {
        self.level.send_replace(level.clamp(0.0, 1.0));
    }

    /// A take or its transcription failed. Only watches that asked for
    /// events hear it; with none, the event goes nowhere.
    pub fn report_error(&self, message: &str) {
        let _ = self.events.send(WatchEvent::Error {
            message: message.to_string(),
        });
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

    /// The daemon quits.
    pub fn stopping(&self) -> bool {
        matches!(*self.phase.lock(), Phase::Stopping)
    }

    /// A take is in progress, whether or not its first frame arrived.
    pub fn in_take(&self) -> bool {
        matches!(
            *self.phase.lock(),
            Phase::Starting(_) | Phase::Recording(..)
        )
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

    /// The styles that a start can name: `verbatim` and the config's styles.
    pub fn set_styles(&self, names: Vec<StyleName>) {
        *self.styles.lock() = names;
    }

    pub fn knows_style(&self, name: StyleName) -> bool {
        self.styles.lock().contains(&name)
    }

    /// The styles that `hush rewrite` can use: polish on and a prompt.
    pub fn set_rewriters(&self, names: Vec<StyleName>) {
        *self.rewriters.lock() = names;
    }

    pub fn can_rewrite(&self, name: StyleName) -> bool {
        self.rewriters.lock().contains(&name)
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
            .with_style(self.take_style().map(|name| name.to_string()))
            .with_safe_mode(self.safe_mode.lock().clone())
            .with_start_delay(self.start_delay())
            .with_version(env!("CARGO_PKG_VERSION"))
            .with_overlay(*self.overlay.lock())
            .with_keep_next(self.keep_next_pending())
    }

    /// The style that the start of the take in progress asked for.
    fn take_style(&self) -> Option<StyleName> {
        match *self.phase.lock() {
            Phase::Starting(flags) | Phase::Recording(_, flags) => flags.style,
            _ => None,
        }
    }

    pub fn snapshot(&self) -> DaemonState {
        match *self.phase.lock() {
            Phase::Idle => DaemonState::Idle,
            Phase::Starting(_) => DaemonState::Starting,
            Phase::Recording(since, flags) => DaemonState::Recording {
                elapsed_ms: since.elapsed().as_millis() as u64,
                locked: flags.locked,
                command: flags.command,
                stops_in_ms: flags
                    .stops_at
                    .map(|at| at.saturating_duration_since(Instant::now()).as_millis() as u64),
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

/// Serve one client on a stream that is already connected, such as the
/// overlay's end of its socket pair.
pub async fn serve_stream(
    stream: std::os::unix::net::UnixStream,
    shared: &SharedState,
    control: &mpsc::Sender<SessionCommand>,
) {
    let stream = match UnixStream::from_std(stream) {
        Ok(stream) => stream,
        Err(e) => {
            debug!("The overlay's socket did not open: {}", e);
            return;
        },
    };
    if let Err(e) = handle_client(stream, shared, control).await {
        debug!("The overlay's socket closed: {:#}", e);
    }
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
        Ok(DaemonCommand::Watch { meter, events }) => {
            return watch(reader, writer, shared, meter, events).await
        },
        Ok(DaemonCommand::Stop { print: true }) => {
            stop_and_print(shared, control, PRINT_TIMEOUT).await
        },
        Ok(DaemonCommand::Transcribe { path }) => {
            transcribe_file(shared, control, path, FILE_TIMEOUT).await
        },
        Ok(DaemonCommand::Unload) => unload_model(shared, control, UNLOAD_TIMEOUT).await,
        Ok(DaemonCommand::Shortcuts { configure }) => {
            portal_shortcuts(shared.shortcuts(), shared.snapshot(), configure).await
        },
        Ok(command) => respond(command, shared, control),
        Err(e) => DaemonResponse::error(format!("Invalid command: {}", e)),
    };
    write_response(&mut writer, &response).await
}

/// The portal shortcuts, after the desktop's dialog when `configure` asks
/// for it. The dialog opens on its own, and a change comes back later as a
/// new binding.
async fn portal_shortcuts(
    handle: Option<ShortcutsHandle>,
    state: DaemonState,
    configure: bool,
) -> DaemonResponse {
    let Some(handle) = handle else {
        return DaemonResponse::error(
            "the daemon reads the hotkeys with evdev, not the GlobalShortcuts portal",
        );
    };
    if configure {
        if let Err(e) = handle.configure().await {
            return DaemonResponse::error(format!("cannot open the shortcut settings: {}", e));
        }
    }
    DaemonResponse::ok(state).with_shortcuts(handle.bindings())
}

async fn write_response(writer: &mut OwnedWriteHalf, response: &DaemonResponse) -> Result<()> {
    write_line(writer, response).await
}

async fn write_line(writer: &mut OwnedWriteHalf, line: &impl serde::Serialize) -> Result<()> {
    let mut payload = serde_json::to_string(line)?;
    payload.push('\n');
    writer
        .write_all(payload.as_bytes())
        .await
        .context("Failed to write IPC response")
}

/// Send the state now and after each change, until the client hangs up.
/// With `meter`, also each new microphone level; with `events`, each error.
async fn watch(
    mut reader: BufReader<OwnedReadHalf>,
    mut writer: OwnedWriteHalf,
    shared: &SharedState,
    meter: bool,
    events: bool,
) -> Result<()> {
    let mut changes = shared.subscribe();
    let mut levels = shared.level.subscribe();
    let mut errors = shared.events.subscribe();
    let mut scratch = [0u8; 64];
    write_response(&mut writer, &shared.status()).await?;
    loop {
        tokio::select! {
            changed = changes.changed() => {
                if changed.is_err() {
                    return Ok(());
                }
                write_response(&mut writer, &shared.status()).await?;
            },
            changed = levels.changed(), if meter => {
                if changed.is_err() {
                    return Ok(());
                }
                let level = *levels.borrow_and_update();
                write_line(&mut writer, &Level { level }).await?;
            },
            event = errors.recv(), if events => match event {
                Ok(event) => write_line(&mut writer, &event).await?,
                Err(broadcast::error::RecvError::Lagged(missed)) => {
                    debug!(missed, "A watch fell behind and missed events");
                },
                Err(broadcast::error::RecvError::Closed) => return Ok(()),
            },
            read = reader.read(&mut scratch) => {
                if matches!(read, Ok(0) | Err(_)) {
                    return Ok(());
                }
            },
        }
    }
}

/// How long an unload waits for the session to free the model.
const UNLOAD_TIMEOUT: Duration = Duration::from_secs(5);

/// Free the local model, and reply once the session says it is unloaded.
/// A take in progress, or a backend without a local model, refuses.
async fn unload_model(
    shared: &SharedState,
    control: &mpsc::Sender<SessionCommand>,
    timeout: Duration,
) -> DaemonResponse {
    match shared.model_state() {
        Some(ModelState::Remote) => {
            return DaemonResponse::error("The transcription backend has no local model to unload")
        },
        Some(ModelState::Unloaded) => return shared.status(),
        _ => {},
    }
    if !shared.is_idle() {
        return DaemonResponse::error("Hush is busy with a take; try again when it is idle");
    }
    if control.send(SessionCommand::UnloadModel).is_err() {
        return DaemonResponse::error("The daemon session is no longer running");
    }
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if shared.model_state() == Some(ModelState::Unloaded) {
            return shared.status();
        }
        if !shared.is_idle() {
            return DaemonResponse::error("A take started, so the model stays loaded");
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    DaemonResponse::error("The model did not unload in time")
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

/// The daemon knows no style of that name: the client's config can have a
/// style that the daemon did not load.
pub fn unknown_style(name: StyleName) -> String {
    format!(
        "The daemon knows no style '{}'. Add [styles.{}] to the config, then restart it with: \
         hush daemon restart",
        name, name
    )
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
    // The reply names the take's language and style, so a client can tell
    // this daemon from an older one that ignores `language` or `style`.
    let start = |language: Option<String>, translate: bool, style: Option<String>| {
        let once = match language.as_deref().map(Language::from_code).transpose() {
            Ok(once) => once,
            Err(e) => return DaemonResponse::error(e.to_string()),
        };
        let style = match style.as_deref().map(StyleName::from_name).transpose() {
            Ok(Some(name)) if !shared.knows_style(name) => {
                return DaemonResponse::error(unknown_style(name))
            },
            Ok(style) => style,
            Err(e) => return DaemonResponse::error(e.to_string()),
        };
        let reply = forward(
            SessionCommand::StartRecording(
                Trigger::Ipc,
                Once {
                    language: once,
                    translate,
                    style,
                },
            ),
            DaemonState::Starting,
        );
        if !reply.ok {
            return reply;
        }
        let language = shared.language_for(once).or(once);
        reply
            .with_language(language.map(|l| l.code().to_string()))
            .with_style(style.map(|name| name.to_string()))
            .with_translate(translate)
    };
    // A stop before the first audio frame discards the take.
    let after_stop = match shared.snapshot() {
        DaemonState::Starting => DaemonState::Idle,
        DaemonState::Recording { .. } => DaemonState::Transcribing,
        other => other,
    };
    match command {
        DaemonCommand::Status | DaemonCommand::Watch { .. } => shared.status(),
        DaemonCommand::Toggle {
            language,
            translate,
            style,
        } => {
            if shared.in_take() {
                forward(SessionCommand::StopRecording, after_stop)
            } else {
                start(language, translate, style)
            }
        },
        DaemonCommand::Start {
            language,
            translate,
            style,
        } => start(language, translate, style),
        DaemonCommand::Stop { .. } => forward(SessionCommand::StopRecording, after_stop),
        DaemonCommand::Cancel => forward(SessionCommand::CancelRecording, DaemonState::Idle),
        DaemonCommand::KeepNext => match shared.request_keep_next(Instant::now()) {
            Ok(()) => shared.status(),
            Err(reason) => DaemonResponse::error(reason),
        },
        DaemonCommand::Reload => forward(SessionCommand::LoadModel, shared.snapshot()),
        DaemonCommand::Quit => forward(SessionCommand::Quit, DaemonState::Stopping),
        DaemonCommand::PasteLast => {
            if shared.last_text().is_none() {
                DaemonResponse::error("Nothing to paste yet: dictate something first")
            } else {
                forward(SessionCommand::PasteLast, DaemonState::Inserting)
            }
        },
        DaemonCommand::Rewrite { style } => {
            let name = match StyleName::from_name(&style) {
                Ok(name) => name,
                Err(e) => return DaemonResponse::error(e.to_string()),
            };
            if !shared.knows_style(name) {
                DaemonResponse::error(unknown_style(name))
            } else if !shared.can_rewrite(name) {
                DaemonResponse::error(format!(
                    "The style {} cannot rewrite text: it needs polish = true and a prompt",
                    name
                ))
            } else if shared.in_take() {
                DaemonResponse::error("A recording is in progress. Rewrite after it ends.")
            } else {
                forward(SessionCommand::Rewrite(name), shared.snapshot())
                    .with_message(format!("Rewriting the selection in the {} style.", name))
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
        DaemonCommand::Shortcuts { .. } => DaemonResponse::error(
            "The shortcut reply waits for the portal; send it over the socket",
        ),
        DaemonCommand::Unload => {
            DaemonResponse::error("An unload waits for the session; send it over the socket")
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::client;
    use crate::ipc::protocol::WatchLine;

    #[tokio::test]
    async fn the_shortcuts_reply_needs_portal_mode() {
        let reply = portal_shortcuts(None, DaemonState::Idle, false).await;
        assert!(!reply.ok);
        assert!(reply.error.unwrap_or_default().contains("evdev"));
        let command: DaemonCommand =
            serde_json::from_str(r#"{"command":"shortcuts","configure":true}"#).unwrap();
        assert_eq!(command, DaemonCommand::Shortcuts { configure: true });
        assert_eq!(
            serde_json::to_string(&DaemonCommand::Shortcuts { configure: false }).unwrap(),
            r#"{"command":"shortcuts"}"#
        );
    }

    #[test]
    fn a_quitting_daemon_reports_stopping() {
        let shared = SharedState::new();
        assert!(!shared.stopping());
        shared.set_stopping();
        assert!(shared.stopping());
    }

    #[test]
    fn toggle_follows_the_take() {
        let shared = SharedState::new();
        let (tx, rx) = mpsc::channel();
        let toggle = || DaemonCommand::Toggle {
            language: None,
            translate: false,
            style: None,
        };
        assert_eq!(
            respond(toggle(), &shared, &tx).state,
            Some(DaemonState::Starting)
        );
        assert_eq!(
            rx.try_recv(),
            Ok(SessionCommand::StartRecording(
                Trigger::Ipc,
                Once::default()
            ))
        );

        shared.set_starting(false, None);
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
    fn status_carries_the_start_delay_of_recent_takes() {
        let shared = SharedState::new();
        let (tx, _rx) = mpsc::channel();
        assert_eq!(
            respond(DaemonCommand::Status, &shared, &tx).start_delay,
            None,
            "no take yet"
        );
        shared.record_start_delay(Duration::from_millis(820));
        shared.record_start_delay(Duration::from_millis(15));
        let delay = respond(DaemonCommand::Status, &shared, &tx)
            .start_delay
            .unwrap();
        assert_eq!((delay.last_ms, delay.worst_ms, delay.takes), (15, 820, 2));
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
        let response = respond(
            DaemonCommand::Start {
                language: None,
                translate: false,
                style: None,
            },
            &shared,
            &tx,
        );
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
            translate: false,
            style: None,
        };

        let reply = respond(start("de"), &shared, &tx);
        assert_eq!(reply.state, Some(DaemonState::Starting));
        assert_eq!(reply.language.as_deref(), Some("de"));
        assert_eq!(
            rx.try_recv(),
            Ok(SessionCommand::StartRecording(
                Trigger::Ipc,
                Once {
                    language: Some(lang("de")),
                    ..Once::default()
                }
            ))
        );

        let reply = respond(
            DaemonCommand::Start {
                language: None,
                translate: false,
                style: None,
            },
            &shared,
            &tx,
        );
        assert_eq!(reply.language.as_deref(), Some("en"), "the config language");
        shared.set_chosen_language(Some(lang("fr")));
        let reply = respond(
            DaemonCommand::Toggle {
                language: None,
                translate: false,
                style: None,
            },
            &shared,
            &tx,
        );
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
                translate: false,
                style: None,
            },
            &shared,
            &tx,
        );
        assert_eq!(reply.state, Some(DaemonState::Transcribing));
        assert_eq!(reply.language, None, "a toggle that stops starts no take");
        assert_eq!(rx.try_recv(), Ok(SessionCommand::StopRecording));
    }

    #[test]
    fn a_start_in_a_style_reaches_the_session_and_names_it() {
        let shared = SharedState::new();
        shared.set_config_language(lang("en"));
        let (tx, rx) = mpsc::channel();
        let start = |style: &str| DaemonCommand::Start {
            language: None,
            translate: false,
            style: Some(style.to_string()),
        };
        let verbatim = styles::VERBATIM;
        let casual = StyleName::parse("casual").unwrap();

        let reply = respond(start("verbatim"), &shared, &tx);
        assert_eq!(reply.state, Some(DaemonState::Starting));
        assert_eq!(reply.style.as_deref(), Some("verbatim"));
        assert_eq!(
            rx.try_recv(),
            Ok(SessionCommand::StartRecording(
                Trigger::Ipc,
                Once {
                    style: Some(verbatim),
                    ..Once::default()
                }
            ))
        );

        let reply = respond(start("casual"), &shared, &tx);
        assert!(!reply.ok, "the daemon has no casual style yet");
        assert!(reply.error.unwrap().contains("'casual'"));
        assert!(rx.try_recv().is_err(), "nothing starts");
        let reply = respond(start("Not A Name"), &shared, &tx);
        assert!(!reply.ok);
        assert!(rx.try_recv().is_err());

        shared.set_styles(vec![verbatim, casual]);
        let reply = respond(
            DaemonCommand::Toggle {
                language: Some("de".to_string()),
                translate: false,
                style: Some("casual".to_string()),
            },
            &shared,
            &tx,
        );
        assert_eq!(reply.style.as_deref(), Some("casual"));
        assert_eq!(reply.language.as_deref(), Some("de"));
        assert_eq!(
            rx.try_recv(),
            Ok(SessionCommand::StartRecording(
                Trigger::Ipc,
                Once {
                    language: Some(lang("de")),
                    translate: false,
                    style: Some(casual),
                }
            ))
        );

        let reply = respond(
            DaemonCommand::Start {
                language: None,
                translate: false,
                style: None,
            },
            &shared,
            &tx,
        );
        assert_eq!(reply.style, None, "a start without a style names none");
    }

    #[test]
    fn a_rewrite_reaches_the_session_only_with_a_style_that_can_rewrite() {
        let shared = SharedState::new();
        let (tx, rx) = mpsc::channel();
        let formal = StyleName::parse("formal").unwrap();
        let raw = StyleName::parse("raw").unwrap();
        shared.set_styles(vec![styles::VERBATIM, formal, raw]);
        shared.set_rewriters(vec![formal]);
        let rewrite = |style: &str| DaemonCommand::Rewrite {
            style: style.to_string(),
        };

        let reply = respond(rewrite("formal"), &shared, &tx);
        assert!(reply.ok);
        assert_eq!(reply.state, Some(DaemonState::Idle));
        assert!(reply.message.unwrap().contains("formal"));
        assert_eq!(rx.try_recv(), Ok(SessionCommand::Rewrite(formal)));

        for (style, needle) in [
            ("verbatim", "cannot rewrite"),
            ("raw", "cannot rewrite"),
            ("nosuch", "knows no style 'nosuch'"),
            ("No Such", "Invalid style name"),
        ] {
            let reply = respond(rewrite(style), &shared, &tx);
            assert!(!reply.ok, "{style}");
            let error = reply.error.unwrap();
            assert!(error.contains(needle), "{style}: {error}");
        }
        assert!(rx.try_recv().is_err(), "nothing reaches the session");

        shared.set_recording(Instant::now());
        let reply = respond(rewrite("formal"), &shared, &tx);
        assert!(!reply.ok, "no rewrite while a take records");
        assert!(rx.try_recv().is_err());
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
        let first = next_state(&mut watcher).await;
        assert_eq!(first.state, Some(DaemonState::Idle));
        assert_eq!(first.version.as_deref(), Some(env!("CARGO_PKG_VERSION")));

        shared.set_starting(false, None);
        let next = next_state(&mut watcher).await;
        assert_eq!(next.state, Some(DaemonState::Starting));
        drop(guard);
    }

    async fn next_state(watcher: &mut client::Watcher) -> DaemonResponse {
        match watcher.next().await.unwrap().unwrap() {
            WatchLine::State(response) => *response,
            other => panic!("expected a state line, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn a_watch_gets_the_level_and_errors_only_when_it_asks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hush.sock");
        let shared = Arc::new(SharedState::new());
        let (tx, _rx) = mpsc::channel();
        let guard = start(&path, Arc::clone(&shared), tx).await.unwrap();

        let mut plain = client::Watcher::connect(&path).await.unwrap();
        let mut full = client::Watcher::connect_with(&path, true, true)
            .await
            .unwrap();
        next_state(&mut plain).await;
        next_state(&mut full).await;

        shared.publish_level(0.25);
        assert_eq!(
            full.next().await.unwrap().unwrap(),
            WatchLine::Level(Level { level: 0.25 })
        );
        shared.report_error("The microphone did not start");
        assert_eq!(
            full.next().await.unwrap().unwrap(),
            WatchLine::Event(WatchEvent::Error {
                message: "The microphone did not start".into()
            })
        );

        shared.set_starting(false, None);
        assert_eq!(
            next_state(&mut plain).await.state,
            Some(DaemonState::Starting),
            "a plain watch got neither the level nor the error"
        );
        drop(guard);
    }

    #[tokio::test]
    async fn unload_waits_for_the_session_and_refuses_a_busy_daemon() {
        let shared = Arc::new(SharedState::new());
        let (tx, rx) = mpsc::channel();
        shared.set_model_state(ModelState::Loaded);
        shared.set_starting(false, None);
        let busy = unload_model(&shared, &tx, Duration::from_millis(50)).await;
        assert!(!busy.ok, "a take in progress keeps the model");
        assert!(rx.try_recv().is_err());

        shared.set_idle();
        let session = Arc::clone(&shared);
        let answer = std::thread::spawn(move || {
            assert_eq!(rx.recv().unwrap(), SessionCommand::UnloadModel);
            session.set_model_state(ModelState::Unloaded);
        });
        let reply = unload_model(&shared, &tx, Duration::from_secs(2)).await;
        answer.join().unwrap();
        assert!(reply.ok);
        assert_eq!(reply.model_state, Some(ModelState::Unloaded));

        shared.set_model_state(ModelState::Remote);
        let cloud = unload_model(&shared, &tx, Duration::from_millis(50)).await;
        assert!(!cloud.ok, "a cloud backend has no local model");
    }

    #[test]
    fn keep_next_holds_for_one_take_within_its_time() {
        let shared = SharedState::new();
        let now = Instant::now();
        assert!(!shared.take_keep_next(now), "no request");
        shared.request_keep_next(now).unwrap();
        assert!(shared.status().keep_next);
        assert!(shared.take_keep_next(now + Duration::from_secs(5)));
        assert!(!shared.take_keep_next(now), "one take only");
        assert!(!shared.status().keep_next);

        shared.request_keep_next(now).unwrap();
        assert!(
            !shared.take_keep_next(now + KEEP_NEXT_FOR + Duration::from_secs(1)),
            "a request expires"
        );

        shared.set_safe_mode(Some("the configuration does not load".into()));
        assert!(shared.request_keep_next(now).is_err());
        assert!(!shared.status().keep_next);
    }

    #[test]
    fn a_take_keeps_its_lock_command_and_style_until_it_ends() {
        let shared = SharedState::new();
        let style = StyleName::parse("casual").unwrap();
        shared.set_starting(true, Some(style));
        shared.set_locked();
        shared.set_recording(Instant::now());
        let status = shared.status();
        assert!(matches!(
            status.state,
            Some(DaemonState::Recording {
                locked: true,
                command: true,
                stops_in_ms: None,
                ..
            })
        ));
        assert_eq!(status.style.as_deref(), Some("casual"));

        shared.set_stops_at(Instant::now() + Duration::from_secs(30));
        let DaemonState::Recording { stops_in_ms, .. } = shared.snapshot() else {
            panic!("the take still records");
        };
        assert!(stops_in_ms.is_some_and(|ms| (29_000..=30_000).contains(&ms)));

        shared.set_idle();
        shared.set_locked();
        assert_eq!(
            shared.snapshot(),
            DaemonState::Idle,
            "a lock outside a take does nothing"
        );
        assert_eq!(shared.status().style, None);
        shared.set_starting(false, None);
        shared.set_recording(Instant::now());
        assert!(
            matches!(
                shared.snapshot(),
                DaemonState::Recording {
                    locked: false,
                    command: false,
                    ..
                }
            ),
            "the next take starts without the old flags"
        );
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

        let started = client::send_to(
            &path,
            DaemonCommand::Start {
                language: None,
                translate: false,
                style: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(started.state, Some(DaemonState::Starting));
        assert_eq!(
            rx.recv().unwrap(),
            SessionCommand::StartRecording(Trigger::Ipc, Once::default())
        );

        drop(guard);
        assert!(!path.exists());
        assert!(client::send_to(&path, DaemonCommand::Status).await.is_err());
    }
}
