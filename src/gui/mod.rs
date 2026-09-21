//! The Hush window: an overview of what still needs setting up, then one page
//! for each group of settings.
//!
//! It runs in its own process (`hush settings`), which the tray and the
//! desktop entry launch, so the daemon never has to own a window.

mod capture;
mod meter;
mod pages;
mod theme;

pub use pages::speech::size_key;

use crate::config::Config;
use crate::ipc::{client, DaemonState};
use crate::permissions::PermissionStatus;
use crate::text_processing::DomainVocabulary;
use crate::transcription::models::{ModelManager, ModelSize};
use anyhow::{Context, Result};
use capture::{Capture, Outcome};
use meter::Meter;
use pages::hotkeys::KeyField;
use pages::Page;
use parking_lot::Mutex;
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
const NAV_WIDTH: f32 = 150.0;

pub const MODEL_ORDER: [ModelSize; 7] = [
    ModelSize::Tiny,
    ModelSize::Base,
    ModelSize::Small,
    ModelSize::Medium,
    ModelSize::Large,
    ModelSize::LargeV2,
    ModelSize::LargeV3,
];

/// A model download in flight, shared with the task doing the work.
#[derive(Default)]
pub struct Download {
    pub size: Option<ModelSize>,
    pub done: u64,
    pub total: u64,
    pub finished: Option<Result<(), String>>,
}

/// What the window shows about the machine, refreshed on a timer.
pub struct Machine {
    pub permissions: PermissionStatus,
    pub daemon: Option<DaemonState>,
    pub installed_models: Vec<ModelSize>,
    pub autostart: bool,
}

impl Machine {
    fn read() -> Self {
        let manager = ModelManager::new(crate::config::paths::models_dir()).ok();
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
            autostart: crate::desktop::autostart_installed(),
        }
    }

    pub fn ready(&self) -> bool {
        self.permissions.ready() && !self.installed_models.is_empty()
    }

    pub fn running(&self) -> bool {
        self.daemon.is_some()
    }

    pub fn busy(&self) -> bool {
        matches!(
            self.daemon,
            Some(
                DaemonState::Recording { .. } | DaemonState::Transcribing | DaemonState::Inserting
            )
        )
    }
}

