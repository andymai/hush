//! Palette, type, and layout shared by every page: settings rows in the
//! style of Warp's preferences, a toggle switch, and the colours the window
//! shares with the overlay and the tray, in the system's light or dark theme.

use egui::{Color32, Context, FontFamily, FontId, Rounding, Stroke, TextStyle};

pub const ACCENT: Color32 = Color32::from_rgb(0x6b, 0x5b, 0xdb);
pub const GOOD: Color32 = Color32::from_rgb(0x3f, 0xa9, 0x5c);
pub const WARN: Color32 = Color32::from_rgb(0xd9, 0x8f, 0x28);
pub const BAD: Color32 = Color32::from_rgb(0xd9, 0x3c, 0x33);
pub const MUTED: Color32 = Color32::from_rgb(0x8a, 0x90, 0x99);

/// Width of a text field or dropdown.
pub const FIELD_WIDTH: f32 = 200.0;
/// Width kept free at the right of a settings row for a dropdown or field.
pub const CONTROL_COLUMN: f32 = 230.0;
/// Height of every button, field, and dropdown.
pub const CONTROL_HEIGHT: f32 = 26.0;
const ROUNDING: f32 = 6.0;

/// The surfaces and strokes of one colour scheme.
pub struct Palette {
    pub panel: Color32,
    pub nav: Color32,
    pub raised: Color32,
    pub raised_hover: Color32,
    pub pressed: Color32,
    pub field: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text: Color32,
    pub text_strong: Color32,
}

const DARK: Palette = Palette {
    panel: Color32::from_rgb(0x1c, 0x1c, 0x20),
    nav: Color32::from_rgb(0x16, 0x16, 0x19),
    raised: Color32::from_rgb(0x2a, 0x2b, 0x31),
    raised_hover: Color32::from_rgb(0x34, 0x35, 0x3c),
    pressed: Color32::from_rgb(0x3d, 0x3e, 0x46),
    field: Color32::from_rgb(0x13, 0x13, 0x16),
    border: Color32::from_rgb(0x30, 0x31, 0x38),
    border_strong: Color32::from_rgb(0x4a, 0x4b, 0x54),
    text: Color32::from_rgb(0xd7, 0xd8, 0xdc),
    text_strong: Color32::from_rgb(0xff, 0xff, 0xff),
};

const LIGHT: Palette = Palette {
    panel: Color32::from_rgb(0xf6, 0xf6, 0xf8),
    nav: Color32::from_rgb(0xec, 0xec, 0xf0),
    raised: Color32::from_rgb(0xff, 0xff, 0xff),
    raised_hover: Color32::from_rgb(0xf0, 0xf0, 0xf3),
    pressed: Color32::from_rgb(0xe4, 0xe4, 0xe9),
    field: Color32::from_rgb(0xff, 0xff, 0xff),
    border: Color32::from_rgb(0xd8, 0xd8, 0xde),
    border_strong: Color32::from_rgb(0xb4, 0xb4, 0xbd),
    text: Color32::from_rgb(0x22, 0x22, 0x27),
    text_strong: Color32::from_rgb(0x00, 0x00, 0x00),
};

pub fn palette(ctx: &Context) -> &'static Palette {
    if ctx.style().visuals.dark_mode {
        &DARK
    } else {
        &LIGHT
    }
}

