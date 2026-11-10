//! How the words reach the application: typed key by key, or pasted, with
//! rules for applications that need one or the other.

use crate::config::settings::MAX_TYPING_DELAY_MS;
use crate::gui::icons;
use crate::gui::theme::FIELD_WIDTH;
use crate::gui::widgets::{
    dropdown, icon_button, intro, list, list_item, number, option, row, section, submitted, Button,
    Field,
};
use crate::gui::Gui;
use crate::text::InsertionMethod;

const ADD_COLUMN: f32 = FIELD_WIDTH + 90.0;

pub fn show(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "Typing");
    row("output.method", "Insert text by")
        .describe(
            "Typing works everywhere, including password fields and games. \
             Pasting handles emoji, other alphabets, and very long text.",
        )
        .reserve(240.0)
        .show(ui, |ui| {
            let method = &mut gui.config.insertion.method;
            dropdown(ui, "insertion_method", 240.0, method_name(*method), |ui| {
                for choice in [
                    InsertionMethod::Auto,
                    InsertionMethod::Uinput,
                    InsertionMethod::Clipboard,
                ] {
                    option(ui, method, choice, method_name(choice));
                }
            });
        });
    row("output.typing_delay", "Between keystrokes")
        .describe("Raise it if an application drops letters.")
        .reserve(110.0)
        .show(ui, |ui| {
            let mut delay = gui.config.insertion.typing_delay_ms;
            if number(ui, &mut delay, 0..=MAX_TYPING_DELAY_MS, "ms").changed() {
                gui.config.insertion.typing_delay_ms = delay;
            }
        });
    by_application(gui, ui);
}

fn by_application(gui: &mut Gui, ui: &mut egui::Ui) {
    section(ui, "By application");
    intro(
        ui,
        "These rules come before the setting above. Part of a window class is \
         enough: konsole matches org.kde.konsole. The History page shows the \
         class of each application you dictate into.",
    );
    app_list(
        ui,
        "output.uinput_apps",
        "Always type in",
        "For applications where a paste does nothing.",
        &mut gui.config.insertion.uinput_apps,
        &mut gui.new_uinput_app,
    );
    app_list(
        ui,
        "output.clipboard_apps",
        "Always paste in",
        "For applications that drop or reorder typed letters.",
        &mut gui.config.insertion.clipboard_apps,
        &mut gui.new_clipboard_app,
    );
}

fn app_list(
    ui: &mut egui::Ui,
    key: &str,
    title: &str,
    description: &str,
    apps: &mut Vec<String>,
    draft: &mut String,
) {
    let mut add = false;
    row(key, title)
        .describe(description)
        .reserve(ADD_COLUMN)
        .show(ui, |ui| {
            let button = Button::new("Add")
                .icon(icons::PLUS)
                .enabled(!draft.trim().is_empty());
            add |= ui.add(button).clicked();
            let response = ui.add(Field::new(draft, "window class").mono());
            add |= submitted(ui, &response);
            if add {
                response.request_focus();
            }
        });
    if add {
        add_app(apps, draft);
        draft.clear();
    }
    let mut remove = None;
    list(ui, |ui| {
        for (index, app) in apps.iter().enumerate() {
            let item_key = format!("{}.{}", key, app);
            list_item(&item_key, icons::APP_WINDOW, app)
                .mono()
                .show(ui, 24.0, |ui| {
                    if icon_button(ui, icons::X, &format!("Remove {}", app)).clicked() {
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
