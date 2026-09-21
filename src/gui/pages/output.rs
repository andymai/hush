//! How the words reach the application: typed key by key, or pasted.

use crate::gui::theme::{row, slider, FIELD_WIDTH};
use crate::gui::Gui;
use crate::text::InsertionMethod;

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
}

fn method_name(method: InsertionMethod) -> &'static str {
    match method {
        InsertionMethod::Auto => "Typing, pasting when it cannot",
        InsertionMethod::Uinput => "Always typing",
        InsertionMethod::Clipboard => "Always pasting",
    }
}
