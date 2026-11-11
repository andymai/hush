//! The Hush window: an overview of what still needs setting up, then one page
//! for each group of settings.
//!
//! It runs in its own process (`hush settings`), which the tray and the
//! desktop entry launch, so the daemon never has to own a window.

mod autosave;
mod capture;
mod chrome;
mod icons;
mod jump;
mod meter;
mod pages;
mod palette;
mod scheme;
mod screenshot;
mod theme;
mod widgets;

pub use pages::speech::size_key;

use crate::audio::take::StartDelay;
use crate::config::settings::{Appearance, TranscriptionBackend, TranscriptionConfig};
use crate::config::Config;
use crate::hotkey::keymap::ModifierKeys;
use crate::hotkey::Role;
use crate::ipc::client::{self, Probe};
use crate::ipc::DaemonState;
use crate::permissions::PermissionStatus;
use crate::text::layout::Layout;
use crate::text_processing::DomainVocabulary;
use crate::transcription::models::{ModelManager, ModelSize};
use anyhow::{Context, Result};
use autosave::{
    close_step, daemon_reads_a_change, CloseFlow, CloseState, CloseStep, Daemon, Debounce,
    Feedback, Files, Pending, Undo,
};
use capture::{Capture, Outcome};
use meter::Meter;
use pages::history::HistoryView;
use pages::hotkeys::KeyTarget;
use pages::speech::WakeView;
use pages::text::{StyleDraft, TermEdit};
use pages::Page;
use parking_lot::Mutex;
use scheme::DesktopScheme;
use screenshot::Screenshot;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::warn;

const POLL_INTERVAL: Duration = Duration::from_millis(700);
const FAST_POLL: Duration = Duration::from_millis(150);
const METER_REPAINT: Duration = Duration::from_millis(50);
const TOAST_DURATION: Duration = Duration::from_secs(4);
const STATUS_TIMEOUT: Duration = Duration::from_millis(300);
/// How long a restarted daemon gets to load its model and answer again.
const RESTART_TIMEOUT: Duration = Duration::from_secs(40);
/// How long the footer says Saved.
const SAVED_DURATION: Duration = Duration::from_secs(2);

pub const MODEL_ORDER: [ModelSize; 8] = [
    ModelSize::Tiny,
    ModelSize::Base,
    ModelSize::Small,
    ModelSize::Medium,
    ModelSize::Large,
    ModelSize::LargeV2,
    ModelSize::LargeV3,
    ModelSize::ParakeetV3,
];

/// A model download in flight, shared with the task doing the work.
#[derive(Default)]
pub struct Download {
    pub size: Option<ModelSize>,
    /// The download fetches the speech model `size`, not only voice detection
    pub fetches_model: bool,
    pub done: u64,
    pub total: u64,
    pub finished: Option<Result<(), String>>,
}

/// What the window shows about the machine, refreshed on a timer.
pub struct Machine {
    pub permissions: PermissionStatus,
    pub daemon: Option<DaemonState>,
    /// The status answer the state came from: a daemon that gave none in
    /// time shows as not running, but may be busy
    pub probe: Probe,
    /// Why the daemon runs in safe mode, when it does
    pub safe_mode: Option<String>,
    /// How long recent takes waited for the first audio, from the daemon
    pub start_delay: Option<StartDelay>,
    /// The configuration file loads now
    pub config_loads: bool,
    pub installed_models: Vec<ModelSize>,
    /// The Silero voice activity detection model is downloaded.
    pub vad_installed: bool,
    pub autostart: bool,
    /// The transcription key variables that hold a key: the hosted
    /// services' and the server's.
    pub cloud_keys: Vec<String>,
}

impl Machine {
    fn read(server_key_env: &str) -> Self {
        let manager = ModelManager::new(crate::config::paths::models_dir()).ok();
        let vad_installed = manager
            .as_ref()
            .is_some_and(|manager| manager.vad_model_path().is_some());
        let installed_models = manager
            .map(|manager| {
                MODEL_ORDER
                    .iter()
                    .filter(|size| manager.get_model_path(size).is_some())
                    .copied()
                    .collect()
            })
            .unwrap_or_default();
        let (probe, reply) = client::reply_blocking(STATUS_TIMEOUT);
        let start_delay = reply.as_ref().and_then(|reply| reply.start_delay);
        Self {
            permissions: crate::permissions::status(),
            daemon: match probe {
                Probe::State(state) => Some(state),
                Probe::NotRunning | Probe::NoAnswer => None,
            },
            probe,
            safe_mode: reply.and_then(|reply| reply.safe_mode),
            start_delay,
            config_loads: Config::load().is_ok(),
            installed_models,
            vad_installed,
            autostart: crate::desktop::autostart_installed(),
            cloud_keys: TranscriptionBackend::ALL
                .iter()
                .filter_map(|backend| backend.key_var())
                .chain(crate::config::server::key_var(server_key_env))
                .filter(|var| crate::config::keys::api_key(var).is_some())
                .map(String::from)
                .collect(),
        }
    }

    /// Whether the chosen backend has its key, or needs none.
    pub fn has_key(&self, transcription: &TranscriptionConfig) -> bool {
        transcription
            .key_var()
            .is_none_or(|var| self.cloud_keys.iter().any(|key| key == var))
    }

    /// Keyboard access, plus a downloaded model, the cloud service's key, or
    /// a server that is set up.
    pub fn ready(&self, transcription: &TranscriptionConfig) -> bool {
        let speech = match transcription.backend {
            TranscriptionBackend::Local => !self.installed_models.is_empty(),
            TranscriptionBackend::OpenAiCompatible => {
                transcription.validate_server().is_ok() && self.has_key(transcription)
            },
            TranscriptionBackend::Groq | TranscriptionBackend::OpenAi => {
                self.has_key(transcription)
            },
        };
        self.permissions.ready() && speech
    }

    pub fn running(&self) -> bool {
        self.daemon.is_some()
    }

