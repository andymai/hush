//! The controls every page draws: settings rows with a title and a
//! description on the left and the control on the right, section labels,
//! buttons, switches, steppers, segmented controls, fields, dropdowns, the
//! key capture field, meters, and list items.
//!
//! Each settings row has a stable key, such as `speech.model`, so a search
//! over the settings can find and reach it.

use super::icons;
use super::theme::{self, palette, Palette, Split};
use egui::text::LayoutJob;
use egui::{
    Align, Align2, Color32, FontId, Galley, Layout, Pos2, Rect, Response, Rounding, Sense, Stroke,
    Ui, UiBuilder, Vec2, Widget, WidgetInfo, WidgetType,
};
use std::ops::RangeInclusive;
use std::sync::Arc;

/// Text laid out for painting, with the line height it should take.
struct Text {
    galley: Arc<Galley>,
    line: f32,
}

impl Text {
    fn new(ui: &Ui, text: &str, font: FontId, color: Color32, width: f32, line: f32) -> Self {
        let job = LayoutJob::simple(text.to_owned(), font, color, width.max(1.0));
        Self {
            galley: ui.fonts(|fonts| fonts.layout_job(job)),
            line,
        }
    }

    fn rows(&self) -> usize {
        self.galley.rows.len().max(1)
    }

    /// The height with the line height the type scale asks for: the font's
    /// own rows, plus the leading split above and below the block.
    fn height(&self) -> f32 {
        self.galley.size().y + self.leading() * 2.0
    }

    fn leading(&self) -> f32 {
        let row = self.galley.size().y / self.rows() as f32;
        ((self.line - row) / 2.0).max(0.0)
    }

    fn paint(&self, ui: &Ui, at: Pos2) {
        ui.painter().galley(
            at + Vec2::new(0.0, self.leading()),
            Arc::clone(&self.galley),
            Color32::PLACEHOLDER,
        );
    }
}

/// Paint `icon` centred on `centre`.
pub fn paint_icon(ui: &Ui, centre: Pos2, icon: char, size: f32, colour: Color32) {
    ui.painter().text(
        centre,
        Align2::CENTER_CENTER,
        icon,
        theme::icon_font(size),
        colour,
    );
}

/// An icon in the flow of a horizontal layout.
pub fn icon(ui: &mut Ui, icon: char, colour: Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(theme::ICON), Sense::hover());
    paint_icon(ui, rect.center(), icon, theme::ICON, colour);
    response
}

/// The focus ring: 2 points of accent, 2 points outside the control.
pub fn focus_ring(ui: &Ui, rect: Rect, rounding: f32) {
    ui.painter().rect_stroke(
        rect.expand(2.0),
        rounding + 2.0,
        Stroke::new(2.0_f32, palette(ui.ctx()).accent),
    );
}

/// A small label over the rows of a section, with room above it for the
/// section before.
pub fn section(ui: &mut Ui, title: &str) {
    if ui.min_rect().height() > 0.0 {
        ui.add_space(theme::SECTION_GAP);
    }
    let colours = palette(ui.ctx());
    let text = Text::new(
        ui,
        title,
        theme::medium(theme::LABEL),
        colours.muted,
        ui.available_width(),
        14.0,
    );
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), text.height() + 4.0),
        Sense::hover(),
    );
    text.paint(ui, rect.min);
}

/// A paragraph of help under a section label, before its rows.
pub fn intro(ui: &mut Ui, text: &str) {
    let colours = palette(ui.ctx());
    paragraph(ui, text, colours.muted);
    ui.add_space(8.0);
}

fn paragraph(ui: &mut Ui, text: &str, colour: Color32) {
    let width = ui.available_width();
    let text = Text::new(
        ui,
        text,
        theme::sans(theme::SMALL),
        colour,
        width,
        theme::SMALL_LINE,
    );
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, text.height()), Sense::hover());
    text.paint(ui, rect.min);
}

/// A line with an icon before it, such as a caution under a section label.
pub fn notice(ui: &mut Ui, icon: char, colour: Color32, text: &str) {
    let width = ui.available_width();
    let body = Text::new(
        ui,
        text,
        theme::sans(theme::SMALL),
        colour,
        width - 20.0,
        theme::SMALL_LINE,
    );
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, body.height()), Sense::hover());
    paint_icon(
        ui,
        rect.min + Vec2::new(7.0, theme::SMALL_LINE / 2.0),
        icon,
        14.0,
        colour,
    );
    body.paint(ui, rect.min + Vec2::new(20.0, 0.0));
}

/// Muted text where a list has nothing to show.
pub fn empty(ui: &mut Ui, text: &str) {
    ui.add_space(theme::ROW_PADDING);
    let colours = palette(ui.ctx());
    paragraph(ui, text, colours.muted);
}

/// A settings row: a title and a description on the left, the control on
/// the right, and a divider above. When the text would get narrower than
/// [`theme::MIN_TEXT`], the control goes under it.
pub struct Row<'a> {
    key: &'a str,
    title: &'a str,
    description: Option<&'a str>,
    value: bool,
    plain: bool,
    problem: Option<&'a str>,
    caution: Option<&'a str>,
    status: Option<bool>,
    control: f32,
}

/// A row with the stable `key` and the `title` a search shows.
pub fn row<'a>(key: &'a str, title: &'a str) -> Row<'a> {
    Row {
        key,
        title,
        description: None,
        value: false,
        plain: false,
        problem: None,
        caution: None,
        status: None,
        control: theme::FIELD_WIDTH,
    }
}

impl<'a> Row<'a> {
    pub fn describe(mut self, text: &'a str) -> Self {
        self.description = Some(text);
        self
    }

    /// The description is a value, such as a path, in the monospace face.
    pub fn value(mut self, text: &'a str) -> Self {
        self.description = Some(text);
        self.value = true;
        self
    }

    /// The title in the regular weight, for text the user wrote.
    pub fn plain(mut self) -> Self {
        self.plain = true;
        self
    }

    /// Red text when the value cannot be used.
    pub fn problem(mut self, text: Option<&'a str>) -> Self {
        self.problem = text;
        self
    }

    /// Amber text when the value works but has a catch.
    pub fn caution(mut self, text: Option<&'a str>) -> Self {
        self.caution = text;
        self
    }

