//! The Hush window: an overview of what still needs setting up, then one page
//! for each group of settings.
//!
//! It runs in its own process (`hush settings`), which the tray and the
//! desktop entry launch, so the daemon never has to own a window.

mod autosave;
mod capture;
mod chrome;
mod icons;
mod meter;
mod pages;
mod scheme;
mod screenshot;
mod theme;
mod widgets;

pub use pages::speech::size_key;

use crate::config::settings::{Appearance, TranscriptionBackend, TranscriptionConfig};
use crate::config::Config;
use crate::hotkey::keymap::ModifierKeys;
use crate::hotkey::Role;
use crate::ipc::{client, DaemonState};
use crate::permissions::PermissionStatus;
use crate::text::layout::Layout;
use crate::text_processing::DomainVocabulary;
use crate::transcription::models::{ModelManager, ModelSize};
use anyhow::{Context, Result};
use autosave::{Daemon, Debounce, Feedback, Saved};
use capture::{Capture, Outcome};
use meter::Meter;
use pages::history::HistoryView;
use pages::hotkeys::KeyTarget;
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
        Self {
            permissions: crate::permissions::status(),
            daemon: client::status_blocking(STATUS_TIMEOUT),
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
    Deferred,
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
    pub try_text: String,
    /// The desktop's light or dark preference, for the System appearance
    desktop: DesktopScheme,
    /// The appearance that `HUSH_SETTINGS_APPEARANCE` forces for a
    /// screenshot
    appearance_override: Option<Appearance>,
    screenshot: Option<Screenshot>,
    /// The last save, or why the window's settings cannot save
    pub feedback: Option<Feedback>,
    /// Settings that failed to save, so they are not tried again each frame
    rejected: Option<Config>,
    rejected_vocabulary: Option<DomainVocabulary>,
    /// Reset everything waits for a second click
    pub confirm_reset: bool,
    debounce: Debounce,
    attempt: Arc<Mutex<Option<Attempt>>>,
    attempting: bool,
    /// The window closes once the pending restart is under way
    closing: bool,
    restart: Option<Restart>,
    poll: Poll,
    last_poll: Instant,
}

impl Gui {
    fn new() -> Result<Self> {
        let config = match Config::load() {
            Ok(config) => config,
            Err(e) => {
                warn!("Falling back to the built-in defaults: {}", e);
                Config::defaults().context("The built-in defaults are unreadable")?
            },
        };
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .context("Failed to start the background runtime")?;
        let machine = Machine::read(&config.transcription.server_key_env);
        let vocabulary = load_vocabulary();
        let manager = ModelManager::new(crate::config::paths::models_dir()).ok();
        let catalogue = MODEL_ORDER
            .iter()
            .filter_map(|size| {
                let info = manager.as_ref()?.get_model_info(size)?;
                Some((*size, info.expected_size))
            })
            .collect();
        let screenshot = Screenshot::from_env();
        Ok(Self {
            page: Page::from_env().unwrap_or(Page::Overview),
            saved: config.clone(),
            config,
            saved_vocabulary: vocabulary.clone(),
            vocabulary,
            machine,
            catalogue,
            recommended: crate::transcription::models::recommended_model(),
            download: Arc::new(Mutex::new(Download::default())),
            runtime,
            toast: None,
            capture: None,
            new_language: String::new(),
            new_style: String::new(),
            new_rewrite: String::new(),
            editing_key: None,
            focus_editor: false,
            layout: crate::text::layout::current(),
            modifier_keys: ModifierKeys::session(),
            meter: None,
            meter_error: None,
            meter_wanted: None,
            microphones: if screenshot.is_some() {
                Vec::new()
            } else {
                meter::input_devices()
            },
            level_shown: 0.0,
            new_term: String::new(),
            editing_term: None,
            term_filter: String::new(),
            style_draft: StyleDraft::default(),
            word_list: pages::text::DEFAULT_WORD_LIST.to_string(),
            new_uinput_app: String::new(),
            new_clipboard_app: String::new(),
            history: HistoryView::default(),
            try_text: String::new(),
            desktop: DesktopScheme::default(),
            appearance_override: screenshot
                .as_ref()
                .and_then(|_| theme::appearance_from_env()),
            screenshot,
            feedback: None,
            rejected: None,
            rejected_vocabulary: None,
            confirm_reset: false,
            debounce: Debounce::default(),
            attempt: Arc::new(Mutex::new(None)),
            attempting: false,
            closing: false,
            restart: None,
            poll: Poll::default(),
            last_poll: Instant::now(),
        })
    }