    pub fn busy(&self) -> bool {
        matches!(
            self.daemon,
            Some(
                DaemonState::Starting
                    | DaemonState::Recording { .. }
                    | DaemonState::Transcribing
                    | DaemonState::Inserting
            )
        )
    }
}

/// A daemon restart after a change, watched until the daemon answers again.
struct Restart {
    since: Instant,
    went_down: bool,
}

/// What a restart attempt found when it asked the daemon once more.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Attempt {
    /// `hush daemon restart` runs
    Issued,
    /// A take began after the last poll, so the restart waits
    Busy,
    /// The daemon gave no answer in time; it may be busy
    NoAnswer,
    /// The configuration file does not load now, so a restarted daemon would
    /// run in safe mode
    HeldByFile(String),
    /// The daemon stopped, so it reads the file when it starts
    NotRunning,
    Failed(String),
}

/// The machine is read on a thread of its own, because the read opens every
/// input device, runs `id`, and waits on the daemon's socket: none of that
/// belongs on the thread that draws the window.
#[derive(Default)]
struct Poll {
    latest: Arc<Mutex<Option<Machine>>>,
    in_flight: Arc<AtomicBool>,
}

pub struct Gui {
    pub config: Config,
    pub saved: Config,
    pub vocabulary: DomainVocabulary,
    pub saved_vocabulary: DomainVocabulary,
    pub page: Page,
    pub machine: Machine,
    /// Every model the catalogue knows, with its download size in bytes.
    pub catalogue: Vec<(ModelSize, u64)>,
    pub recommended: ModelSize,
    pub download: Arc<Mutex<Download>>,
    pub runtime: tokio::runtime::Runtime,
    pub toast: Option<(String, Instant)>,
    /// The hotkey field waiting for a key press, and the reader collecting it.
    pub capture: Option<(KeyTarget, Capture)>,
    /// The language code for the next language key; empty until one is picked.
    pub new_language: String,
    /// The style name for the next style key; empty until one is picked.
    pub new_style: String,
    /// The style name for the next rewrite key; empty until one is picked.
    pub new_rewrite: String,
    /// The hotkey field showing a text box instead of a capture button
    pub editing_key: Option<Role>,
    pub focus_editor: bool,
    /// The keyboard layout when the window opened.
    pub layout: Option<Layout>,
    /// What the modifier keys type when the window opened.
    pub modifier_keys: ModifierKeys,
    pub meter: Option<Meter>,
    pub meter_error: Option<String>,
    /// The microphone the meter was last asked to open, so a failure is
    /// reported once rather than retried every frame.
    pub meter_wanted: Option<String>,
    pub microphones: Vec<String>,
    pub level_shown: f32,
    pub new_term: String,
    pub editing_term: Option<TermEdit>,
    /// Text that narrows the vocabulary list on the Text page
    pub term_filter: String,
    /// The form on the Text page that adds or changes a style
    pub style_draft: StyleDraft,
    /// The plain-text word list that the Text page imports and exports
    pub word_list: String,
    pub new_uinput_app: String,
    pub new_clipboard_app: String,
    pub history: HistoryView,
    /// Whether the microphone sleeps between takes, and Keep it awake
    pub wake: WakeView,
    pub try_text: String,
    /// The desktop's light or dark preference, for the System appearance
    desktop: DesktopScheme,
    /// The appearance that `HUSH_SETTINGS_APPEARANCE` forces for a
    /// screenshot
    appearance_override: Option<Appearance>,
    screenshot: Option<Screenshot>,
    /// The files behind the window, as it last read or wrote them
    pub files: Files,
    /// The last save of each file, or why it cannot save
    config_feedback: Option<Feedback>,
    vocabulary_feedback: Option<Feedback>,
    /// Settings that failed to save, so they are not tried again each frame
    rejected: Option<Config>,
    rejected_vocabulary: Option<DomainVocabulary>,
    /// Choices that wait for other settings before they can save
    pub pending: Pending,
    /// The last removal, which Undo in the footer puts back
    pub undo: Option<Undo>,
    /// Reset everything waits for a second click
    pub confirm_reset: bool,
    /// The page of the last frame, to disarm a second click on a change
    last_page: Page,
    /// The command palette that Ctrl+K opens
    palette: palette::Palette,
    /// The next frame shows the page from its top
    scroll_top: bool,
    debounce: Debounce,
    attempt: Arc<Mutex<Option<Attempt>>>,
    attempting: bool,
    close: CloseFlow,
    /// Why the last restart try found the configuration file broken
    held_by_file: Option<String>,
    /// Restart tries in a row that the file or a silent daemon held back
    held_tries: u32,
    /// The next save comes from the window itself, such as a model that
    /// finished its download, not from the user
    window_change: bool,
    /// Replace with defaults, which deletes the kept audio, waits for a
    /// second click
    pub confirm_replace: bool,
    restart: Option<Restart>,
    poll: Poll,
    last_poll: Instant,
}

impl Gui {
    fn new() -> Result<Self> {
        let mut files = Files::before_reading();
        let (config, config_error) = match Config::load() {
            Ok(config) => (config, None),
            Err(e) => {
                warn!("Showing the built-in defaults: {:#}", e);
                let defaults =
                    Config::defaults().context("The built-in defaults are unreadable")?;
                (defaults, Some(format!("{:#}", e)))
            },
        };
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .context("Failed to start the background runtime")?;
        let machine = Machine::read(&config.transcription.server_key_env);
        let (vocabulary, vocabulary_error) = match load_vocabulary() {
            Ok(vocabulary) => (vocabulary, None),
            Err(e) => {
                warn!("Showing an empty vocabulary: {:#}", e);
                (DomainVocabulary::default(), Some(format!("{:#}", e)))
            },
        };
        files.config_error = config_error;
        files.vocabulary_error = vocabulary_error;
        let manager = ModelManager::new(crate::config::paths::models_dir()).ok();
        let catalogue = MODEL_ORDER
            .iter()
            .filter_map(|size| {
                let info = manager.as_ref()?.get_model_info(size)?;
                Some((*size, info.expected_size))
            })
            .collect();
        let mut gui = Self::with(config, vocabulary, files, machine, runtime);
        gui.catalogue = catalogue;
        gui.recommended = crate::transcription::models::recommended_model();
        gui.layout = crate::text::layout::current();
        gui.modifier_keys = ModifierKeys::session();
        gui.page = Page::from_env().unwrap_or(Page::Overview);
        gui.last_page = gui.page;
        gui.screenshot = Screenshot::from_env();
        if gui.screenshot.is_some() {
            gui.appearance_override = theme::appearance_from_env();
            if let Some(query) = Screenshot::palette_query() {
                gui.palette.open_with(&query);
            }
        } else {
            gui.microphones = meter::input_devices();
        }
        Ok(gui)
    }