    /// A check or an alert before the title.
    pub fn status(mut self, ok: bool) -> Self {
        self.status = Some(ok);
        self
    }

    /// The width the control needs; the text takes the rest.
    pub fn reserve(mut self, width: f32) -> Self {
        self.control = width;
        self
    }

    pub fn show<R>(self, ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
        ui.push_id(self.key, |ui| self.draw(ui, add)).inner
    }

    /// A row whose control is a value field, which saves on Enter or when it
    /// loses focus. The caller saves the committed text or rejects it.
    pub fn value_field(self, ui: &mut Ui, saved: &str, placeholder: &str) -> Option<Committed> {
        let key = self.key;
        let outcome = self.show(ui, |ui| {
            setting_field(ui, key, saved, placeholder, FieldKind::Value, false)
        });
        match outcome {
            FieldOutcome::Commit(committed) => Some(committed),
            _ => None,
        }
    }

    /// A row whose control is a whole number from `range` with `unit` after
    /// it. Text that is no such number stays in the field with a problem.
    pub fn number_field(
        self,
        ui: &mut Ui,
        saved: u64,
        range: RangeInclusive<u64>,
        unit: &str,
    ) -> Option<(u64, Committed)> {
        let key = self.key;
        let saved = saved.to_string();
        let outcome = self.show(ui, |ui| {
            let unit = egui::RichText::new(unit)
                .font(theme::mono(theme::MONO))
                .color(palette(ui.ctx()).muted);
            ui.label(unit);
            setting_field(ui, key, &saved, "", FieldKind::Number, false)
        });
        let FieldOutcome::Commit(committed) = outcome else {
            return None;
        };
        match parse_number(&committed.text, &range) {
            Ok(number) => Some((number, committed)),
            Err(problem) => {
                committed.reject(ui.ctx(), problem);
                None
            },
        }
    }

    fn draw<R>(self, ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
        let colours = palette(ui.ctx());
        let width = ui.available_width();
        let origin = ui.cursor().min;
        ui.painter().hline(
            origin.x..=origin.x + width,
            origin.y + 0.5,
            Stroke::new(1.0_f32, colours.line),
        );
        let top = origin.y + 1.0 + theme::ROW_PADDING;
        let split = theme::split(width, self.control);
        let text_width = match split {
            Split::Beside { text } => text,
            Split::Stacked => width,
        };
        let drafted = ui
            .data(|data| data.get_temp::<Draft>(draft_id(self.key)))
            .and_then(|draft| draft.problem);
        let block = self.text_block(ui, colours, text_width, drafted.as_deref());
        let text_height = block.height;

        let (text_top, control_rect, layout) = match split {
            Split::Beside { text } => {
                let height = text_height.max(theme::CONTROL_HEIGHT);
                (
                    top + (height - text_height) / 2.0,
                    Rect::from_min_max(
                        Pos2::new(origin.x + text + theme::ROW_GAP, top),
                        Pos2::new(origin.x + width, top + height),
                    ),
                    Layout::right_to_left(Align::Center),
                )
            },
            Split::Stacked => {
                let control_top = top + text_height + 8.0;
                (
                    top,
                    Rect::from_min_max(
                        Pos2::new(origin.x, control_top),
                        Pos2::new(origin.x + width, control_top + theme::CONTROL_HEIGHT),
                    ),
                    Layout::right_to_left(Align::Center),
                )
            },
        };
        block.paint(ui, Pos2::new(origin.x, text_top));

        let mut control = ui.new_child(UiBuilder::new().max_rect(control_rect).layout(layout));
        control.spacing_mut().item_spacing.x = 8.0;
        let inner = add(&mut control);
        let bottom = control
            .min_rect()
            .bottom()
            .max(text_top + text_height)
            .max(control_rect.bottom());
        ui.advance_cursor_after_rect(Rect::from_min_max(
            origin,
            Pos2::new(origin.x + width, bottom + theme::ROW_PADDING),
        ));
        inner
    }

    fn text_block(&self, ui: &Ui, colours: &Palette, width: f32, drafted: Option<&str>) -> Block {
        let mut lines = Vec::new();
        let status_width = if self.status.is_some() { 24.0 } else { 0.0 };
        if !self.title.is_empty() {
            let font = if self.plain {
                theme::sans(theme::BODY)
            } else {
                theme::medium(theme::BODY)
            };
            lines.push(Line {
                icon: self.status.map(|ok| {
                    if ok {
                        (icons::CIRCLE_CHECK, colours.good, theme::ICON)
                    } else {
                        (icons::CIRCLE_ALERT, colours.warn, theme::ICON)
                    }
                }),
                text: Text::new(
                    ui,
                    self.title,
                    font,
                    colours.ink,
                    width - status_width,
                    theme::BODY_LINE,
                ),
                indent: status_width,
                gap: 0.0,
            });
        }
        if let Some(description) = self.description {
            let font = if self.value {
                theme::mono(theme::MONO)
            } else {
                theme::sans(theme::SMALL)
            };
            lines.push(Line {
                icon: None,
                text: Text::new(
                    ui,
                    description,
                    font,
                    colours.muted,
                    width,
                    theme::SMALL_LINE,
                ),
                indent: 0.0,
                gap: 0.0,
            });
        }
        for (text, icon, colour) in [
            (drafted.or(self.problem), icons::CIRCLE_X, colours.bad),
            (self.caution, icons::TRIANGLE_ALERT, colours.warn),
        ] {
            if let Some(text) = text {
                lines.push(Line {
                    icon: Some((icon, colour, 14.0)),
                    text: Text::new(
                        ui,
                        text,
                        theme::sans(theme::SMALL),
                        colour,
                        width - 20.0,
                        theme::SMALL_LINE,
                    ),
                    indent: 20.0,
                    gap: 4.0,
                });
            }
        }
        let height = lines.iter().map(|line| line.gap + line.text.height()).sum();
        Block { lines, height }
    }
}

struct Line {
    /// An icon in the indent: the glyph, its colour, and its size
    icon: Option<(char, Color32, f32)>,
    text: Text,
    indent: f32,
    gap: f32,
}

