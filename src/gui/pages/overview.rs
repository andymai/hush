//! Whether Hush can work on this machine, and a box to try it in.

use super::speech::server_status;
use super::Page;
use crate::config::settings::TranscriptionBackend;
use crate::gui::icons;
use crate::gui::theme::{palette, CONTROL_HEIGHT};
use crate::gui::widgets::{self, intro, row, section, switch, text_area, Button, Size, Variant};
use crate::gui::Gui;
use crate::ipc::DaemonState;
use crate::transcription::models::ModelSize;

pub fn show(gui: &mut Gui, ui: &mut egui::Ui) {
    running(gui, ui);
    access(gui, ui);
    model(gui, ui);
    try_it(gui, ui);
}

fn running(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Status");
    let running = gui.machine.running();
    let safe_mode = gui
        .machine
        .safe_mode
        .clone()
        .filter(|_| running)
        .map(|reason| safe_mode_text(&reason));
    let (title, description) = match (running, &safe_mode) {
        (true, Some(_)) => ("Hush runs in safe mode", "Listening for the hotkey."),
        (true, None) => ("Hush is running", "Listening for the hotkey."),
        (false, _) => ("Hush is not running", "Start it to dictate."),
    };
    row("overview.daemon", title)
        .status(running && safe_mode.is_none())
        .describe(description)
        .caution(safe_mode.as_deref())
        .reserve(200.0)
        .show(ui, |ui| {
            if running {
                if ui
                    .add(Button::new("Restart").icon(icons::ROTATE_CW))
                    .clicked()
                {
                    gui.run_hush(&["daemon", "restart"], "Restarting…");
                }
                if ui
                    .add(Button::new("Stop").icon(icons::CIRCLE_STOP))
                    .clicked()
                {
                    gui.run_hush(&["daemon", "stop"], "Stopping…");
                }
            } else if ui
                .add(
                    Button::new("Start Hush")
                        .icon(icons::POWER)
                        .variant(Variant::Primary),
                )
                .clicked()
            {
                gui.run_hush(&["daemon", "start"], "Starting…");
            }
        });
    let mut autostart = gui.machine.autostart;
    row("overview.autostart", "Start Hush when I log in")
        .describe("Adds Hush to your desktop session's autostart.")
        .reserve(36.0)
        .show(ui, |ui| {
            if switch(ui, &mut autostart).changed() {
                if autostart {
                    gui.run_hush(&["install", "--autostart"], "Added to your login.");
                } else {
                    gui.run_hush(&["uninstall", "--autostart"], "Removed from your login.");
                }
            }
        });
}

/// What safe mode means in the window, with its reason.
pub fn safe_mode_text(reason: &str) -> String {
    format!(
        "Hush writes and deletes nothing in the history and the kept audio, because {}. \
         It restarts on its own once the file loads.",
        reason
    )
}

fn access(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Keyboard access");
    let status = &gui.machine.permissions;
    let ready = status.ready();
    let description = if ready {
        format!(
            "Reads the keyboard ({} of {} devices) and types into other windows.",
            status.readable_event_nodes, status.event_nodes
        )
    } else {
        let mut text = String::from(
            "One rule gives Hush the keyboard and the virtual keyboard it types with. \
             You will be asked for your password, and may need to log out once.",
        );
        if status.in_container {
            text.push_str(" This is a container: install the rule on the host, then restart Hush.");
        }
        text
    };
    let title = if ready {
        "Hush can read and type"
    } else {
        "Hush cannot read the keyboard yet"
    };
    row("overview.access", title)
        .status(ready)
        .describe(&description)
        .reserve(if ready { 0.0 } else { 150.0 })
        .show(ui, |ui| {
            if !ready
                && ui
                    .add(Button::new("Grant access").icon(icons::SHIELD_CHECK))
                    .clicked()
            {
                gui.run_hush(&["setup", "permissions"], "Asking for permission…");
            }
        });
}

fn model(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Speech model");
    let backend = gui.config.transcription.backend;
    if backend == TranscriptionBackend::OpenAiCompatible {
        let (ok, title, description) = server_status(gui);
        row("overview.model", &title)
            .status(ok)
            .describe(&description)
            .reserve(100.0)
            .show(ui, |ui| {
                if widgets::button(ui, "Change").clicked() {
                    gui.page = Page::Speech;
                }
            });
        return;
    }
    if let Some(var) = backend.key_var() {
        let has_key = gui.machine.has_key(&gui.config.transcription);
        let (title, description) = if has_key {
            (
                format!("{} transcribes your speech", backend.name()),
                format!("The audio of each dictation goes to {}.", backend.name()),
            )
        } else {
            (
                format!("{} needs an API key", backend.name()),
                format!("Put {} in ~/.config/hush/.env.", var),
            )
        };
        row("overview.model", &title)
            .status(has_key)
            .describe(&description)
            .reserve(100.0)
            .show(ui, |ui| {
                if widgets::button(ui, "Change").clicked() {
                    gui.page = Page::Speech;
                }
            });
        return;
    }
    let chosen: Option<ModelSize> = gui.config.transcription.model_size.parse().ok();
    let installed = chosen.is_some_and(|size| gui.machine.installed_models.contains(&size));
    let title = if installed {
        format!("{} is installed", gui.config.transcription.model_size)
    } else if gui.machine.installed_models.is_empty() {
        "No model downloaded yet".to_string()
    } else {
        format!("{} is not downloaded", gui.config.transcription.model_size)
    };
    row("overview.model", &title)
        .status(installed)
        .describe("Runs on your machine. Larger models are more accurate and slower.")
        .reserve(140.0)
        .show(ui, |ui| {
            if widgets::button(ui, "Choose a model").clicked() {
                gui.page = Page::Speech;
            }
        });
}

fn try_it(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Try it");
    intro(
        ui,
        &format!(
            "Click in the box, hold {}, and speak. The words land here.",
            gui.saved.hotkey.combination
        ),
    );
    let width = ui.available_width();
    text_area(ui, &mut gui.try_text, "Nothing yet", 3, width);
    ui.add_space(4.0);
    let colours = palette(ui.ctx());
    ui.allocate_ui_with_layout(
        egui::vec2(width, CONTROL_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            match gui.machine.daemon {
                None => widgets::status(ui, icons::CIRCLE_ALERT, colours.warn, "Start Hush first."),
                Some(DaemonState::Starting) => widgets::busy(ui, "Starting the microphone…"),
                Some(DaemonState::Recording { elapsed_ms }) => widgets::status(
                    ui,
                    icons::MIC,
                    colours.accent,
                    &format!("Listening… {:.1} s", elapsed_ms as f64 / 1000.0),
                ),
                Some(DaemonState::Transcribing) => widgets::busy(ui, "Transcribing…"),
                Some(DaemonState::Inserting) => widgets::busy(ui, "Typing…"),
                Some(DaemonState::Stopping) => widgets::busy(ui, "Stopping…"),
                Some(DaemonState::Idle) => {
                    if gui.machine.permissions.ready() {
                        widgets::status(ui, icons::CIRCLE_CHECK, colours.good, "Ready.");
                    } else {
                        widgets::status(
                            ui,
                            icons::CIRCLE_ALERT,
                            colours.warn,
                            "Grant keyboard access first.",
                        );
                    }
                },
            }
            if !gui.try_text.is_empty() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let clear = Button::new("Clear")
                        .icon(icons::X)
                        .variant(Variant::Quiet)
                        .size(Size::Small);
                    if ui.add(clear).clicked() {
                        gui.try_text.clear();
                    }
                });
            }
        },
    );
}