    /// The window's state for these settings, files, and machine, with
    /// nothing read from the desktop: no model catalogue, no keyboard
    /// layout, and no microphones.
    fn with(
        config: Config,
        vocabulary: DomainVocabulary,
        files: Files,
        machine: Machine,
        runtime: tokio::runtime::Runtime,
    ) -> Self {
        Self {
            page: Page::Overview,
            saved: config.clone(),
            config,
            saved_vocabulary: vocabulary.clone(),
            vocabulary,
            machine,
            catalogue: Vec::new(),
            recommended: ModelSize::Base,
            download: Arc::new(Mutex::new(Download::default())),
            runtime,
            toast: None,
            capture: None,
            new_language: String::new(),
            new_style: String::new(),
            new_rewrite: String::new(),
            editing_key: None,
            focus_editor: false,
            layout: None,
            modifier_keys: ModifierKeys::physical(),
            meter: None,
            meter_error: None,
            meter_wanted: None,
            microphones: Vec::new(),
            level_shown: 0.0,
            new_term: String::new(),
            editing_term: None,
            term_filter: String::new(),
            style_draft: StyleDraft::default(),
            word_list: pages::text::DEFAULT_WORD_LIST.to_string(),
            new_uinput_app: String::new(),
            new_clipboard_app: String::new(),
            history: HistoryView::default(),
            wake: WakeView::default(),
            try_text: String::new(),
            desktop: DesktopScheme::default(),
            appearance_override: None,
            screenshot: None,
            files,
            config_feedback: None,
            vocabulary_feedback: None,
            rejected: None,
            rejected_vocabulary: None,
            pending: Pending::default(),
            undo: None,
            confirm_reset: false,
            last_page: Page::Overview,
            palette: palette::Palette::default(),
            scroll_top: false,
            debounce: Debounce::default(),
            attempt: Arc::new(Mutex::new(None)),
            attempting: false,
            close: CloseFlow::default(),
            held_by_file: None,
            held_tries: 0,
            window_change: false,
            confirm_replace: false,
            restart: None,
            poll: Poll::default(),
            last_poll: Instant::now(),
        }
    }

    pub fn say(&mut self, message: impl Into<String>) {
        self.toast = Some((message.into(), Instant::now()));
    }

    /// Open the command palette. A key field that listens stops, because it
    /// reads /dev/input whatever has the focus, and would take the keys
    /// typed into the palette.
    pub fn open_palette(&mut self, ctx: &egui::Context) {
        self.capture = None;
        self.palette.open(ctx);
    }

    /// Open `page`, and drop the key field that listens and the questions
    /// of the page before.
    pub fn go_to(&mut self, page: Page) {
        self.capture = None;
        if self.page != page {
            self.page = page;
            self.last_page = page;
            self.disarm();
        }
    }

    /// Drop every question that waits for a second click.
    fn disarm(&mut self) {
        self.confirm_reset = false;
        self.confirm_replace = false;
        self.history.disarm();
        self.wake.confirm = None;
    }

    /// Put a change into the window's settings, which save at the end of the
    /// frame. A change that would stop the settings from saving is refused
    /// with the reason, and the settings keep their value.
    pub fn try_set(&mut self, change: impl FnOnce(&mut Config)) -> Result<(), String> {
        self.config = checked_change(&self.config, change)?;
        Ok(())
    }

    /// Make a removal that Undo in the footer can put back. `removal` says
    /// whether it removed something.
    pub fn remove(&mut self, label: impl Into<String>, removal: impl FnOnce(&mut Gui) -> bool) {
        let before = (self.config.clone(), self.vocabulary.clone());
        if removal(self) {
            let after = (self.config.clone(), self.vocabulary.clone());
            self.undo = Some(Undo::new(label.into(), before, after, Instant::now()));
        }
    }

    /// Put back what the last removal took, on top of the settings as they
    /// are now, so a change merged from outside since the removal stays. An
    /// undo never lowers the kept audio days, because that deletes audio.
    pub fn undo(&mut self) {
        let Some(undo) = self.undo.take() else {
            return;
        };
        let now = (self.config.clone(), self.vocabulary.clone());
        match undo.restore(&now) {
            Ok((mut config, vocabulary)) => {
                let days = self.config.history.keep_audio_days;
                if config.history.keep_audio_days < days {
                    config.history.keep_audio_days = days;
                    self.say("Undo kept the audio days. Lower them on the History page.");
                }
                self.config = config;
                self.vocabulary = vocabulary;
            },
            Err(e) => self.say(format!("Could not undo: {:#}", e)),
        }
    }

    /// The vocabulary file changed outside the window's own writes: read it,
    /// and keep the window's unsaved changes on top.
    pub fn vocabulary_changed_on_disk(&mut self) {
        match self.files.reload_vocabulary() {
            Ok(theirs) => {
                let merged = autosave::rebase(&self.saved_vocabulary, &self.vocabulary, &theirs)
                    .unwrap_or_else(|_| theirs.clone());
                self.saved_vocabulary = theirs;
                self.vocabulary = merged;
            },
            Err(e) => warn!("Could not read the vocabulary again: {:#}", e),
        }
    }

    /// Put each unfinished choice into the settings once they can take it.
    fn settle_pending(&mut self) {
        if let Some(backend) = self.pending.backend {
            self.pending.backend = choose(&mut self.config, backend, |config, backend| {
                config.transcription.backend = backend
            });
        }
        if let Some(provider) = self.pending.provider {
            self.pending.provider = choose(&mut self.config, provider, |config, provider| {
                config.llm.provider = provider
            });
        }
    }