struct Block {
    lines: Vec<Line>,
    height: f32,
}

impl Block {
    fn paint(&self, ui: &Ui, at: Pos2) {
        let mut y = at.y;
        for line in &self.lines {
            y += line.gap;
            if let Some((icon, colour, size)) = line.icon {
                paint_icon(
                    ui,
                    Pos2::new(at.x + size / 2.0, y + line.text.line / 2.0),
                    icon,
                    size,
                    colour,
                );
            }
            line.text.paint(ui, Pos2::new(at.x + line.indent, y));
            y += line.text.height();
        }
    }
}

/// Buttons that act on the rows above, at the right, with no title.
pub fn actions<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.add_space(8.0);
    let width = ui.available_width();
    ui.allocate_ui_with_layout(
        Vec2::new(width, theme::CONTROL_HEIGHT),
        Layout::right_to_left(Align::Center),
        add,
    )
    .inner
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    /// The one main action: a neutral fill
    Primary,
    /// An outlined button
    Secondary,
    /// No edge, ink text
    Ghost,
    /// No edge, muted text until hovered
    Quiet,
    /// An outlined button for an action that deletes
    Danger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    /// 30 points tall, the height of a field
    Medium,
    /// 24 points tall, for a list
    Small,
    /// A square of 24 points with only the icon
    Icon,
}

/// A button with an optional Lucide icon before the label.
pub struct Button<'a> {
    label: &'a str,
    icon: Option<char>,
    variant: Variant,
    size: Size,
    enabled: bool,
}

impl<'a> Button<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            icon: None,
            variant: Variant::Secondary,
            size: Size::Medium,
            enabled: true,
        }
    }

    /// A square button with only `icon`; `label` names it for the screen
    /// reader and the tooltip.
    pub fn icon_only(icon: char, label: &'a str) -> Self {
        Self {
            label,
            icon: Some(icon),
            variant: Variant::Quiet,
            size: Size::Icon,
            enabled: true,
        }
    }

    pub fn icon(mut self, icon: char) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl Widget for Button<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let colours = palette(ui.ctx());
        let (height, pad, gap, font, icon_size) = match self.size {
            Size::Medium => (
                theme::CONTROL_HEIGHT,
                12.0,
                6.0,
                theme::medium(theme::BODY),
                theme::ICON,
            ),
            Size::Small => (
                theme::SMALL_CONTROL,
                8.0,
                4.0,
                theme::medium(theme::SMALL),
                14.0,
            ),
            Size::Icon => (
                theme::SMALL_CONTROL,
                0.0,
                0.0,
                theme::medium(theme::SMALL),
                theme::ICON,
            ),
        };
        let galley = (self.size != Size::Icon).then(|| {
            ui.fonts(|fonts| {
                fonts.layout_no_wrap(self.label.to_owned(), font, Color32::PLACEHOLDER)
            })
        });
        let content = match (&galley, self.icon) {
            (Some(galley), Some(_)) => icon_size + gap + galley.size().x,
            (Some(galley), None) => galley.size().x,
            (None, _) => icon_size,
        };
        let width = if self.size == Size::Icon {
            height
        } else {
            content + pad * 2.0
        };
        let sense = if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), sense);
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, self.enabled, self.label));
        if !ui.is_rect_visible(rect) {
            return response;
        }
        let hovered = self.enabled && response.hovered();
        let (fill, edge, ink) = button_colours(colours, self.variant, self.enabled, hovered);
        ui.painter()
            .rect(rect, theme::RADIUS_SM, fill, Stroke::new(1.0_f32, edge));
        if response.has_focus() {
            focus_ring(ui, rect, theme::RADIUS_SM);
        }
        let mut x = rect.center().x - content / 2.0;
        if let Some(icon) = self.icon {
            paint_icon(
                ui,
                Pos2::new(x + icon_size / 2.0, rect.center().y),
                icon,
                icon_size,
                ink,
            );
            x += icon_size + gap;
        }
        if let Some(galley) = galley {
            let at = Pos2::new(x, rect.center().y - galley.size().y / 2.0);
            ui.painter().galley(at, galley, ink);
        }
        response
    }
}

fn button_colours(
    colours: &Palette,
    variant: Variant,
    enabled: bool,
    hovered: bool,
) -> (Color32, Color32, Color32) {
    let none = Color32::TRANSPARENT;
    if !enabled {
        return match variant {
            Variant::Secondary | Variant::Danger | Variant::Primary => {
                (colours.surface, colours.line, colours.faint)
            },
            Variant::Ghost | Variant::Quiet => (none, none, colours.faint),
        };
    }
    let hover = if hovered { colours.hover } else { none };
    match variant {
        Variant::Primary => {
            let fill = if hovered {
                colours.primary.lerp_to_gamma(colours.surface, 0.12)
            } else {
                colours.primary
            };
            (fill, fill, colours.on_primary)
        },
        Variant::Secondary => (
            if hovered {
                colours.hover
            } else {
                colours.surface
            },
            colours.control,
            colours.ink,
        ),
        Variant::Danger => (
            if hovered {
                colours.hover
            } else {
                colours.surface
            },
            colours.bad,
            colours.bad,
        ),
        Variant::Ghost => (hover, none, colours.ink),
        Variant::Quiet => (
            hover,
            none,
            if hovered { colours.ink } else { colours.muted },
        ),
    }
}

/// A secondary button.
pub fn button(ui: &mut Ui, label: &str) -> Response {
    ui.add(Button::new(label))
}

/// A quiet square button with only an icon, named by its tooltip.
pub fn icon_button(ui: &mut Ui, icon: char, label: &str) -> Response {
    ui.add(Button::icon_only(icon, label)).on_hover_text(label)
}

/// A switch: a capsule with a knob that slides right when on.
pub fn switch(ui: &mut Ui, on: &mut bool) -> Response {
    let colours = palette(ui.ctx());
    let size = Vec2::new(36.0, 20.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, ui.is_enabled(), *on, ""));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let how_on = ui.ctx().animate_bool_responsive(response.id, *on);
    let track = if *on { colours.primary } else { colours.sunken };
    let edge = if *on {
        colours.primary
    } else {
        colours.control
    };
    let radius = rect.height() / 2.0;
    ui.painter()
        .rect(rect, radius, track, Stroke::new(1.0_f32, edge));
    let knob = if *on {
        colours.on_primary
    } else {
        colours.muted
    };
    let x = egui::lerp((rect.left() + 10.0)..=(rect.right() - 10.0), how_on);
    ui.painter()
        .circle_filled(Pos2::new(x, rect.center().y), 7.0, knob);
    if response.has_focus() {
        focus_ring(ui, rect, radius);
    }
    response
}

