//! The Hush window: what needs setting up, and every setting.
//!
//! It runs in its own process (`hush settings`), which the tray and the
//! desktop entry launch, so the daemon never has to own a window.

mod settings;
mod setup;
mod theme;

use crate::config::Config;
use crate::permissions::PermissionStatus;
use crate::transcription::models::{ModelManager, ModelSize};
use anyhow::{Context, Result};
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::warn;

const POLL_INTERVAL: Duration = Duration::from_millis(700);
const TOAST_DURATION: Duration = Duration::from_secs(4);

pub const MODEL_ORDER: [ModelSize; 7] = [
    ModelSize::Tiny,
    ModelSize::Base,
    ModelSize::Small,
    ModelSize::Medium,
    ModelSize::Large,
    ModelSize::LargeV2,
    ModelSize::LargeV3,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Setup,
    Settings,
}

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
    pub daemon_running: bool,
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
            daemon_running: crate::ipc::paths::socket_path().exists(),
            installed_models,
            autostart: crate::desktop::autostart_installed(),
        }
    }

    pub fn ready(&self) -> bool {
        self.permissions.ready() && !self.installed_models.is_empty()
    }
}

pub struct Gui {
    pub config: Config,
    pub saved: Config,
    pub tab: Tab,
    pub machine: Machine,
    pub download: Arc<Mutex<Download>>,
    pub runtime: tokio::runtime::Runtime,
    pub toast: Option<(String, Instant)>,
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
        let machine = Machine::read();
        Ok(Self {
            tab: if machine.ready() {
                Tab::Settings
            } else {
                Tab::Setup
            },
            saved: config.clone(),
            config,
            machine,
            download: Arc::new(Mutex::new(Download::default())),
            runtime: tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .context("Failed to start the background runtime")?,
            toast: None,
            last_poll: Instant::now(),
        })
    }

    pub fn say(&mut self, message: impl Into<String>) {
        self.toast = Some((message.into(), Instant::now()));
    }

    pub fn unsaved(&self) -> bool {
        self.config != self.saved
    }

    pub fn save(&mut self) {
        let path = crate::config::paths::config_path();
        match self.config.save_to_file(&path) {
            Ok(()) => {
                self.saved = self.config.clone();
                if self.machine.daemon_running {
                    self.say("Saved. Restart Hush to apply.");
                } else {
                    self.say("Saved.");
                }
            },
            Err(e) => self.say(format!("Could not save: {}", e)),
        }
    }

    pub fn refresh(&mut self) {
        self.machine = Machine::read();
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
}

impl eframe::App for Gui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        theme::apply(ctx);
        if self.last_poll.elapsed() >= POLL_INTERVAL {
            self.last_poll = Instant::now();
            self.refresh();
        }
        let finished = {
            let mut download = self.download.lock();
            let finished = download.finished.take();
            if finished.is_some() {
                download.size = None;
            }
            finished
        };
        match finished {
            Some(Ok(())) => {
                self.say("Model ready.");
                self.refresh();
            },
            Some(Err(e)) => self.say(format!("Download failed: {}", e)),
            None => {},
        }
        if self
            .toast
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() > TOAST_DURATION)
        {
            self.toast = None;
        }

        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading("Hush");
                ui.add_space(12.0);
                let setup_label = if self.machine.ready() {
                    "Setup".to_string()
                } else {
                    "Setup ●".to_string()
                };
                ui.selectable_value(&mut self.tab, Tab::Setup, setup_label);
                ui.selectable_value(&mut self.tab, Tab::Settings, "Settings");
            });
            ui.add_space(6.0);
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if let Some((message, _)) = &self.toast {
                    ui.label(message.clone());
                } else if self.machine.daemon_running {
                    ui.label("Hush is running.");
                } else {
                    ui.label("Hush is not running.");
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.tab == Tab::Settings {
                        ui.add_enabled_ui(self.unsaved(), |ui| {
                            if ui.button("Save").clicked() {
                                self.save();
                            }
                            if ui.button("Revert").clicked() {
                                self.config = self.saved.clone();
                            }
                        });
                    }
                });
            });
            ui.add_space(4.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| match self.tab {
                Tab::Setup => setup::show(self, ui),
                Tab::Settings => settings::show(self, ui),
            });
        });

        if self.download.lock().size.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        } else {
            ctx.request_repaint_after(POLL_INTERVAL);
        }
    }
}

/// Open the window. Blocks until the user closes it.
pub fn run() -> Result<()> {
    let app = Gui::new()?;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([640.0, 620.0])
            .with_min_inner_size([520.0, 420.0])
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

    #[test]
    fn the_first_tab_depends_on_what_is_missing() {
        let ready = Machine {
            permissions: PermissionStatus {
                in_container: false,
                rule_installed: true,
                uinput_present: true,
                uinput_writable: true,
                event_nodes: 4,
                readable_event_nodes: 4,
                in_input_group: true,
            },
            daemon_running: true,
            installed_models: vec![ModelSize::Base],
            autostart: false,
        };
        assert!(ready.ready());

        let missing_model = Machine {
            installed_models: Vec::new(),
            ..ready
        };
        assert!(!missing_model.ready(), "a model is required");
    }

    #[test]
    fn sizes_read_as_megabytes() {
        assert_eq!(megabytes(147_000_000), "147 MB");
        assert_eq!(megabytes(0), "0 MB");
    }
}