    /// Write what changed: the configuration, the vocabulary, or both. A
    /// change that the daemon reads restarts it after the changes stop.
    fn persist(&mut self, now: Instant) {
        if self.screenshot_mode() {
            return;
        }
        self.settle_pending();
        if self.config == self.saved {
            self.rejected = None;
            if matches!(self.config_feedback, Some(Feedback::Failed(_))) {
                self.config_feedback = None;
            }
        } else if self.files.config_error.is_some() {
            self.config_feedback = Some(Feedback::Failed(
                "Not saved: the configuration file does not load. Replace it at the top of \
                 the page, or fix the file and open the window again."
                    .to_string(),
            ));
        } else if self.rejected.as_ref() != Some(&self.config) {
            self.save_config(now);
        }
        if self.vocabulary == self.saved_vocabulary {
            self.rejected_vocabulary = None;
            if matches!(self.vocabulary_feedback, Some(Feedback::Failed(_))) {
                self.vocabulary_feedback = None;
            }
        } else if self.files.vocabulary_error.is_some() {
            self.vocabulary_feedback = Some(Feedback::Failed(
                "Not saved: the vocabulary file does not load. Replace it at the top of \
                 the page, or fix the file and open the window again."
                    .to_string(),
            ));
        } else if self.rejected_vocabulary.as_ref() != Some(&self.vocabulary) {
            self.save_vocabulary(now);
        }
        if let Some(undo) = self.undo.as_mut() {
            if !undo.still(&self.config, &self.vocabulary, now) {
                self.undo = None;
            }
        }
        self.window_change = false;
    }

    fn save_config(&mut self, now: Instant) {
        let written = self.files.write_config(&self.saved, &self.config);
        let (base, merged) = match written {
            Ok(written) => written,
            Err(e) => {
                self.rejected = Some(self.config.clone());
                self.config_feedback = Some(Feedback::Failed(format!("Not saved: {:#}", e)));
                self.save_failed();
                return;
            },
        };
        let keep_days = merged.history.keep_audio_days;
        if keep_days != base.history.keep_audio_days {
            if let Err(e) = crate::audio::store::sweep_now(keep_days) {
                warn!("Could not sweep the kept audio: {:#}", e);
            }
            self.history.reload();
        }
        if daemon_reads_a_change(&base, &merged) {
            self.debounce.change(now);
        }
        self.config = merged.clone();
        self.saved = merged;
        self.rejected = None;
        self.user_changed_something();
        self.config_feedback = Some(Feedback::Saved(now));
    }

    fn save_vocabulary(&mut self, now: Instant) {
        match self
            .files
            .write_vocabulary(&self.saved_vocabulary, &self.vocabulary)
        {
            Ok(merged) => {
                self.vocabulary = merged.clone();
                self.saved_vocabulary = merged;
                self.rejected_vocabulary = None;
                self.user_changed_something();
                self.vocabulary_feedback = Some(Feedback::Saved(now));
                self.debounce.change(now);
            },
            Err(e) => {
                self.rejected_vocabulary = Some(self.vocabulary.clone());
                self.vocabulary_feedback =
                    Some(Feedback::Failed(format!("Vocabulary not saved: {:#}", e)));
                self.save_failed();
            },
        }
    }

    /// Keep a copy of the file that does not load, then write the window's
    /// settings over it.
    pub fn replace_config(&mut self) {
        let mut config = self.config.clone();
        if let Some(history) = self.files.history_in_file() {
            config.history = history;
        }
        match self.files.replace_config(&config) {
            Ok(backup) => {
                self.confirm_replace = false;
                self.held_by_file = None;
                // A daemon that started on the broken file runs in safe mode.
                self.debounce.change(Instant::now());
                self.config = config.clone();
                self.saved = config;
                self.rejected = None;
                self.config_feedback = Some(Feedback::Saved(Instant::now()));
                match backup {
                    Some(backup) => self.say(format!("Kept the old file as {}.", backup.display())),
                    None => self.say("Wrote the configuration."),
                }
            },
            Err(e) => self.say(format!("Could not replace the file: {:#}", e)),
        }
    }

    /// [`Gui::replace_config`] for the vocabulary.
    pub fn replace_vocabulary(&mut self) {
        match self.files.replace_vocabulary(&self.vocabulary) {
            Ok(backup) => {
                self.saved_vocabulary = self.vocabulary.clone();
                self.rejected_vocabulary = None;
                self.vocabulary_feedback = Some(Feedback::Saved(Instant::now()));
                match backup {
                    Some(backup) => self.say(format!("Kept the old file as {}.", backup.display())),
                    None => self.say("Wrote the vocabulary."),
                }
            },
            Err(e) => self.say(format!("Could not replace the file: {:#}", e)),
        }
    }

