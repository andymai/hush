//! How the words reach the application: typed key by key, or pasted, with
//! rules for applications that need one or the other.

use crate::gui::theme::{field, hint, quiet_button, row, section, slider, FIELD_WIDTH};
use crate::gui::Gui;
use crate::text::InsertionMethod;

const ADD_COLUMN: f32 = FIELD_WIDTH + 80.0;

pub fn show(gui: &mut Gui, ui: &mut egui::Ui) {
    ui.add_space(6.0);
    row("Insert text by")
        .describe(
            "Typing works everywhere, including password fields and games. \
             Pasting handles emoji, other alphabets, and very long text.",
        )
        .show(ui, |ui| {
            egui::ComboBox::from_id_salt("insertion_method")
                .width(FIELD_WIDTH)
                .selected_text(method_name(gui.config.insertion.method))
                .show_ui(ui, |ui| {
                    for method in [
                        InsertionMethod::Auto,
                        InsertionMethod::Uinput,
                        InsertionMethod::Clipboard,
                    ] {
                        ui.selectable_value(
                            &mut gui.config.insertion.method,
                            method,
                            method_name(method),
                        );
                    }
                });
        });
    row("Between keystrokes")
        .describe("Raise it if an application drops letters.")
        .show(ui, |ui| {
            slider(
                ui,
                egui::Slider::new(&mut gui.config.insertion.typing_delay_ms, 0..=50).suffix(" ms"),
            );
        });
    by_application(gui, ui);
}

fn by_application(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "By application");
    hint(
        ui,
        "These rules come before the setting above. Part of a window class is \
         enough: konsole matches org.kde.konsole. The History page shows the \
         class of each application you dictate into.",
    );
    app_list(
        ui,
        "Always type in",
        "For applications where a paste does nothing.",
        &mut gui.config.insertion.uinput_apps,
        &mut gui.new_uinput_app,
    );
    app_list(
        ui,
        "Always paste in",
        "For applications that drop or reorder typed letters.",
        &mut gui.config.insertion.clipboard_apps,
        &mut gui.new_clipboard_app,
    );
}

fn app_list(
    ui: &mut egui::Ui,
    title: &str,
    description: &str,
    apps: &mut Vec<String>,
    draft: &mut String,
) {
    let mut add = false;
    row(title)
        .describe(description)
        .reserve(ADD_COLUMN)
        .show(ui, |ui| {
            add |= ui
                .add_enabled(!draft.trim().is_empty(), egui::Button::new("Add"))
                .clicked();
            let response = field(ui, draft, "window class");
            add |= response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if add {
                response.request_focus();
            }
        });
    if add {
        add_app(apps, draft);
        draft.clear();
    }
    let mut remove = None;
    ui.indent(title, |ui| {
        for (index, app) in apps.iter().enumerate() {
            row(app).show(ui, |ui| {
                if quiet_button(ui, "Remove").clicked() {
                    remove = Some(index);
                }
            });
        }
    });
    if let Some(index) = remove {
        apps.remove(index);
    }
}

/// Add a trimmed class unless it is blank or already listed in any case.
fn add_app(apps: &mut Vec<String>, draft: &str) -> bool {
    let app = draft.trim();
    if app.is_empty() || apps.iter().any(|kept| kept.eq_ignore_ascii_case(app)) {
        return false;
    }
    apps.push(app.to_string());
    true
}

fn method_name(method: InsertionMethod) -> &'static str {
    match method {
        InsertionMethod::Auto => "Typing, pasting when it cannot",
        InsertionMethod::Uinput => "Always typing",
        InsertionMethod::Clipboard => "Always pasting",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_are_trimmed_and_listed_once() {
        let mut apps = Vec::new();
        assert!(add_app(&mut apps, "  konsole "));
        assert!(!add_app(&mut apps, "Konsole"), "same class in another case");
        assert!(!add_app(&mut apps, "   "));
        assert!(add_app(&mut apps, "kitty"));
        assert_eq!(apps, ["konsole", "kitty"]);
    }
}
