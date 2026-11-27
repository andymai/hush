//! The first run, one step at a time: keyboard access, the model, the
//! dictation key, a take to try, the hardware tune, the start at login, and
//! privacy. Each step can be skipped, and `setup.toml` keeps the progress,
//! so the setup resumes where it stopped.

use super::{hotkeys, overview, speech, Page};
use crate::config::settings::TranscriptionBackend;
use crate::gui::widgets::{intro, row, section, switch_row, Button, Size, Variant};
use crate::gui::{icons, Gui};
use crate::hotkey::Role;
use crate::ipc::protocol::DaemonCommand;
use crate::portal::InputMode;
use crate::setup::{self, Facts, Progress, SetupLock, SetupState, Step};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{debug, warn};

pub struct Setup {
    state: SetupState,
    path: PathBuf,
    /// The open step; `None` when every step is done or skipped
    current: Option<Step>,
    /// The model step started its download once
    downloading: bool,
    /// The daemon was asked to keep the next take for the tune
    asked_keep: bool,
    _lock: SetupLock,
}

impl Setup {
    /// Resume from `setup.toml`, or start from what `facts` finds. The file
    /// is written at once, before the window saves any setting.
    pub fn open(facts: &Facts, lock: SetupLock) -> Self {
        let path = setup::path();
        let state = SetupState::load(&path).unwrap_or_else(|| SetupState::derive(facts));
        let setup = Self {
            current: state.next(),
            state,
            path,
            downloading: false,
            asked_keep: false,
            _lock: lock,
        };
        setup.save();
        setup
    }

    fn save(&self) {
        if let Err(e) = self.state.save(&self.path) {
            warn!("The setup could not keep its progress: {:#}", e);
        }
    }

    fn finish(&mut self, step: Step, progress: Progress) {
        self.state.set(step, progress);
        self.save();
        self.current = self.state.next();
    }
}

pub fn show(gui: &mut Gui, ui: &mut egui::Ui) {
    complete_what_is_ready(gui);
    let Some(current) = gui.setup.as_ref().map(|setup| setup.current) else {
        return;
    };
    section(ui, "Set up Hush");
    intro(
        ui,
        "Seven steps get Hush ready. You can skip any of them: each one also has its \
         place in the settings.",
    );
    for step in Step::ALL {
        if current == Some(step) {
            open_step(gui, ui, step);
        } else {
            closed_step(gui, ui, step);
        }
    }
    if current.is_none() {
        finished(gui, ui);
    }
}

/// Keyboard access and the model need nothing more once they are there, as
/// after a grant or a download: the setup goes on by itself.
fn complete_what_is_ready(gui: &mut Gui) {
    let ready = |gui: &Gui, step: Step| match step {
        Step::Access => gui.machine.can_type(),
        Step::Model => model_ready(gui),
        _ => false,
    };
    while let Some(step) = gui.setup.as_ref().and_then(|setup| setup.current) {
        let pending = gui
            .setup
            .as_ref()
            .is_some_and(|setup| setup.state.progress(step) == Progress::Pending);
        if !(pending && ready(gui, step)) {
            return;
        }
        if let Some(setup) = gui.setup.as_mut() {
            setup.finish(step, Progress::Done);
        }
    }
}

/// A step that is not open: its state, and a button that opens it.
fn closed_step(gui: &mut Gui, ui: &mut egui::Ui, step: Step) {
    let Some(setup) = gui.setup.as_mut() else {
        return;
    };
    let progress = setup.state.progress(step);
    let (description, label) = match progress {
        Progress::Done => ("Done.", "Change"),
        Progress::Skipped => ("Skipped.", "Do it now"),
        Progress::Pending => ("To do.", "Do it now"),
    };
    let key = format!("setup.{:?}", step).to_lowercase();
    let mut built = row(&key, step.title()).describe(description).reserve(120.0);
    if progress == Progress::Done {
        built = built.status(true);
    }
    built.show(ui, |ui| {
        let button = Button::new(label).variant(Variant::Quiet).size(Size::Small);
        if ui.add(button).clicked() {
            setup.current = Some(step);
        }
    });
}

/// The open step: its rows, then Continue, which waits for the step's goal,
/// and Skip.
fn open_step(gui: &mut Gui, ui: &mut egui::Ui, step: Step) {
    let number = Step::ALL.iter().position(|s| *s == step).unwrap_or(0) + 1;
    section(
        ui,
        &format!("Step {} of {}: {}", number, Step::ALL.len(), step.title()),
    );
    let met = match step {
        Step::Access => access(gui, ui),
        Step::Model => model(gui, ui),
        Step::Hotkey => hotkey(gui, ui),
        Step::TryIt => try_it(gui, ui),
        Step::Tune => tune(gui, ui),
        Step::Autostart => {
            overview::autostart_row(gui, ui);
            true
        },
        Step::Privacy => privacy(gui, ui),
    };
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let next = Button::new("Continue")
            .variant(Variant::Primary)
            .enabled(met);
        if ui.add(next).clicked() {
            if let Some(setup) = gui.setup.as_mut() {
                setup.finish(step, Progress::Done);
            }
        }
        if ui
            .add(Button::new("Skip").variant(Variant::Quiet))
            .clicked()
        {
            if let Some(setup) = gui.setup.as_mut() {
                setup.finish(step, Progress::Skipped);
            }
        }
    });
    ui.add_space(8.0);
}