    /// What a close would lose, when anything.
    fn unsaved(&self) -> Option<&'static str> {
        if self.config != self.saved || self.vocabulary != self.saved_vocabulary {
            Some("Some settings did not save")
        } else if self.pending.backend.is_some() || self.pending.provider.is_some() {
            Some("A choice waits for other settings")
        } else {
            None
        }
    }

    /// Before the pages draw: a close request commits the focused field
    /// first, and asks once when something would be lost.
    fn intercept_close(&mut self, ctx: &egui::Context) {
        if !ctx.input(|input| input.viewport().close_requested()) {
            return;
        }
        self.palette.dismiss(ctx);
        let focused = ctx.memory(|memory| memory.focused());
        let step = close_step(CloseState {
            focused: focused.is_some(),
            closing: self.close.closing,
            warned: self.close.warned,
            unsaved: self.unsaved().is_some(),
            restart_owed: self.restart_owed(),
        });
        match step {
            CloseStep::CommitField => {
                if let Some(focused) = focused {
                    ctx.memory_mut(|memory| memory.surrender_focus(focused));
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.close.closing = true;
                self.close.commit_frame = true;
            },
            CloseStep::Warn => {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.close.warned = true;
                let reason = self.unsaved().unwrap_or("Something did not save");
                self.say(format!("{}. Close again to discard it.", reason));
            },
            CloseStep::WaitForRestart => {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.close.closing = true;
            },
            // A second close while a take keeps the restart waiting closes
            // at once; the daemon reads the change when it next starts.
            CloseStep::Close => {},
        }
    }

    /// A new change after a close request: the window stays open, and the
    /// next close asks again when something would be lost.
    fn user_changed_something(&mut self) {
        self.close.changed(!self.window_change);
    }

    fn save_failed(&mut self) {
        self.close.failed();
    }

    /// After the pages drew: a close that committed the focused field stops
    /// when the field did not save, or when something else would be lost.
    fn closing_after_pages(&mut self, rejected: bool) {
        if !std::mem::take(&mut self.close.commit_frame) || !self.close.closing {
            return;
        }
        if rejected {
            self.close.closing = false;
            self.close.warned = true;
            self.say("A field did not save. Close again to discard it.");
        } else if !self.close.warned {
            if let Some(reason) = self.unsaved() {
                self.close.closing = false;
                self.close.warned = true;
                self.say(format!("{}. Close again to discard it.", reason));
            }
        }
    }

    /// Whether the window may close now that it wants to.
    fn may_close(&self) -> bool {
        !self.restart_owed() && (self.close.warned || self.unsaved().is_none())
    }

    /// Restart the daemon once the changes stop and it is idle, and close
    /// the window once a restart that it owes is under way.
    fn drive_restart(&mut self, ctx: &egui::Context, now: Instant) {
        let attempt = self.attempt.lock().take();
        if let Some(attempt) = attempt {
            self.attempting = false;
            match attempt {
                Attempt::Issued => {
                    self.held_tries = 0;
                    self.held_by_file = None;
                    self.restart = Some(Restart {
                        since: now,
                        went_down: false,
                    });
                },
                Attempt::Busy => {
                    self.held_tries = 0;
                    self.debounce.change(now);
                    self.request_refresh(ctx);
                },
                Attempt::NoAnswer => self.hold_restart(now),
                Attempt::HeldByFile(reason) => {
                    self.held_by_file = Some(reason);
                    self.hold_restart(now);
                },
                Attempt::NotRunning => self.held_tries = 0,
                Attempt::Failed(e) => self.say(format!("Could not restart Hush: {}", e)),
            }
        }
        if self.close.closing
            && autosave::hurry_on_close(self.held_by_file.is_some(), self.held_tries)
        {
            self.debounce.hurry(now);
        }
        let daemon = Daemon::of(
            self.machine.probe,
            self.restart.is_some() || self.attempting,
            self.files.config_error.is_none(),
        );
        if self.debounce.fire(now, daemon) {
            self.attempt_restart(ctx);
        }
        if self.close.closing && self.may_close() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if let Some(wait) = self.debounce.wait(now, self.restart_poll()) {
            ctx.request_repaint_after(wait.max(Duration::from_millis(20)));
        }
    }

    /// Try the held restart again, at the normal poll rate after a few tries.
    fn hold_restart(&mut self, now: Instant) {
        self.held_tries = self.held_tries.saturating_add(1);
        let delay =
            autosave::retry_delay(self.held_tries, autosave::RESTART_DEBOUNCE, POLL_INTERVAL);
        self.debounce.change_in(now, delay);
    }

    /// The redraw and poll rate while a restart waits.
    fn restart_poll(&self) -> Duration {
        autosave::retry_delay(self.held_tries, FAST_POLL, POLL_INTERVAL)
    }

    /// A change that the daemon reads is saved, but the daemon does not run
    /// with it yet.
    fn restart_owed(&self) -> bool {
        autosave::restart_owed(
            self.debounce.pending(),
            self.files.config_error.is_some() || self.held_by_file.is_some(),
            self.attempting,
        )
    }

    /// A change waits for a restart that the broken configuration file holds
    /// back.
    pub fn restart_waits_for_file(&self) -> bool {
        self.debounce.pending()
            && (self.files.config_error.is_some() || self.held_by_file.is_some())
    }

    /// Ask the daemon once more on a thread, so a take that began after the
    /// last poll is not cut, then restart it. A daemon that does not answer
    /// in time may be busy, so the restart waits.
    fn attempt_restart(&mut self, ctx: &egui::Context) {
        self.attempting = true;
        let slot = Arc::clone(&self.attempt);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let loads = Config::load().map(|_| ());
            let attempt = match client::probe_blocking(STATUS_TIMEOUT) {
                Probe::NotRunning => Attempt::NotRunning,
                Probe::State(DaemonState::Idle) if loads.is_err() => {
                    let reason = loads.err().map(|e| format!("{:#}", e)).unwrap_or_default();
                    Attempt::HeldByFile(reason)
                },
                Probe::State(DaemonState::Idle) => {
                    let binary = std::env::current_exe().unwrap_or_else(|_| "hush".into());
                    match std::process::Command::new(binary)
                        .args(["daemon", "restart"])
                        .spawn()
                    {
                        Ok(_) => Attempt::Issued,
                        Err(e) => Attempt::Failed(e.to_string()),
                    }
                },
                Probe::State(_) => Attempt::Busy,
                Probe::NoAnswer => Attempt::NoAnswer,
            };
            *slot.lock() = Some(attempt);
            ctx.request_repaint();
        });
    }

    /// What the footer says about the daemon and a restart that waits.
    pub fn restart_waits_for_take(&self) -> bool {
        self.debounce.pending() && self.machine.busy()
    }

    /// How the last saves went, while the footer shows it: a failure of
    /// either file first, else the latest Saved.
    pub fn save_feedback(&self) -> Option<&Feedback> {
        let failed = [&self.config_feedback, &self.vocabulary_feedback]
            .into_iter()
            .flatten()
            .find(|feedback| matches!(feedback, Feedback::Failed(_)));
        if failed.is_some() {
            return failed;
        }
        [&self.config_feedback, &self.vocabulary_feedback]
            .into_iter()
            .flatten()
            .filter_map(|feedback| match feedback {
                Feedback::Saved(at) if at.elapsed() <= SAVED_DURATION => Some((at, feedback)),
                _ => None,
            })
            .max_by_key(|(at, _)| **at)
            .map(|(_, feedback)| feedback)
    }

    /// Read the machine on a background thread; `take_refresh` picks up the
    /// result on a later frame.
    pub fn request_refresh(&mut self, ctx: &egui::Context) {
        if self.poll.in_flight.swap(true, Ordering::AcqRel) {
            return;
        }
        let latest = Arc::clone(&self.poll.latest);
        let in_flight = Arc::clone(&self.poll.in_flight);
        let ctx = ctx.clone();
        let server_key_env = self.config.transcription.server_key_env.clone();
        std::thread::spawn(move || {
            let machine = Machine::read(&server_key_env);
            *latest.lock() = Some(machine);
            in_flight.store(false, Ordering::Release);
            ctx.request_repaint();
        });
    }

    fn take_refresh(&mut self) {
        let Some(machine) = self.poll.latest.lock().take() else {
            return;
        };
        self.machine = machine;
        self.follow_file_and_safe_mode();
        if self.debounce.pending() {
            match self.machine.probe {
                Probe::NoAnswer => self.held_tries = self.held_tries.saturating_add(1),
                Probe::State(_) if self.held_by_file.is_none() => self.held_tries = 0,
                _ => {},
            }
        }
        let Some(restart) = self.restart.as_mut() else {
            return;
        };
        if !self.machine.running() {
            restart.went_down = true;
        }
        let back = self.machine.running()
            && (restart.went_down || restart.since.elapsed() > Duration::from_secs(3));
        if back {
            self.restart = None;
            self.say("Applied.");
        } else if restart.since.elapsed() > RESTART_TIMEOUT {
            self.restart = None;
            self.say("Saved, but Hush did not come back. Run hush doctor.");
        }
    }

    /// After each poll: a configuration file that loads again replaces the
    /// defaults the window showed, when the user changed nothing, and a
    /// daemon in safe mode gets a restart.
    fn follow_file_and_safe_mode(&mut self) {
        if !self.machine.config_loads {
            return;
        }
        if self.files.config_error.is_some() && self.config == self.saved {
            match self.files.reload_config() {
                Ok(config) => {
                    self.config = config.clone();
                    self.saved = config;
                    self.rejected = None;
                    self.config_feedback = None;
                    self.say("The configuration file loads again.");
                },
                Err(e) => warn!("Could not read the configuration again: {:#}", e),
            }
        }
        self.held_by_file = None;
        let on_its_way = self.debounce.pending() || self.attempting || self.restart.is_some();
        if autosave::leave_safe_mode(
            self.machine.safe_mode.is_some(),
            self.machine.config_loads,
            on_its_way,
        ) {
            self.debounce.change(Instant::now());
        }
    }

    pub fn applying(&self) -> bool {
        self.restart.is_some() || self.attempting
    }

    /// The window draws one frame for a screenshot, so it leaves the
    /// microphone closed.
    pub fn screenshot_mode(&self) -> bool {
        self.screenshot.is_some()
    }

    /// Run a `hush` subcommand in the background, so the window keeps drawing.
    pub fn run_hush(&mut self, args: &[&str], done: &str) {
        let binary = std::env::current_exe().unwrap_or_else(|_| "hush".into());
        let owned: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        match std::process::Command::new(binary).args(&owned).spawn() {
            Ok(_) => self.say(done),
            Err(e) => self.say(format!("Could not run hush {}: {}", owned.join(" "), e)),
        }
    }

    fn finish_download(&mut self, ctx: &egui::Context) {
        let finished = {
            let mut download = self.download.lock();
            let finished = download.finished.take();
            let size = download.size.filter(|_| download.fetches_model);
            if finished.is_some() {
                download.size = None;
            }
            finished.map(|result| (result, size))
        };
        match finished {
            Some((Ok(()), fetched)) => {
                self.window_change = true;
                pages::speech::model_downloaded(&mut self.config.transcription, fetched);
                self.say("Model ready.");
                self.request_refresh(ctx);
            },
            Some((Err(e), _)) => self.say(format!("Download failed: {}", e)),
            None => {},
        }
    }

    fn finish_capture(&mut self) {
        let Some((target, capture)) = &self.capture else {
            return;
        };
        match capture.outcome() {
            Outcome::Listening => {},
            Outcome::Chord(chord) => {
                let target = target.clone();
                self.capture = None;
                match target {
                    KeyTarget::Action(role) => {
                        match pages::hotkeys::refusal(&self.config.hotkey, role, &chord) {
                            Some(message) => self.say(message),
                            None => {
                                if let Some(slot) = self.config.hotkey.chord_mut(role) {
                                    *slot = chord;
                                }
                            },
                        }
                    },
                    KeyTarget::Chord(map, own) => pages::hotkeys::bind_chord(self, map, own, chord),
                }
            },
            Outcome::Ended => self.capture = None,
        }
    }

    fn poll_interval(&self) -> Duration {
        autosave::poll_rate(
            self.applying() || self.capture.is_some() || self.machine.busy(),
            self.debounce.pending(),
            self.held_tries,
            FAST_POLL,
            POLL_INTERVAL,
        )
    }
}

