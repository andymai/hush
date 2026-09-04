//! What Hush still needs before it can type for you: keyboard access, a model,
//! and something to start it.

use super::theme::{hint, section, state_line};
use super::{megabytes, Gui, MODEL_ORDER};
use crate::transcription::models::{recommended_model, ModelManager, ModelSize};
use std::sync::Arc;

pub fn show(gui: &mut Gui, ui: &mut egui::Ui) {
    permissions(gui, ui);
    models(gui, ui);
    running(gui, ui);
}

fn permissions(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Keyboard access");
    let status = &gui.machine.permissions;
    state_line(
        ui,
        status.readable_event_nodes > 0,
        &format!(
            "Reads the keyboard ({} of {} devices)",
            status.readable_event_nodes, status.event_nodes
        ),
    );
    state_line(ui, status.uinput_writable, "Types into other windows");
    if status.ready() {
        hint(ui, "Nothing to do here.");
        return;
    }
    if status.in_container {
        hint(
            ui,
            "This is a container. Install the rule on the host, then restart Hush.",
        );
    }
    hint(
        ui,
        "One rule gives Hush the keyboard and the virtual keyboard it types with. \
         You will be asked for your password, and may need to log out once.",
    );
    if ui.button("Grant access").clicked() {
        gui.run_hush(&["setup", "permissions"], "Asking for permission…");
    }
}

fn models(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Speech model");
    hint(
        ui,
        "Runs on your machine. Larger models are more accurate and slower; \
         base is a good start, turbo or large if you have a GPU.",
    );

    let downloading = gui.download.lock().size;
    if let Some(size) = downloading {
        let (done, total) = {
            let download = gui.download.lock();
            (download.done, download.total)
        };
        let fraction = if total > 0 {
            done as f32 / total as f32
        } else {
            0.0
        };
        ui.add(egui::ProgressBar::new(fraction).text(format!(
            "{:?}: {} of {}",
            size,
            megabytes(done),
            megabytes(total)
        )));
        return;
    }

    let manager = ModelManager::new(crate::config::paths::models_dir()).ok();
    let Some(manager) = manager else {
        hint(ui, "The models directory is not readable.");
        return;
    };
    let selected: Option<ModelSize> = gui.config.transcription.model_size.parse().ok();
    let recommended = recommended_model();
    for size in MODEL_ORDER {
        let Some(info) = manager.get_model_info(&size) else {
            continue;
        };
        let installed = gui.machine.installed_models.contains(&size);
        ui.horizontal(|ui| {
            let is_selected = selected == Some(size);
            let label = if size == recommended {
                format!(
                    "{} ({}) — suggested for this machine",
                    info.name,
                    megabytes(info.expected_size)
                )
            } else {
                format!("{} ({})", info.name, megabytes(info.expected_size))
            };
            if ui
                .radio(is_selected && installed, label)
                .on_hover_text(if installed {
                    "Installed"
                } else {
                    "Not downloaded yet"
                })
                .clicked()
                && installed
            {
                gui.config.transcription.model_size = size_key(size).to_string();
                gui.save();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if installed {
                    state_line(ui, true, "Installed");
                } else if ui.button("Download").clicked() {
                    start_download(gui, size);
                }
            });
        });
    }
}

fn running(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Running");
    state_line(
        ui,
        gui.machine.daemon_running,
        if gui.machine.daemon_running {
            "Hush is listening for the hotkey"
        } else {
            "Hush is not running"
        },
    );
    ui.horizontal(|ui| {
        if gui.machine.daemon_running {
            if ui.button("Stop").clicked() {
                gui.run_hush(&["daemon", "stop"], "Stopping…");
            }
            if ui.button("Restart").clicked() {
                gui.run_hush(&["daemon", "restart"], "Restarting…");
            }
        } else if ui.button("Start Hush").clicked() {
            gui.run_hush(&["daemon", "start"], "Starting…");
        }
    });

    let mut autostart = gui.machine.autostart;
    if ui
        .checkbox(&mut autostart, "Start Hush when I log in")
        .changed()
    {
        if autostart {
            gui.run_hush(&["install", "--autostart"], "Added to your login.");
        } else {
            gui.run_hush(&["uninstall", "--autostart"], "Removed from your login.");
        }
    }
    hint(
        ui,
        &format!(
            "Hold {} to dictate once Hush is running.",
            gui.config.hotkey.combination
        ),
    );
}

fn start_download(gui: &mut Gui, size: ModelSize) {
    {
        let mut download = gui.download.lock();
        download.size = Some(size);
        download.done = 0;
        download.total = 0;
        download.finished = None;
    }
    let shared = Arc::clone(&gui.download);
    let models_dir = crate::config::paths::models_dir();
    gui.runtime.spawn(async move {
        let result = match ModelManager::new(&models_dir) {
            Ok(manager) => {
                let progress = shared.clone();
                manager
                    .ensure_model_downloaded_with(&size, &move |done, total| {
                        let mut download = progress.lock();
                        download.done = done;
                        download.total = total;
                    })
                    .await
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            },
            Err(e) => Err(e.to_string()),
        };
        shared.lock().finished = Some(result);
    });
    gui.say("Downloading…");
}

/// The name this model has in `transcription.model_size`.
pub fn size_key(size: ModelSize) -> &'static str {
    match size {
        ModelSize::Tiny => "tiny",
        ModelSize::Base => "base",
        ModelSize::Small => "small",
        ModelSize::Medium => "medium",
        ModelSize::Large => "large",
        ModelSize::LargeV2 => "large-v2",
        ModelSize::LargeV3 => "large-v3",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_model_name_parses_back() {
        for size in MODEL_ORDER {
            let key = size_key(size);
            assert_eq!(
                key.parse::<ModelSize>().ok(),
                Some(size),
                "{key} round trips"
            );
        }
    }
}