/// A daemon restart after Save, watched until the daemon answers again.
struct Restart {
    since: Instant,
    went_down: bool,
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
    pub capture: Option<(KeyField, Capture)>,
    /// The hotkey field showing a text box instead of a capture button.
    pub editing_key: Option<KeyField>,
    pub focus_editor: bool,
    pub meter: Option<Meter>,
    pub meter_error: Option<String>,
    /// The microphone the meter was last asked to open, so a failure is
    /// reported once rather than retried every frame.
    pub meter_wanted: Option<String>,
    pub microphones: Vec<String>,
    pub level_shown: f32,
    pub new_term: String,
    pub try_text: String,
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
        let machine = Machine::read();
        let vocabulary = load_vocabulary();
        let manager = ModelManager::new(crate::config::paths::models_dir()).ok();
        let catalogue = MODEL_ORDER
            .iter()
            .filter_map(|size| {
                let info = manager.as_ref()?.get_model_info(size)?;
                Some((*size, info.expected_size))
            })
            .collect();
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
            editing_key: None,
            focus_editor: false,
            meter: None,
            meter_error: None,
            meter_wanted: None,
            microphones: meter::input_devices(),
            level_shown: 0.0,
            new_term: String::new(),
            try_text: String::new(),
            restart: None,
            poll: Poll::default(),
            last_poll: Instant::now(),
        })
    }

    pub fn say(&mut self, message: impl Into<String>) {
        self.toast = Some((message.into(), Instant::now()));
    }

    pub fn unsaved(&self) -> bool {
        self.config != self.saved || self.vocabulary != self.saved_vocabulary
    }

    /// Write the configuration and the vocabulary, then restart a running
    /// daemon so the change is live.
    pub fn save(&mut self) {
        let path = crate::config::paths::config_path();
        if let Err(e) = self.config.save_to_file(&path) {
            self.say(format!("Could not save: {}", e));
            return;
        }
        self.saved = self.config.clone();
        if self.vocabulary != self.saved_vocabulary {
            if let Err(e) = save_vocabulary(&self.vocabulary) {
                self.say(format!("Saved the settings, but not the vocabulary: {}", e));
                return;
            }
            self.saved_vocabulary = self.vocabulary.clone();
        }
        if self.machine.running() {
            self.run_hush(&["daemon", "restart"], "Applying…");
            self.restart = Some(Restart {
                since: Instant::now(),
                went_down: false,
            });
        } else {
            self.say("Saved.");
        }
    }

    pub fn revert(&mut self) {
        self.config = self.saved.clone();
        self.vocabulary = self.saved_vocabulary.clone();
        self.capture = None;
        self.editing_key = None;
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
        std::thread::spawn(move || {
            let machine = Machine::read();
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
        self.restart.is_some()
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
            let size = download.size;
            if finished.is_some() {
                download.size = None;
            }
            finished.map(|result| (result, size))
        };
        match finished {
            Some((Ok(()), size)) => {
                if let Some(size) = size {
                    self.config.transcription.model_size = size_key(size).to_string();
                }
                self.say("Model ready. Save to use it.");
                self.request_refresh(ctx);
            },
            Some((Err(e), _)) => self.say(format!("Download failed: {}", e)),
            None => {},
        }
    }

    fn finish_capture(&mut self) {
        let Some((field, capture)) = &self.capture else {
            return;
        };
        match capture.outcome() {
            Outcome::Listening => {},
            Outcome::Chord(chord) => {
                let field = *field;
                *field.value(&mut self.config) = chord;
                self.capture = None;
            },
            Outcome::Ended => self.capture = None,
        }
    }

    fn poll_interval(&self) -> Duration {
        if self.applying() || self.capture.is_some() || self.machine.busy() {
            FAST_POLL
        } else {
            POLL_INTERVAL
        }
    }
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

impl eframe::App for Gui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        theme::apply(ctx);
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

        let nav_frame = egui::Frame::side_top_panel(&ctx.style())
            .fill(theme::palette(ctx).nav)
            .inner_margin(egui::Margin::symmetric(8.0, 4.0));
        egui::SidePanel::left("nav")
            .resizable(false)
            .exact_width(NAV_WIDTH)
            .frame(nav_frame)
            .show(ctx, |ui| {
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new("Hush").strong().size(20.0));
                });
                ui.add_space(14.0);
                ui.spacing_mut().item_spacing.y = 2.0;
                for page in Page::ALL {
                    let response = theme::nav_item(ui, self.page == page, page.title());
                    if response.clicked() {
                        self.page = page;
                    }
                    if page == Page::Overview && !self.machine.ready() {
                        let centre =
                            egui::pos2(response.rect.right() - 12.0, response.rect.center().y);
                        ui.painter().circle_filled(centre, 4.0, theme::WARN);
                    }
                }
            });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if let Some((message, _)) = &self.toast {
                    ui.label(message.clone());
                } else if self.applying() {
                    ui.spinner();
                    ui.label("Applying…");
                } else if self.machine.running() {
                    theme::state_line(ui, true, "Hush is running");
                } else {
                    theme::state_line(ui, false, "Hush is not running");
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_save = self.unsaved() && !self.applying();
                    ui.add_enabled_ui(can_save, |ui| {
                        let label = if can_save {
                            egui::RichText::new("Save")
                                .strong()
                                .color(egui::Color32::WHITE)
                        } else {
                            egui::RichText::new("Save")
                        };
                        let size = egui::vec2(72.0, theme::CONTROL_HEIGHT);
                        let save = egui::Button::new(label).min_size(size).fill(if can_save {
                            theme::ACCENT
                        } else {
                            ui.visuals().widgets.inactive.weak_bg_fill
                        });
                        if ui
                            .add(save)
                            .on_hover_text("Writes the file and restarts Hush with it")
                            .clicked()
                        {
                            self.save();
                        }
                        if ui.add(egui::Button::new("Revert").min_size(size)).clicked() {
                            self.revert();
                        }
                    });
                    if self.unsaved() && !self.applying() {
                        theme::hint(ui, "Unsaved changes");
                    }
                });
            });
            ui.add_space(6.0);
        });

        let frame = egui::Frame::central_panel(&ctx.style())
            .inner_margin(egui::Margin::symmetric(28.0, 18.0));
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(self.page.title()).strong().size(20.0));
                    ui.add_space(2.0);
                    pages::show(self, ui);
                    ui.add_space(16.0);
                });
        });

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
    let app = Gui::new()?;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([760.0, 720.0])
            .with_min_inner_size([620.0, 460.0])
            .with_app_id("io.github.andymai.hush")
            .with_title("Hush"),
        ..Default::default()
    };
    eframe::run_native("Hush", options, Box::new(|_| Ok(Box::new(app))))
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
            autostart: false,
        }
    }

    #[test]
    fn readiness_needs_permissions_and_a_model() {
        assert!(ready_machine().ready());
        let missing_model = Machine {
            installed_models: Vec::new(),
            ..ready_machine()
        };
        assert!(!missing_model.ready(), "a model is required");
    }

    #[test]
    fn a_daemon_mid_dictation_counts_as_busy() {
        let mut machine = ready_machine();
        assert!(machine.running() && !machine.busy());
        machine.daemon = Some(DaemonState::Recording { elapsed_ms: 10 });
        assert!(machine.busy());
        machine.daemon = None;
        assert!(!machine.running() && !machine.busy());
    }

    #[test]
    fn sizes_read_as_megabytes() {
        assert_eq!(megabytes(147_000_000), "147 MB");
        assert_eq!(megabytes(0), "0 MB");
    }
}