/// `config` with `change`, unless the changed settings would not save.
pub fn checked_change(config: &Config, change: impl FnOnce(&mut Config)) -> Result<Config, String> {
    let mut changed = config.clone();
    change(&mut changed);
    changed.validate().map_err(|e| e.to_string())?;
    Ok(changed)
}

/// Apply `choice` with `apply` when the settings can take it; otherwise the
/// settings stay as they are and the choice comes back to wait.
pub fn choose<T: Copy>(
    config: &mut Config,
    choice: T,
    apply: impl FnOnce(&mut Config, T),
) -> Option<T> {
    match checked_change(config, |config| apply(config, choice)) {
        Ok(next) => {
            *config = next;
            None
        },
        Err(_) => Some(choice),
    }
}

/// The vocabulary file; a missing file is an empty vocabulary.
fn load_vocabulary() -> Result<DomainVocabulary> {
    let path = DomainVocabulary::default_path();
    if !path.exists() {
        return Ok(DomainVocabulary::default());
    }
    DomainVocabulary::load_from_file(&path)
}

impl Gui {
    /// The scheme to draw now: the override, else the setting, which may
    /// follow the desktop.
    fn theme(&self) -> egui::Theme {
        let appearance = self
            .appearance_override
            .unwrap_or(self.config.gui.appearance);
        theme::resolve(appearance, self.desktop.get())
    }