/// A row whose only control is a switch.
pub fn switch_row(
    ui: &mut Ui,
    key: &str,
    title: &str,
    description: &str,
    on: &mut bool,
) -> Response {
    row(key, title)
        .describe(description)
        .reserve(36.0)
        .show(ui, |ui| switch(ui, on))
}

/// `−`, the value, `+`: for a small whole number. `show` writes the value.
pub fn stepper(
    ui: &mut Ui,
    value: &mut u64,
    range: RangeInclusive<u64>,
    show: impl Fn(u64) -> String,
) -> Response {
    let colours = palette(ui.ctx());
    let (min, max) = (*range.start(), *range.end());
    let font = theme::mono_medium(theme::MONO);
    let widest = [min, max, *value]
        .into_iter()
        .map(|n| {
            ui.fonts(|fonts| {
                fonts
                    .layout_no_wrap(show(n), font.clone(), colours.ink)
                    .size()
                    .x
            })
        })
        .fold(32.0_f32, f32::max);
    let before = *value;
    let inner = ui.allocate_ui_with_layout(
        Vec2::new(
            widest + 2.0 * (theme::SMALL_CONTROL + 4.0),
            theme::CONTROL_HEIGHT,
        ),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let fewer = Button::icon_only(icons::MINUS, "Less")
                .variant(Variant::Secondary)
                .enabled(*value > min);
            if ui.add(fewer).clicked() {
                *value = value.saturating_sub(1).max(min);
            }
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(widest, theme::CONTROL_HEIGHT), Sense::hover());
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                show(*value),
                font.clone(),
                colours.ink,
            );
            let more = Button::icon_only(icons::PLUS, "More")
                .variant(Variant::Secondary)
                .enabled(*value < max);
            if ui.add(more).clicked() {
                *value = (*value + 1).min(max);
            }
        },
    );
    let mut response = inner.response;
    if *value != before {
        response.mark_changed();
    }
    response
}

/// Two or three choices side by side, the chosen one filled.
pub fn segmented<T: PartialEq + Copy>(
    ui: &mut Ui,
    value: &mut T,
    options: &[(T, &str)],
) -> Response {
    let colours = palette(ui.ctx());
    let font = theme::medium(theme::SMALL);
    let galleys: Vec<_> = options
        .iter()
        .map(|(_, label)| {
            ui.fonts(|fonts| {
                fonts.layout_no_wrap((*label).to_owned(), font.clone(), Color32::PLACEHOLDER)
            })
        })
        .collect();
    let widths: Vec<f32> = galleys.iter().map(|g| g.size().x + 24.0).collect();
    let inner_width: f32 = widths.iter().sum::<f32>() + 2.0 * (options.len() as f32 - 1.0);
    let size = Vec2::new(inner_width + 6.0, theme::CONTROL_HEIGHT);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect(
        rect,
        theme::RADIUS_SM,
        colours.surface,
        Stroke::new(1.0_f32, colours.control),
    );
    let mut x = rect.left() + 3.0;
    for (((option, label), galley), width) in options.iter().zip(galleys).zip(widths) {
        let cell = Rect::from_min_size(
            Pos2::new(x, rect.center().y - theme::SMALL_CONTROL / 2.0),
            Vec2::new(width, theme::SMALL_CONTROL),
        );
        let id = response.id.with(label);
        let cell_response = ui.interact(cell, id, Sense::click());
        cell_response.widget_info(|| {
            WidgetInfo::selected(
                WidgetType::RadioButton,
                ui.is_enabled(),
                *value == *option,
                *label,
            )
        });
        if cell_response.clicked() && *value != *option {
            *value = *option;
            response.mark_changed();
        }
        let chosen = *value == *option;
        if chosen {
            ui.painter()
                .rect_filled(cell, theme::RADIUS_XS, colours.selection);
        }
        let ink = if chosen || cell_response.hovered() {
            colours.ink
        } else {
            colours.muted
        };
        ui.painter()
            .galley(cell.center() - galley.size() / 2.0, galley, ink);
        if cell_response.has_focus() {
            focus_ring(ui, cell, theme::RADIUS_XS);
        }
        x += width + 2.0;
    }
    response
}

/// A one-line text field.
pub struct Field<'a> {
    text: &'a mut String,
    placeholder: &'a str,
    width: f32,
    mono: bool,
    right: bool,
    invalid: bool,
    icon: Option<char>,
}