pub fn apply(ctx: &Context) {
    let mut style = (*ctx.style()).clone();
    let palette = if style.visuals.dark_mode {
        &DARK
    } else {
        &LIGHT
    };

    style.text_styles = [
        (
            TextStyle::Small,
            FontId::new(11.5, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Heading,
            FontId::new(20.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();

    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 4.0);
    style.spacing.interact_size = egui::vec2(40.0, CONTROL_HEIGHT);
    style.spacing.slider_width = 140.0;
    style.spacing.combo_width = FIELD_WIDTH;
    style.spacing.icon_width = 18.0;
    style.spacing.icon_width_inner = 10.0;
    style.spacing.indent = 18.0;

    let rounding = Rounding::same(ROUNDING);
    let visuals = &mut style.visuals;
    visuals.panel_fill = palette.panel;
    visuals.window_fill = palette.panel;
    visuals.extreme_bg_color = palette.field;
    visuals.faint_bg_color = palette.raised;
    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.hyperlink_color = ACCENT;
    visuals.window_rounding = rounding;
    visuals.menu_rounding = rounding;
    visuals.slider_trailing_fill = true;

    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_fill = palette.raised;
    widgets.noninteractive.weak_bg_fill = palette.raised;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, palette.border);
    widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, palette.text);
    widgets.noninteractive.rounding = rounding;

    widgets.inactive.bg_fill = palette.field;
    widgets.inactive.weak_bg_fill = palette.raised;
    widgets.inactive.bg_stroke = Stroke::new(1.0_f32, palette.border_strong);
    widgets.inactive.fg_stroke = Stroke::new(1.0_f32, palette.text);
    widgets.inactive.rounding = rounding;
    widgets.inactive.expansion = 0.0;

    widgets.hovered.bg_fill = palette.raised_hover;
    widgets.hovered.weak_bg_fill = palette.raised_hover;
    widgets.hovered.bg_stroke = Stroke::new(1.0_f32, palette.border_strong);
    widgets.hovered.fg_stroke = Stroke::new(1.5_f32, palette.text_strong);
    widgets.hovered.rounding = rounding;
    widgets.hovered.expansion = 0.0;

    widgets.active.bg_fill = palette.pressed;
    widgets.active.weak_bg_fill = palette.pressed;
    widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    widgets.active.fg_stroke = Stroke::new(2.0_f32, palette.text_strong);
    widgets.active.rounding = rounding;
    widgets.active.expansion = 0.0;

    widgets.open.bg_fill = palette.raised_hover;
    widgets.open.weak_bg_fill = palette.raised_hover;
    widgets.open.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    widgets.open.fg_stroke = Stroke::new(1.0_f32, palette.text_strong);
    widgets.open.rounding = rounding;
    widgets.open.expansion = 0.0;

    ctx.set_style(style);
}

/// A heading with breathing room above it.
pub fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(18.0);
    ui.label(egui::RichText::new(title).strong().size(15.0));
    ui.add_space(2.0);
}

/// One line of explanation, wrapped to the page.
pub fn hint(ui: &mut egui::Ui, text: &str) {
    ui.add(egui::Label::new(egui::RichText::new(text).size(12.5).color(MUTED)).wrap());
}

/// A small filled circle, painted rather than typed because the bundled
/// font has no glyph for it.
pub fn dot(ui: &mut egui::Ui, colour: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 18.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.0, colour);
}

/// A green or amber dot with a label.
pub fn state_line(ui: &mut egui::Ui, ok: bool, text: &str) {
    ui.horizontal(|ui| {
        dot(ui, if ok { GOOD } else { WARN });
        ui.label(text);
    });
}

/// A text field of the standard width and height.
pub fn field(ui: &mut egui::Ui, value: &mut String, placeholder: &str) -> egui::Response {
    ui.add_sized(
        [FIELD_WIDTH, CONTROL_HEIGHT],
        egui::TextEdit::singleline(value)
            .hint_text(placeholder)
            .margin(egui::vec2(8.0, 4.0)),
    )
}

/// A button whose label reads as a secondary action beside a control.
pub fn quiet_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(egui::Button::new(egui::RichText::new(label).color(MUTED)).frame(false))
}