    /// The page in the centred column, scrolled under the title bar.
    fn page(&mut self, ui: &mut egui::Ui) {
        let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]);
        if std::mem::take(&mut self.scroll_top) {
            area = area.vertical_scroll_offset(0.0);
        }
        area.show(ui, |ui| {
            let origin = ui.cursor().min;
            let (left, width) = theme::column(ui.available_width());
            let max = egui::Rect::from_min_size(
                origin + egui::vec2(left, theme::PAGE_PADDING),
                egui::vec2(width, f32::INFINITY),
            );
            let mut column = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(max)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            column.set_width(width);
            column.spacing_mut().item_spacing.y = 0.0;
            pages::show(self, &mut column);
            let bottom = column.min_rect().bottom() + theme::PAGE_PADDING;
            ui.advance_cursor_after_rect(egui::Rect::from_min_max(
                origin,
                egui::pos2(origin.x + ui.available_width(), bottom),
            ));
        });
    }
}

impl eframe::App for Gui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let theme = self.theme();
        if ctx.theme() != theme {
            ctx.set_theme(theme);
        }
        if self.last_poll.elapsed() >= self.poll_interval() {
            self.last_poll = Instant::now();
            self.request_refresh(ctx);
        }
        self.take_refresh();
        self.finish_download(ctx);
        pages::speech::finish_wake(self);
        self.finish_capture();
        self.palette.keys(ctx, self.capture.is_some());
        if self.palette.is_open() {
            self.capture = None;
        }
        if self.page != Page::Speech {
            self.meter = None;
            self.meter_error = None;
        }
        if self
            .toast
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() > TOAST_DURATION)
        {
            self.toast = None;
        }
        if self.page != self.last_page {
            self.last_page = self.page;
            self.disarm();
        }
        self.intercept_close(ctx);

        let colours = theme::palette(ctx);
        egui::SidePanel::left("nav")
            .resizable(false)
            .exact_width(theme::SIDEBAR_WIDTH)
            .frame(egui::Frame::none().fill(colours.bg))
            .show(ctx, |ui| chrome::sidebar(self, ui));
        egui::TopBottomPanel::top("title")
            .exact_height(theme::BAR_HEIGHT)
            .frame(
                egui::Frame::none()
                    .fill(colours.surface)
                    .inner_margin(egui::Margin {
                        left: 16.0,
                        right: 8.0,
                        top: 0.0,
                        bottom: 0.0,
                    }),
            )
            .show(ctx, |ui| chrome::title_bar(ui, self.page.title()));
        egui::TopBottomPanel::bottom("footer")
            .exact_height(theme::FOOTER_HEIGHT)
            .frame(
                egui::Frame::none()
                    .fill(colours.surface)
                    .inner_margin(egui::Margin::symmetric(16.0, 0.0)),
            )
            .show(ctx, |ui| chrome::footer(self, ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(colours.surface))
            .show(ctx, |ui| self.page(ui));
        chrome::resize_edges(ctx);
        chrome::window_border(ctx);
        if self.palette.is_open() {
            self.history.refresh();
        }
        let mut commands = std::mem::take(&mut self.palette);
        let pick = commands.show(ctx, self);
        self.palette = commands;
        if let Some(pick) = pick {
            palette::run(self, ctx, pick);
        }
        jump::expire(ctx);
        let now = Instant::now();
        let rejected = widgets::take_rejections(ctx) > 0;
        let before = (
            self.config_feedback.clone(),
            self.vocabulary_feedback.clone(),
        );
        self.persist(now);
        if (
            self.config_feedback.clone(),
            self.vocabulary_feedback.clone(),
        ) != before
        {
            ctx.request_repaint();
        }
        self.closing_after_pages(rejected);
        self.drive_restart(ctx, now);

        if let Some(screenshot) = self.screenshot.as_mut() {
            screenshot.step(ctx);
            return;
        }
        let repaint = if self.download.lock().size.is_some() {
            Duration::from_millis(100)
        } else if self.meter.is_some() {
            METER_REPAINT
        } else {
            self.poll_interval()
        };
        ctx.request_repaint_after(repaint);
    }
}