impl<'a> Field<'a> {
    pub fn new(text: &'a mut String, placeholder: &'a str) -> Self {
        Self {
            text,
            placeholder,
            width: theme::FIELD_WIDTH,
            mono: false,
            right: false,
            invalid: false,
            icon: None,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// A value such as a key, a path, or an address, in the monospace face.
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    /// Numbers, aligned right.
    pub fn number(mut self) -> Self {
        self.mono = true;
        self.right = true;
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// An icon inside the field, before the text.
    pub fn icon(mut self, icon: char) -> Self {
        self.icon = Some(icon);
        self
    }
}

impl Widget for Field<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let colours = palette(ui.ctx());
        let font = if self.mono {
            theme::mono(theme::MONO)
        } else {
            theme::sans(theme::BODY)
        };
        let left = if self.icon.is_some() { 30.0 } else { 8.0 };
        let margin = egui::Margin {
            left,
            right: 8.0,
            top: 0.0,
            bottom: 0.0,
        };
        let hint = egui::RichText::new(self.placeholder)
            .font(font.clone())
            .color(colours.muted);
        let edit = egui::TextEdit::singleline(self.text)
            .font(font)
            .text_color(colours.ink)
            .hint_text(hint)
            .frame(false)
            .margin(margin)
            .horizontal_align(if self.right { Align::Max } else { Align::Min })
            .vertical_align(Align::Center)
            .min_size(Vec2::new(self.width, theme::CONTROL_HEIGHT))
            .desired_width(self.width - left - 8.0);
        let background = ui.painter().add(egui::Shape::Noop);
        let response = ui.add(edit);
        let rect = response.rect + margin;
        let edge = if self.invalid {
            colours.bad
        } else {
            colours.control
        };
        ui.painter().set(
            background,
            egui::epaint::RectShape::new(
                rect,
                theme::RADIUS_SM,
                colours.surface,
                Stroke::new(1.0_f32, edge),
            ),
        );
        if let Some(icon) = self.icon {
            paint_icon(
                ui,
                Pos2::new(rect.left() + 15.0, rect.center().y),
                icon,
                theme::ICON,
                colours.muted,
            );
        }
        if response.has_focus() {
            focus_ring(ui, rect, theme::RADIUS_SM);
        }
        response
    }
}

/// A text box of several lines, `width` wide.
pub fn text_area(
    ui: &mut Ui,
    text: &mut String,
    placeholder: &str,
    rows: usize,
    width: f32,
) -> Response {
    let colours = palette(ui.ctx());
    let hint = egui::RichText::new(placeholder)
        .font(theme::sans(theme::BODY))
        .color(colours.muted);
    let margin = egui::Margin::symmetric(10.0, 8.0);
    let background = ui.painter().add(egui::Shape::Noop);
    let response = ui.add(
        egui::TextEdit::multiline(text)
            .font(theme::sans(theme::BODY))
            .text_color(colours.ink)
            .hint_text(hint)
            .frame(false)
            .margin(margin)
            .desired_rows(rows)
            .desired_width(width - 20.0),
    );
    let rect = response.rect + margin;
    ui.painter().set(
        background,
        egui::epaint::RectShape::new(
            rect,
            theme::RADIUS_SM,
            colours.surface,
            Stroke::new(1.0_f32, colours.control),
        ),
    );
    if response.has_focus() {
        focus_ring(ui, rect, theme::RADIUS_SM);
    }
    response
}

/// An icon and a short line of muted text, such as what the daemon does.
pub fn status(ui: &mut Ui, icon: char, colour: Color32, text: &str) {
    let colours = palette(ui.ctx());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        self::icon(ui, icon, colour);
        self::text(ui, text, theme::sans(theme::SMALL), colours.muted);
    });
}

/// A spinner and a short line of muted text.
pub fn busy(ui: &mut Ui, text: &str) {
    let colours = palette(ui.ctx());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.add(egui::Spinner::new().size(14.0).color(colours.muted));
        self::text(ui, text, theme::sans(theme::SMALL), colours.muted);
    });
}

/// A field of the standard width in the regular face.
pub fn field(ui: &mut Ui, text: &mut String, placeholder: &str) -> Response {
    ui.add(Field::new(text, placeholder))
}

/// A field for a value: an address, a model name, a window class, a path.
pub fn value_field(ui: &mut Ui, text: &mut String, placeholder: &str) -> Response {
    ui.add(Field::new(text, placeholder).mono())
}

/// A field enter-pressed after the user typed in it.
pub fn submitted(ui: &Ui, response: &Response) -> bool {
    response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter))
}

/// A field's text while the user edits it, and why its last commit failed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Draft {
    pub text: String,
    pub problem: Option<String>,
}

/// What the user did to a field in one frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldEvent {
    Idle,
    Editing,
    /// Enter, or the focus moved away
    Commit,
    Escape,
}

/// One frame of a field that saves on Enter or when it loses focus: the
/// draft it keeps, and the text to save. Escape drops the draft, so the
/// field shows the saved text again. A commit of the saved text saves
/// nothing and clears a failed draft.
pub fn draft_step(
    draft: Option<Draft>,
    saved: &str,
    text: String,
    event: FieldEvent,
) -> (Option<Draft>, Option<String>) {
    match event {
        FieldEvent::Idle => (draft, None),
        FieldEvent::Editing => (
            Some(Draft {
                text,
                problem: draft.and_then(|draft| draft.problem),
            }),
            None,
        ),
        FieldEvent::Escape => (None, None),
        FieldEvent::Commit if text == saved => (None, None),
        FieldEvent::Commit => (None, Some(text)),
    }
}

fn draft_id(key: &str) -> egui::Id {
    egui::Id::new(("hush-draft", key))
}

/// Text that the user committed in the field of the row `key`.
#[derive(Debug)]
pub struct Committed {
    pub text: String,
    key: egui::Id,
}

impl Committed {
    /// Keep the text in the field, with `problem` under its row. The setting
    /// keeps its saved value.
    pub fn reject(self, ctx: &egui::Context, problem: impl Into<String>) {
        let draft = Draft {
            text: self.text,
            problem: Some(problem.into()),
        };
        ctx.data_mut(|data| data.insert_temp(self.key, draft));
        ctx.request_repaint();
    }
}

/// The outcome of one frame of a [`setting_field`].
#[derive(Debug)]
pub enum FieldOutcome {
    Idle,
    Editing,
    /// The user committed a change, which the caller saves or rejects
    Commit(Committed),
    /// The focus left with nothing to save: Escape, or the saved text
    Closed,
}

/// How a [`setting_field`] looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    /// A value such as an address or a model name, in the monospace face
    Value,
    /// A whole number, aligned right
    Number,
}