    pub fn say(&mut self, message: impl Into<String>) {
        self.toast = Some((message.into(), Instant::now()));
    }

    /// Put a change into the window's settings, which save at the end of the
    /// frame. A change that would stop settings that save from saving is
    /// refused with the reason, and the settings keep their value.
    pub fn try_set(&mut self, change: impl FnOnce(&mut Config)) -> Result<(), String> {
        self.config = checked_change(&self.config, change)?;
        Ok(())
    }

    /// Write what changed: the configuration, the vocabulary, or both. A
    /// change that the daemon reads restarts it after the changes stop.
    fn persist(&mut self, now: Instant) {
        if self.screenshot_mode() {
            return;
        }
        if self.config == self.saved && self.vocabulary == self.saved_vocabulary {
            if matches!(self.feedback, Some(Feedback::Failed(_))) {
                self.feedback = None;
            }
            self.rejected = None;
            self.rejected_vocabulary = None;
        }
        if self.config != self.saved && self.rejected.as_ref() != Some(&self.config) {
            self.save_config(now);
        }
        if self.vocabulary != self.saved_vocabulary
            && self.rejected_vocabulary.as_ref() != Some(&self.vocabulary)
        {
            match save_vocabulary(&self.vocabulary) {
                Ok(()) => {
                    self.saved_vocabulary = self.vocabulary.clone();
                    self.rejected_vocabulary = None;
                    self.feedback = Some(Feedback::Saved(now));
                    self.debounce.change(now);
                },
                Err(e) => {
                    self.rejected_vocabulary = Some(self.vocabulary.clone());
                    self.feedback = Some(Feedback::Failed(format!(
                        "Could not save the vocabulary: {:#}",
                        e
                    )));
                },
            }
        }
    }

    fn save_config(&mut self, now: Instant) {
        let path = crate::config::paths::config_path();
        let restart = match autosave::save_config(&self.saved, &self.config, |config| {
            config.save_to_file(&path)
        }) {
            Saved::Written { restart } => restart,
            Saved::Refused(problem) => {
                self.rejected = Some(self.config.clone());
                self.feedback = Some(Feedback::Failed(problem));
                return;
            },
        };
        let keep_days = self.config.history.keep_audio_days;
        if keep_days != self.saved.history.keep_audio_days {
            if let Err(e) = crate::audio::store::sweep_now(keep_days) {
                warn!("Could not sweep the kept audio: {:#}", e);
            }
            self.history.reload();
        }
        if restart {
            self.debounce.change(now);
        }
        self.saved = self.config.clone();
        self.rejected = None;
        self.feedback = Some(Feedback::Saved(now));
    }