fn access(gui: &mut Gui, ui: &mut egui::Ui) -> bool {
    intro(
        ui,
        "Hush reads its keys from the keyboard and types the text through a virtual \
         keyboard, so it works in every application.",
    );
    overview::access_row(gui, ui);
    gui.machine.can_type()
}

/// The model of the settings is on this machine, or a service has its key.
fn model_ready(gui: &Gui) -> bool {
    let transcription = &gui.config.transcription;
    match transcription.backend {
        TranscriptionBackend::Local => {
            transcription.model_path.is_some()
                || transcription
                    .model_size
                    .parse()
                    .is_ok_and(|size| gui.machine.installed_models.contains(&size))
        },
        _ => gui.machine.ready(transcription) || gui.machine.has_key(transcription),
    }
}

fn model(gui: &mut Gui, ui: &mut egui::Ui) -> bool {
    intro(
        ui,
        "Hush transcribes on this machine with a Whisper model. base downloads now, and \
         the hardware tune can pick a more accurate one later.",
    );
    let start = gui.setup.as_ref().is_some_and(|setup| !setup.downloading);
    if start && !model_ready(gui) && gui.download.lock().size.is_none() {
        if let Some(size) = speech::download_offer(gui) {
            speech::start_download(gui, size);
        }
        if let Some(setup) = gui.setup.as_mut() {
            setup.downloading = true;
        }
    }
    speech::model_rows(gui, ui);
    model_ready(gui)
}

fn hotkey(gui: &mut Gui, ui: &mut egui::Ui) -> bool {
    if InputMode::detect(gui.config.input.backend).is_portal() {
        intro(
            ui,
            "The desktop asks which key starts a dictation the first time you dictate. \
             Change it later on the Hotkeys page.",
        );
        return true;
    }
    intro(
        ui,
        "Hold the key and speak, then let go: the words appear where you type. \
         Double-tap it to keep recording without holding, and press Escape to throw a \
         take away.",
    );
    hotkeys::key_row(
        gui,
        ui,
        Role::Dictate,
        "Click the key field, then press the key you want.",
    );
    true
}

fn try_it(gui: &mut Gui, ui: &mut egui::Ui) -> bool {
    let ask = gui.machine.running() && gui.setup.as_ref().is_some_and(|setup| !setup.asked_keep);
    if ask {
        // The hardware tune times the models on this take.
        std::thread::spawn(|| {
            let reply = crate::ipc::client::command_blocking(
                &DaemonCommand::KeepNext,
                Duration::from_secs(2),
            );
            debug!(
                kept = reply.is_some_and(|reply| reply.ok),
                "Asked the daemon to keep the next take"
            );
        });
        if let Some(setup) = gui.setup.as_mut() {
            setup.asked_keep = true;
        }
    }
    overview::daemon_row(gui, ui);
    overview::try_box(gui, ui);
    !gui.try_text.trim().is_empty()
}

fn tune(gui: &mut Gui, ui: &mut egui::Ui) -> bool {
    if gui.config.transcription.backend.is_cloud() {
        intro(
            ui,
            "A service transcribes for you, so this machine needs no tune.",
        );
        return true;
    }
    speech::tune(gui, ui);
    let pick = gui.machine.tune.as_ref().map(|tune| tune.pick.model);
    if let Some(pick) = pick.filter(|pick| gui.config.transcription.model_size != pick.to_string())
    {
        let title = format!("Use {}", speech::size_key(pick));
        row("setup.use_pick", &title)
            .describe("Downloads the model when it is not here yet, then uses it.")
            .reserve(120.0)
            .show(ui, |ui| {
                if ui
                    .add(Button::new("Use it").icon(icons::DOWNLOAD))
                    .clicked()
                {
                    gui.config.transcription.model_size = pick.to_string();
                    if !gui.machine.installed_models.contains(&pick) {
                        speech::start_download(gui, pick);
                    }
                }
            });
    }
    gui.machine.tune.is_some()
}

fn privacy(gui: &mut Gui, ui: &mut egui::Ui) -> bool {
    intro(
        ui,
        "Your voice stays on this machine unless you pick a cloud service on the Speech \
         page. These keep less, or check for more.",
    );
    switch_row(
        ui,
        "setup.history",
        "Keep a history of transcripts",
        "The History page searches them and types one again. Only you can read the file.",
        &mut gui.config.history.enabled,
    );
    if crate::updates::InstallKind::current().checks() {
        switch_row(
            ui,
            "setup.updates",
            "Check for new releases",
            "Once a day, ask GitHub for the newest release. The request sends only the Hush \
             version.",
            &mut gui.config.updates.check,
        );
    }
    true
}

fn finished(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Done");
    let description = format!(
        "Hold {} anywhere and speak. The pages in the sidebar hold every setting.",
        gui.saved.hotkey.combination
    );
    row("setup.done", "Hush is set up")
        .status(true)
        .describe(&description)
        .reserve(160.0)
        .show(ui, |ui| {
            if ui.add(Button::new("Open the overview")).clicked() {
                gui.go_to(Page::Overview);
            }
        });
}