/// The field of the row `key` for a setting saved as `saved`. It saves on
/// Enter or when it loses focus, and Escape restores the saved text. A
/// rejected commit keeps its text, and the row shows the problem.
pub fn setting_field(
    ui: &mut Ui,
    key: &str,
    saved: &str,
    placeholder: &str,
    kind: FieldKind,
    focus: bool,
) -> FieldOutcome {
    let id = draft_id(key);
    let draft: Option<Draft> = ui.data(|data| data.get_temp(id));
    let mut text = draft
        .as_ref()
        .map_or_else(|| saved.to_owned(), |draft| draft.text.clone());
    let invalid = draft.as_ref().is_some_and(|draft| draft.problem.is_some());
    let field = Field::new(&mut text, placeholder).invalid(invalid);
    let field = match kind {
        FieldKind::Value => field.mono(),
        FieldKind::Number => field.number().width(72.0),
    };
    let response = ui.add(field);
    if focus {
        response.request_focus();
    }
    let event = if response.lost_focus() {
        if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            FieldEvent::Escape
        } else {
            FieldEvent::Commit
        }
    } else if response.has_focus() {
        FieldEvent::Editing
    } else {
        FieldEvent::Idle
    };
    let (next, commit) = draft_step(draft, saved, text, event);
    ui.data_mut(|data| match next {
        Some(next) => data.insert_temp(id, next),
        None => data.remove::<Draft>(id),
    });
    match (event, commit) {
        (_, Some(text)) => FieldOutcome::Commit(Committed { text, key: id }),
        (FieldEvent::Commit | FieldEvent::Escape, None) => FieldOutcome::Closed,
        (FieldEvent::Editing, None) => FieldOutcome::Editing,
        (FieldEvent::Idle, None) => FieldOutcome::Idle,
    }
}

/// Drop the draft of the row `key`, as when its field closes.
pub fn forget_draft(ctx: &egui::Context, key: &str) {
    ctx.data_mut(|data| data.remove::<Draft>(draft_id(key)));
}

/// A whole number from `range`, or the problem to show.
pub fn parse_number(text: &str, range: &RangeInclusive<u64>) -> Result<u64, String> {
    let problem = || {
        format!(
            "Enter a whole number from {} to {}.",
            range.start(),
            range.end()
        )
    };
    let number: u64 = text.trim().parse().map_err(|_| problem())?;
    if range.contains(&number) {
        Ok(number)
    } else {
        Err(problem())
    }
}

/// A drop-down that shows `selected` and lists what `add` draws, with
/// [`option`] for each choice.
pub fn dropdown<R>(
    ui: &mut Ui,
    id_salt: impl std::hash::Hash,
    width: f32,
    selected: &str,
    add: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    let colours = palette(ui.ctx());
    let text = egui::RichText::new(selected)
        .font(theme::sans(theme::BODY))
        .color(colours.ink);
    let shown = egui::ComboBox::from_id_salt(id_salt)
        .width(width)
        .height(320.0)
        .selected_text(text)
        .truncate()
        .icon(|ui, rect, _visuals, _open, _above| {
            paint_icon(
                ui,
                rect.center(),
                icons::CHEVRON_DOWN,
                theme::ICON,
                palette(ui.ctx()).muted,
            );
        })
        .show_ui(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.set_min_width(width - 8.0);
            add(ui)
        });
    if shown.response.has_focus() {
        focus_ring(ui, shown.response.rect, theme::RADIUS_SM);
    }
    shown.inner
}

/// One choice in a [`dropdown`], with a check on the chosen one.
pub fn option<T: PartialEq>(ui: &mut Ui, current: &mut T, value: T, label: &str) -> Response {
    let colours = palette(ui.ctx());
    let chosen = *current == value;
    let width = ui.available_width().max(ui.min_rect().width());
    let galley = ui.fonts(|fonts| {
        fonts.layout_no_wrap(label.to_owned(), theme::sans(theme::BODY), colours.ink)
    });
    let width = width.max(galley.size().x + 40.0);
    let (rect, mut response) = ui.allocate_exact_size(Vec2::new(width, 28.0), Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, true, chosen, label));
    if response.hovered() || response.has_focus() {
        ui.painter()
            .rect_filled(rect, theme::RADIUS_XS, colours.hover);
    }
    ui.painter().galley(
        Pos2::new(rect.left() + 8.0, rect.center().y - galley.size().y / 2.0),
        galley,
        colours.ink,
    );
    if chosen {
        paint_icon(
            ui,
            Pos2::new(rect.right() - 16.0, rect.center().y),
            icons::CHECK,
            14.0,
            colours.ink,
        );
    }
    if response.clicked() && !chosen {
        *current = value;
        response.mark_changed();
    }
    response
}

/// The field that captures a key: the chord's keys as key caps, a prompt
/// while it listens, or "Not set".
pub fn key_field(
    ui: &mut Ui,
    chord: &str,
    listening: bool,
    width: f32,
    placeholder: &str,
) -> Response {
    let colours = palette(ui.ctx());
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, theme::CONTROL_HEIGHT), Sense::click());
    let label = if listening {
        "Press a key or button"
    } else if chord.is_empty() {
        placeholder
    } else {
        chord
    };
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), label));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let fill = if response.hovered() && !listening {
        colours.hover
    } else {
        colours.surface
    };
    ui.painter().rect(
        rect,
        theme::RADIUS_SM,
        fill,
        Stroke::new(1.0_f32, colours.control),
    );
    if listening || response.has_focus() {
        focus_ring(ui, rect, theme::RADIUS_SM);
    }
    paint_icon(
        ui,
        Pos2::new(rect.left() + 16.0, rect.center().y),
        icons::KEYBOARD,
        theme::ICON,
        colours.muted,
    );
    let start = rect.left() + 32.0;
    if listening || chord.is_empty() {
        let text = if listening {
            "Press a key or button…"
        } else {
            placeholder
        };
        ui.painter().text(
            Pos2::new(start, rect.center().y),
            Align2::LEFT_CENTER,
            text,
            theme::sans(theme::BODY),
            colours.muted,
        );
        return response;
    }
    let mut x = start;
    for part in chord.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        let galley = ui.fonts(|fonts| {
            fonts.layout_no_wrap(part.to_owned(), theme::mono(theme::MONO), colours.ink)
        });
        let cap = Rect::from_min_size(
            Pos2::new(x, rect.center().y - 10.0),
            Vec2::new((galley.size().x + 10.0).max(20.0), 20.0),
        );
        if cap.right() > rect.right() - 6.0 {
            break;
        }
        ui.painter().rect(
            cap,
            theme::RADIUS_XS,
            colours.sunken,
            Stroke::new(1.0_f32, colours.line),
        );
        ui.painter()
            .galley(cap.center() - galley.size() / 2.0, galley, colours.ink);
        x = cap.right() + 4.0;
    }
    response
}