/// Open the window. Blocks until the user closes it.
pub fn run() -> Result<()> {
    let mut app = Gui::new()?;
    let size = app
        .screenshot
        .as_ref()
        .and_then(Screenshot::size)
        .unwrap_or([1000.0, 720.0]);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(size)
            .with_min_inner_size([720.0, 480.0])
            .with_decorations(false)
            .with_app_id("io.github.andymai.hush")
            .with_title("Hush"),
        ..Default::default()
    };
    eframe::run_native(
        "Hush",
        options,
        Box::new(|creation| {
            theme::install(&creation.egui_ctx);
            app.desktop = DesktopScheme::watch(&creation.egui_ctx);
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Could not open the Hush window: {}", e))
}

/// Bytes as a short human-readable string.
pub fn megabytes(bytes: u64) -> String {
    format!("{:.0} MB", bytes as f64 / 1_000_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready_machine() -> Machine {
        Machine {
            permissions: PermissionStatus {
                in_container: false,
                rule_installed: true,
                uinput_present: true,
                uinput_writable: true,
                event_nodes: 4,
                readable_event_nodes: 4,
                in_input_group: true,
            },
            daemon: Some(DaemonState::Idle),
            probe: Probe::State(DaemonState::Idle),
            safe_mode: None,
            start_delay: None,
            config_loads: true,
            installed_models: vec![ModelSize::Base],
            vad_installed: true,
            autostart: false,
            cloud_keys: Vec::new(),
        }
    }

    #[test]
    fn readiness_needs_permissions_and_a_model() {
        let mut transcription = Config::defaults().unwrap().transcription;
        assert!(ready_machine().ready(&transcription));
        let missing_model = Machine {
            installed_models: Vec::new(),
            ..ready_machine()
        };
        assert!(!missing_model.ready(&transcription), "a model is required");

        transcription.backend = TranscriptionBackend::Groq;
        assert!(
            !missing_model.ready(&transcription),
            "a cloud service needs its key"
        );
        let with_key = Machine {
            cloud_keys: vec!["GROQ_API_KEY".to_string()],
            ..missing_model
        };
        assert!(
            with_key.ready(&transcription),
            "and then needs no local model"
        );
        transcription.backend = TranscriptionBackend::OpenAi;
        assert!(!with_key.has_key(&transcription));
    }

    #[test]
    fn a_server_is_ready_with_an_address_a_model_and_any_key_it_names() {
        let machine = Machine {
            installed_models: Vec::new(),
            ..ready_machine()
        };
        let mut transcription = Config::defaults().unwrap().transcription;
        transcription.backend = TranscriptionBackend::OpenAiCompatible;
        assert!(!machine.ready(&transcription), "no address yet");
        transcription.server_url = "http://localhost:8080".into();
        transcription.server_model = "whisper-1".into();
        assert!(machine.ready(&transcription), "no key is needed");
        transcription.server_key_env = "STT_KEY".into();
        assert!(!machine.ready(&transcription), "the named key is missing");
        let with_key = Machine {
            cloud_keys: vec!["STT_KEY".to_string()],
            ..machine
        };
        assert!(with_key.ready(&transcription));
    }

    #[test]
    fn a_daemon_mid_dictation_counts_as_busy() {
        let mut machine = ready_machine();
        assert!(machine.running() && !machine.busy());
        machine.daemon = Some(DaemonState::Recording { elapsed_ms: 10 });
        assert!(machine.busy());
        machine.daemon = Some(DaemonState::Starting);
        assert!(machine.busy());
        machine.daemon = None;
        assert!(!machine.running() && !machine.busy());
    }

    #[test]
    fn an_invalid_value_never_replaces_settings_that_save() {
        let config = Config::defaults().unwrap();
        let err =
            checked_change(&config, |c| c.hotkey.combination = "Ctrl+Bogus".into()).unwrap_err();
        assert!(err.contains("Bogus"), "{err}");
        let changed = checked_change(&config, |c| c.hotkey.tap_ms = 0).unwrap();
        assert_eq!(changed.hotkey.tap_ms, 0);
    }

    #[test]
    fn an_unfinished_choice_waits_outside_the_settings_until_they_take_it() {
        let set = |config: &mut Config, backend| config.transcription.backend = backend;
        let mut config = Config::defaults().unwrap();
        let server = TranscriptionBackend::OpenAiCompatible;
        assert_eq!(
            choose(&mut config, server, set),
            Some(server),
            "no address yet"
        );
        assert_eq!(config.transcription.backend, TranscriptionBackend::Local);
        assert!(config.validate().is_ok(), "the settings still save");

        config = checked_change(&config, |c| {
            c.transcription.server_url = "http://localhost:8080/v1".into()
        })
        .unwrap();
        assert_eq!(
            choose(&mut config, server, set),
            Some(server),
            "no model yet"
        );
        config = checked_change(&config, |c| {
            c.transcription.server_model = "whisper-large-v3".into()
        })
        .unwrap();
        assert_eq!(choose(&mut config, server, set), None);
        assert_eq!(config.transcription.backend, server);
        assert_eq!(
            choose(&mut config, TranscriptionBackend::Groq, set),
            None,
            "a choice that saves applies at once"
        );
    }

    #[test]
    fn sizes_read_as_megabytes() {
        assert_eq!(megabytes(147_000_000), "147 MB");
        assert_eq!(megabytes(0), "0 MB");
    }

    /// A window on the defaults that reads nothing of this machine. Its
    /// files are in a folder that nothing writes.
    pub(super) fn window() -> Gui {
        let folder = std::env::temp_dir().join("hush-gui-test-never-written");
        let files = Files::at(folder.join("config.toml"), folder.join("vocabulary.json"));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        Gui::with(
            Config::defaults().unwrap(),
            DomainVocabulary::default(),
            files,
            ready_machine(),
            runtime,
        )
    }

    fn listen(gui: &mut Gui) {
        gui.capture = Some((KeyTarget::Action(Role::Dictate), Capture::stand_in()));
    }

    #[test]
    fn a_page_change_stops_a_key_field_that_listens() {
        let mut gui = window();
        listen(&mut gui);
        gui.go_to(Page::Text);
        assert!(gui.capture.is_none());
        assert_eq!((gui.page, gui.last_page), (Page::Text, Page::Text));
        listen(&mut gui);
        gui.go_to(Page::Text);
        assert!(
            gui.capture.is_none(),
            "a jump on the same page stops it too"
        );
    }

    #[test]
    fn commands_in_the_sidebar_opens_the_palette_and_stops_a_key_field() {
        let mut gui = window();
        listen(&mut gui);
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let at = egui::pos2(100.0, 696.0);
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        for events in [
            Vec::new(),
            vec![egui::Event::PointerMoved(at), button(true)],
            vec![button(false)],
        ] {
            let input = egui::RawInput {
                events,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 720.0),
                )),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::SidePanel::left("nav")
                    .resizable(false)
                    .exact_width(theme::SIDEBAR_WIDTH)
                    .frame(egui::Frame::none())
                    .show(ctx, |ui| chrome::sidebar(&mut gui, ui));
            });
        }
        assert!(gui.palette.is_open());
        assert!(gui.capture.is_none());
    }
}
