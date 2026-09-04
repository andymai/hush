//! Spacing and accent shared by every pane, so the window reads as one thing
//! next to the overlay and the tray.

use egui::{Color32, Context};

pub const ACCENT: Color32 = Color32::from_rgb(0x6b, 0x5b, 0xdb);
pub const GOOD: Color32 = Color32::from_rgb(0x3f, 0xa9, 0x5c);
pub const WARN: Color32 = Color32::from_rgb(0xd9, 0x8f, 0x28);
pub const MUTED: Color32 = Color32::from_rgb(0x8a, 0x90, 0x99);

pub fn apply(ctx: &Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.visuals.selection.bg_fill = ACCENT;
    style.visuals.hyperlink_color = ACCENT;
    ctx.set_style(style);
}

/// A heading with a little breathing room above it.
pub fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(10.0);
    ui.label(egui::RichText::new(title).strong().size(15.0));
    ui.add_space(2.0);
}

/// One line of explanation under a control.
pub fn hint(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).size(11.5).color(MUTED));
}

/// A green tick or an amber dot, with a label.
pub fn state_line(ui: &mut egui::Ui, ok: bool, text: &str) {
    ui.horizontal(|ui| {
        let (mark, colour) = if ok { ("●", GOOD) } else { ("●", WARN) };
        ui.label(egui::RichText::new(mark).color(colour));
        ui.label(text);
    });
}