/// A switch: accent when on, a knob that slides across.
pub fn toggle(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let (rect, mut response) = ui.allocate_exact_size(egui::vec2(40.0, 22.0), egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *on, "")
    });
    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool_responsive(response.id, *on);
        let off = ui.visuals().widgets.hovered.bg_stroke.color;
        let mut track = if *on { ACCENT } else { off };
        let mut knob = Color32::WHITE;
        if !ui.is_enabled() {
            track = track.gamma_multiply(0.5);
            knob = knob.gamma_multiply(0.6);
        }
        let radius = rect.height() / 2.0;
        ui.painter().rect_filled(rect, radius, track);
        let x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        ui.painter()
            .circle_filled(egui::pos2(x, rect.center().y), radius - 3.0, knob);
    }
    response
}

/// One entry in the sidebar: a soft accent pill when selected.
pub fn nav_item(ui: &mut egui::Ui, selected: bool, label: &str) -> egui::Response {
    let size = egui::vec2(ui.available_width(), 30.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let fill = if selected {
            ACCENT.gamma_multiply(0.35)
        } else if response.hovered() {
            ui.visuals().widgets.hovered.weak_bg_fill
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, ROUNDING, fill);
        let colour = if selected {
            ui.visuals().strong_text_color()
        } else {
            ui.visuals().text_color()
        };
        ui.painter().text(
            rect.left_center() + egui::vec2(10.0, 0.0),
            egui::Align2::LEFT_CENTER,
            label,
            TextStyle::Body.resolve(ui.style()),
            colour,
        );
    }
    response
}

/// A slider with its value box after the track, the width of a field.
pub fn slider(ui: &mut egui::Ui, slider: egui::Slider<'_>) -> egui::Response {
    ui.allocate_ui_with_layout(
        egui::vec2(FIELD_WIDTH + 10.0, CONTROL_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| ui.add(slider),
    )
    .inner
}

/// Width a toggle needs at the right of a row.
pub const TOGGLE_COLUMN: f32 = 70.0;

/// A row whose only control is a switch.
pub fn toggle_row(
    ui: &mut egui::Ui,
    title: &str,
    description: &str,
    on: &mut bool,
) -> egui::Response {
    row(title)
        .describe(description)
        .reserve(TOGGLE_COLUMN)
        .show(ui, |ui| toggle(ui, on))
}

/// A settings row: title and description on the left, the control on the
/// right, a rule underneath.
pub struct Row<'a> {
    title: &'a str,
    description: Option<&'a str>,
    error: Option<&'a str>,
    status: Option<bool>,
    reserve: f32,
}

pub fn row(title: &str) -> Row<'_> {
    Row {
        title,
        description: None,
        error: None,
        status: None,
        reserve: CONTROL_COLUMN,
    }
}

impl<'a> Row<'a> {
    pub fn describe(mut self, text: &'a str) -> Self {
        self.description = Some(text);
        self
    }

    /// Red text under the description when the value cannot be used.
    pub fn problem(mut self, text: Option<&'a str>) -> Self {
        self.error = text;
        self
    }

    /// A green or amber dot before the title.
    pub fn status(mut self, ok: bool) -> Self {
        self.status = Some(ok);
        self
    }

    /// How much width the control needs; the text takes the rest.
    pub fn reserve(mut self, width: f32) -> Self {
        self.reserve = width;
        self
    }

    pub fn show<R>(self, ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
        ui.add_space(6.0);
        let inner = ui
            .horizontal(|ui| {
                let text_width = (ui.available_width() - self.reserve).max(140.0);
                ui.vertical(|ui| {
                    ui.set_max_width(text_width);
                    ui.spacing_mut().item_spacing.y = 3.0;
                    match self.status {
                        Some(ok) => state_line(ui, ok, self.title),
                        None => {
                            ui.label(self.title);
                        },
                    }
                    if let Some(text) = self.description {
                        hint(ui, text);
                    }
                    if let Some(text) = self.error {
                        ui.add(
                            egui::Label::new(egui::RichText::new(text).size(12.5).color(BAD))
                                .wrap(),
                        );
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add)
                    .inner
            })
            .inner;
        ui.add_space(6.0);
        ui.separator();
        inner
    }
}
