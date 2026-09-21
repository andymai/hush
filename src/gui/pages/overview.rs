//! Whether Hush can work on this machine, and a box to try it in.

use super::Page;
use crate::gui::theme::{dot, hint, row, section, state_line, toggle, GOOD, TOGGLE_COLUMN};
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
    section(ui, "Hush");
    let running = gui.machine.running();
    let (title, description) = if running {
        ("Hush is running", "Listening for the hotkey.")
    } else {
        ("Hush is not running", "Start it to dictate.")
    };
    row(title)
        .status(running)
        .describe(description)
        .show(ui, |ui| {
            if running {
                if ui.button("Restart").clicked() {
                    gui.run_hush(&["daemon", "restart"], "Restarting…");
                }
                if ui.button("Stop").clicked() {
                    gui.run_hush(&["daemon", "stop"], "Stopping…");
                }
            } else if ui.button("Start Hush").clicked() {
                gui.run_hush(&["daemon", "start"], "Starting…");
            }
        });
    let mut autostart = gui.machine.autostart;
    row("Start Hush when I log in")
        .describe("Adds Hush to your desktop session's autostart.")
        .reserve(TOGGLE_COLUMN)
        .show(ui, |ui| {
            if toggle(ui, &mut autostart).changed() {
                if autostart {
                    gui.run_hush(&["install", "--autostart"], "Added to your login.");
                } else {
                    gui.run_hush(&["uninstall", "--autostart"], "Removed from your login.");
                }
            }
        });
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
    row(if ready {
        "Hush can read and type"
    } else {
        "Hush cannot read the keyboard yet"
    })
    .status(ready)
    .describe(&description)
    .show(ui, |ui| {
        if !ready && ui.button("Grant access").clicked() {
            gui.run_hush(&["setup", "permissions"], "Asking for permission…");
        }
    });
}

fn model(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Speech model");
    let chosen: Option<ModelSize> = gui.config.transcription.model_size.parse().ok();
    let installed = chosen.is_some_and(|size| gui.machine.installed_models.contains(&size));
    let title = if installed {
        format!("{} is installed", gui.config.transcription.model_size)
    } else if gui.machine.installed_models.is_empty() {
        "No model downloaded yet".to_string()
    } else {
        format!("{} is not downloaded", gui.config.transcription.model_size)
    };
    row(&title)
        .status(installed)
        .describe("Runs on your machine. Larger models are more accurate and slower.")
        .show(ui, |ui| {
            if ui.button("Choose a model").clicked() {
                gui.page = Page::Speech;
            }
        });
}

fn try_it(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Try it");
    hint(
        ui,
        &format!(
            "Click in the box, hold {}, and speak. The words land here.",
            gui.saved.hotkey.combination
        ),
    );
    ui.add_space(4.0);
    ui.add(
        egui::TextEdit::multiline(&mut gui.try_text)
            .desired_rows(3)
            .desired_width(f32::INFINITY)
            .margin(egui::vec2(8.0, 6.0))
            .hint_text("Nothing yet"),
    );
    ui.horizontal(|ui| {
        match gui.machine.daemon {
            None => state_line(ui, false, "Start Hush first."),
            Some(DaemonState::Recording { elapsed_ms }) => {
                dot(ui, GOOD);
                ui.label(format!("Listening… {:.1} s", elapsed_ms as f64 / 1000.0));
            },
            Some(DaemonState::Transcribing) => {
                ui.spinner();
                ui.label("Transcribing…");
            },
            Some(DaemonState::Inserting) => {
                ui.spinner();
                ui.label("Typing…");
            },
            Some(DaemonState::Stopping) => {
                ui.spinner();
                ui.label("Stopping…");
            },
            Some(DaemonState::Idle) => {
                if gui.machine.permissions.ready() {
                    hint(ui, "Ready.");
                } else {
                    state_line(ui, false, "Grant keyboard access first.");
                }
            },
        }
        if !gui.try_text.is_empty() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Clear").clicked() {
                    gui.try_text.clear();
                }
            });
        }
    });
}