/// A capsule filled with the accent to `fraction`.
pub fn meter(ui: &mut Ui, fraction: f32, width: f32) -> Response {
    let colours = palette(ui.ctx());
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, theme::CONTROL_HEIGHT), Sense::hover());
    let track = Rect::from_center_size(rect.center(), Vec2::new(width, 6.0));
    ui.painter().rect_filled(track, 3.0, colours.line);
    let fraction = fraction.clamp(0.0, 1.0);
    if fraction > 0.0 {
        let fill = Rect::from_min_size(
            track.min,
            Vec2::new((track.width() * fraction).max(6.0), track.height()),
        );
        ui.painter().rect_filled(fill, 3.0, colours.accent);
    }
    response
}

/// One entry of a list under a row: an icon, the text, and the actions that
/// `add` draws at the right.
pub struct ListItem<'a> {
    key: &'a str,
    icon: char,
    text: &'a str,
    detail: Option<&'a str>,
    mono: bool,
}

pub fn list_item<'a>(key: &'a str, icon: char, text: &'a str) -> ListItem<'a> {
    ListItem {
        key,
        icon,
        text,
        detail: None,
        mono: false,
    }
}

impl<'a> ListItem<'a> {
    /// A second, muted line.
    pub fn detail(mut self, text: &'a str) -> Self {
        self.detail = Some(text);
        self
    }

    /// The text is a value, such as a window class.
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    pub fn show<R>(self, ui: &mut Ui, actions: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
        ui.push_id(self.key, |ui| self.draw(ui, actions, add)).inner
    }

    fn draw<R>(self, ui: &mut Ui, actions: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
        let colours = palette(ui.ctx());
        let width = ui.available_width();
        let text_width = (width - 8.0 - 16.0 - 12.0 - actions - 12.0).max(60.0);
        let font = if self.mono {
            theme::mono(theme::MONO)
        } else {
            theme::sans(theme::BODY)
        };
        let text = Text::new(
            ui,
            self.text,
            font,
            colours.ink,
            text_width,
            theme::BODY_LINE,
        );
        let detail = self.detail.map(|detail| {
            Text::new(
                ui,
                detail,
                theme::sans(theme::SMALL),
                colours.muted,
                text_width,
                theme::SMALL_LINE,
            )
        });
        let content = text.height() + detail.as_ref().map_or(0.0, Text::height);
        let height = (content + 16.0).max(36.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
        if ui.rect_contains_pointer(rect) {
            ui.painter()
                .rect_filled(rect, theme::RADIUS_SM, colours.hover);
        }
        let top = rect.center().y - content / 2.0;
        paint_icon(
            ui,
            Pos2::new(rect.left() + 16.0, top + theme::BODY_LINE / 2.0),
            self.icon,
            theme::ICON,
            colours.muted,
        );
        let x = rect.left() + 8.0 + 16.0 + 12.0;
        text.paint(ui, Pos2::new(x, top));
        if let Some(detail) = &detail {
            detail.paint(ui, Pos2::new(x, top + text.height()));
        }
        let controls = Rect::from_min_max(
            Pos2::new(rect.right() - 4.0 - actions, rect.top()),
            Pos2::new(rect.right() - 4.0, rect.bottom()),
        );
        let mut ui = ui.new_child(
            UiBuilder::new()
                .max_rect(controls)
                .layout(Layout::right_to_left(Align::Center)),
        );
        ui.spacing_mut().item_spacing.x = 4.0;
        add(&mut ui)
    }
}

/// The line that a list starts with when no row comes before it.
pub fn divider(ui: &mut Ui) {
    let line = palette(ui.ctx()).line;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 9.0), Sense::hover());
    ui.painter()
        .hline(rect.x_range(), rect.top() + 0.5, Stroke::new(1.0_f32, line));
}

/// The space a list takes under its row, and after it.
pub fn list<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let inner = ui
        .scope(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            add(ui)
        })
        .inner;
    ui.add_space(8.0);
    inner
}

/// A rounded chip, for a word that a fix proposes.
pub fn chip<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let colours = palette(ui.ctx());
    egui::Frame::none()
        .fill(colours.surface)
        .stroke(Stroke::new(1.0_f32, colours.control))
        .rounding(Rounding::same(14.0))
        .inner_margin(egui::Margin {
            left: 12.0,
            right: 4.0,
            top: 2.0,
            bottom: 2.0,
        })
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.horizontal(add).inner
        })
        .inner
}