    /// Restart the daemon once the changes stop and it is idle, and close
    /// the window once a restart that it owes is under way.
    fn drive_restart(&mut self, ctx: &egui::Context, now: Instant) {
        let attempt = self.attempt.lock().take();
        if let Some(attempt) = attempt {
            self.attempting = false;
            match attempt {
                Attempt::Issued => {
                    self.restart = Some(Restart {
                        since: now,
                        went_down: false,
                    });
                },
                Attempt::Deferred => {
                    self.debounce.change(now);
                    self.request_refresh(ctx);
                },
                Attempt::NotRunning => {},
                Attempt::Failed(e) => self.say(format!("Could not restart Hush: {}", e)),
            }
        }
        // A second close while a take keeps the restart waiting closes at
        // once; the daemon reads the change when it next starts.
        if ctx.input(|input| input.viewport().close_requested())
            && self.restart_owed()
            && !self.closing
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.closing = true;
        }
        if self.closing {
            self.debounce.hurry(now);
        }
        let daemon = Daemon::of(
            self.machine.daemon,
            self.restart.is_some() || self.attempting,
        );
        if self.debounce.fire(now, daemon) {
            self.attempt_restart(ctx);
        }
        if self.closing && !self.restart_owed() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if let Some(wait) = self.debounce.wait(now) {
            ctx.request_repaint_after(wait.max(Duration::from_millis(20)));
        }
    }

    /// A change that the daemon reads is saved, but the daemon does not run
    /// with it yet.
    fn restart_owed(&self) -> bool {
        self.debounce.pending() || self.attempting
    }

    /// Ask the daemon once more on a thread, so a take that began after the
    /// last poll is not cut, then restart it.
    fn attempt_restart(&mut self, ctx: &egui::Context) {
        self.attempting = true;
        let slot = Arc::clone(&self.attempt);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let attempt = match client::status_blocking(STATUS_TIMEOUT) {
                None => Attempt::NotRunning,
                Some(DaemonState::Idle) => {
                    let binary = std::env::current_exe().unwrap_or_else(|_| "hush".into());
                    match std::process::Command::new(binary)
                        .args(["daemon", "restart"])
                        .spawn()
                    {
                        Ok(_) => Attempt::Issued,
                        Err(e) => Attempt::Failed(e.to_string()),
                    }
                },
                Some(_) => Attempt::Deferred,
            };
            *slot.lock() = Some(attempt);
            ctx.request_repaint();
        });
    }

    /// What the footer says about the daemon and a restart that waits.
    pub fn restart_waits_for_take(&self) -> bool {
        self.debounce.pending() && self.machine.busy()
    }

    /// The last save, while the footer shows it.
    pub fn save_feedback(&self) -> Option<&Feedback> {
        match &self.feedback {
            Some(Feedback::Saved(at)) if at.elapsed() > SAVED_DURATION => None,
            other => other.as_ref(),
        }
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
        if self.applying()
            || self.debounce.pending()
            || self.capture.is_some()
            || self.machine.busy()
        {
            FAST_POLL
        } else {
            POLL_INTERVAL
        }
    }
}

/// `config` with `change`, unless the change would stop settings that save
/// from saving. Settings that already fail to save, such as a server picked
/// before its address, take any change, so the user can finish them.
fn checked_change(config: &Config, change: impl FnOnce(&mut Config)) -> Result<Config, String> {
    let mut changed = config.clone();
    change(&mut changed);
    if config.validate().is_ok() {
        changed.validate().map_err(|e| e.to_string())?;
    }
    Ok(changed)
}

fn load_vocabulary() -> DomainVocabulary {
    let path = DomainVocabulary::default_path();
    if !path.exists() {
        return DomainVocabulary::default();
    }
    DomainVocabulary::load_from_file(&path).unwrap_or_else(|e| {
        warn!("Could not read the vocabulary file: {}", e);
        DomainVocabulary::default()
    })
}

fn save_vocabulary(vocabulary: &DomainVocabulary) -> Result<()> {
    let path = DomainVocabulary::default_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    vocabulary.save_to_file(&path)
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
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
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
        self.finish_capture();
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
        let now = Instant::now();
        let before = self.feedback.clone();
        self.persist(now);
        if self.feedback != before {
            ctx.request_repaint();
        }
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
    fn settings_that_cannot_save_yet_take_the_changes_that_finish_them() {
        let mut config = Config::defaults().unwrap();
        config.transcription.backend = TranscriptionBackend::OpenAiCompatible;
        assert!(config.validate().is_err(), "no server address yet");
        let config = checked_change(&config, |c| {
            c.transcription.server_url = "http://localhost:8080/v1".into()
        })
        .unwrap();
        assert!(config.validate().is_err(), "no model yet");
        let config = checked_change(&config, |c| {
            c.transcription.server_model = "whisper-large-v3".into()
        })
        .unwrap();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn sizes_read_as_megabytes() {
        assert_eq!(megabytes(147_000_000), "147 MB");
        assert_eq!(megabytes(0), "0 MB");
    }
}