/// Text in one of the type scale's styles.
pub fn text(ui: &mut Ui, text: &str, font: FontId, colour: Color32) -> Response {
    ui.add(
        egui::Label::new(egui::RichText::new(text).font(font).color(colour))
            .wrap_mode(egui::TextWrapMode::Extend),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_outside_its_range_is_a_problem_not_a_clamp() {
        let range = 1..=3650;
        assert_eq!(parse_number(" 30 ", &range), Ok(30));
        assert_eq!(parse_number("1", &range), Ok(1));
        assert_eq!(parse_number("3650", &range), Ok(3650));
        let problem = Err("Enter a whole number from 1 to 3650.".to_string());
        for text in ["0", "99999", "thirty", "-4", "", "2.5"] {
            assert_eq!(parse_number(text, &range), problem, "{text:?}");
        }
    }

    /// The window's frames for a field, driven by input events.
    struct Window {
        ctx: egui::Context,
    }

    impl Window {
        fn new() -> Self {
            let ctx = egui::Context::default();
            crate::gui::theme::install(&ctx);
            Self { ctx }
        }

        fn frame(&self, events: Vec<egui::Event>, saved: &str, focus: bool) -> String {
            let mut outcome = String::new();
            let input = egui::RawInput {
                events,
                ..Default::default()
            };
            let _ = self.ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    outcome = match setting_field(ui, "test", saved, "", FieldKind::Value, focus) {
                        FieldOutcome::Idle => "idle".into(),
                        FieldOutcome::Editing => "editing".into(),
                        FieldOutcome::Closed => "closed".into(),
                        FieldOutcome::Commit(commit) => format!("commit {}", commit.text),
                    };
                });
            });
            outcome
        }

        fn number(&self, events: Vec<egui::Event>, saved: u64) -> Option<u64> {
            let mut number = None;
            let input = egui::RawInput {
                events,
                ..Default::default()
            };
            let _ = self.ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    number = row("test", "Test")
                        .number_field(ui, saved, 0..=200, "ms")
                        .map(|(number, _)| number);
                });
            });
            number
        }

        fn draft(&self) -> Option<Draft> {
            self.ctx.data(|data| data.get_temp(draft_id("test")))
        }
    }

    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn typed(text: &str) -> egui::Event {
        egui::Event::Text(text.to_string())
    }

    /// A click far from the field, which moves the focus away.
    fn click_away() -> Vec<egui::Event> {
        let at = egui::pos2(5000.0, 5000.0);
        vec![
            egui::Event::PointerMoved(at),
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }

    #[test]
    fn enter_saves_the_typed_text_in_a_real_frame() {
        let window = Window::new();
        window.frame(Vec::new(), "llama", true);
        assert_eq!(window.frame(vec![typed("3")], "llama", false), "editing");
        assert_eq!(window.draft(), draft("llama3", None));
        assert_eq!(
            window.frame(vec![key(egui::Key::Enter)], "llama", false),
            "commit llama3"
        );
        assert_eq!(window.draft(), None);
    }

    #[test]
    fn leaving_the_field_saves_the_typed_text_in_a_real_frame() {
        let window = Window::new();
        window.frame(Vec::new(), "llama", true);
        window.frame(vec![typed("3")], "llama", false);
        assert_eq!(window.frame(click_away(), "llama", false), "commit llama3");
    }

    #[test]
    fn escape_puts_back_the_saved_text_in_a_real_frame() {
        let window = Window::new();
        window.frame(Vec::new(), "llama", true);
        window.frame(vec![typed("3")], "llama", false);
        assert_eq!(
            window.frame(vec![key(egui::Key::Escape)], "llama", false),
            "closed"
        );
        assert_eq!(window.draft(), None);
        assert_eq!(window.frame(Vec::new(), "llama", false), "idle");
    }

    #[test]
    fn a_number_in_its_range_saves_on_enter() {
        let window = Window::new();
        window.number(vec![key(egui::Key::Tab)], 10);
        window.number(Vec::new(), 10);
        window.number(vec![typed("5")], 10);
        assert_eq!(window.number(vec![key(egui::Key::Enter)], 10), Some(105));
        assert_eq!(window.draft(), None);
    }

    #[test]
    fn an_invalid_number_never_saves_and_stays_with_its_problem() {
        let window = Window::new();
        window.number(vec![key(egui::Key::Tab)], 10);
        window.number(Vec::new(), 10);
        window.number(vec![typed("0x")], 10);
        assert_eq!(window.number(vec![key(egui::Key::Enter)], 10), None);
        let kept = window.draft().unwrap();
        assert_eq!(kept.text, "100x");
        assert_eq!(
            kept.problem.as_deref(),
            Some("Enter a whole number from 0 to 200.")
        );
    }

    fn draft(text: &str, problem: Option<&str>) -> Option<Draft> {
        Some(Draft {
            text: text.to_string(),
            problem: problem.map(str::to_string),
        })
    }

    #[test]
    fn typing_keeps_a_draft_and_saves_nothing() {
        let (next, commit) = draft_step(None, "llama3.2", "llama3".into(), FieldEvent::Editing);
        assert_eq!((next, commit), (draft("llama3", None), None));
        let (next, commit) = draft_step(draft("x", None), "llama3.2", "x".into(), FieldEvent::Idle);
        assert_eq!((next, commit), (draft("x", None), None), "idle keeps it");
    }

    #[test]
    fn enter_or_leaving_the_field_commits_a_change() {
        let (next, commit) = draft_step(
            draft("qwen3", None),
            "llama3.2",
            "qwen3".into(),
            FieldEvent::Commit,
        );
        assert_eq!((next, commit), (None, Some("qwen3".to_string())));
        let (next, commit) = draft_step(
            draft("llama3.2", Some("Not a model.")),
            "llama3.2",
            "llama3.2".into(),
            FieldEvent::Commit,
        );
        assert_eq!(
            (next, commit),
            (None, None),
            "the saved text saves nothing and clears the problem"
        );
    }

    #[test]
    fn escape_restores_the_saved_text() {
        let (next, commit) = draft_step(
            draft("half typed", Some("Enter a whole number from 0 to 200.")),
            "10",
            "half typed".into(),
            FieldEvent::Escape,
        );
        assert_eq!((next, commit), (None, None));
    }

    #[test]
    fn a_rejected_commit_stays_in_its_field_with_the_problem() {
        let ctx = egui::Context::default();
        let committed = Committed {
            text: "Ctrl+Bogus".to_string(),
            key: draft_id("hotkeys.dictate"),
        };
        committed.reject(&ctx, "Unknown key Bogus");
        let kept = ctx.data(|data| data.get_temp::<Draft>(draft_id("hotkeys.dictate")));
        assert_eq!(kept, draft("Ctrl+Bogus", Some("Unknown key Bogus")));
        forget_draft(&ctx, "hotkeys.dictate");
        let kept = ctx.data(|data| data.get_temp::<Draft>(draft_id("hotkeys.dictate")));
        assert_eq!(kept, None);
    }

    #[test]
    fn a_rejected_value_keeps_its_problem_while_the_user_fixes_it() {
        let (next, _) = draft_step(
            draft("50000", Some("Enter a whole number from 0 to 200.")),
            "10",
            "5000".into(),
            FieldEvent::Editing,
        );
        assert_eq!(
            next,
            draft("5000", Some("Enter a whole number from 0 to 200."))
        );
    }

    #[test]
    fn disabled_buttons_read_as_disabled_whatever_their_variant() {
        for variant in [
            Variant::Primary,
            Variant::Secondary,
            Variant::Ghost,
            Variant::Quiet,
            Variant::Danger,
        ] {
            let (_, _, ink) = button_colours(&theme::LIGHT, variant, false, true);
            assert_eq!(ink, theme::LIGHT.faint, "{variant:?}");
        }
        let (fill, _, ink) = button_colours(&theme::DARK, Variant::Primary, true, false);
        assert_eq!((fill, ink), (theme::DARK.primary, theme::DARK.on_primary));
    }
}
