//! The window around the pages. The window draws its own frame, so it looks
//! the same on every desktop: a title bar with the page title and the
//! minimize, maximize, and close buttons, a sidebar with the Hush mark and
//! the pages, a footer with what Hush does and how the last save went, and
//! edges that resize it.

use super::autosave::Feedback;
use super::icons;
use super::pages::Page;
use super::theme::{self, palette};
use super::widgets::{self, Button, Size, Variant};
use super::Gui;
use egui::epaint::{Mesh, RectShape, TessellationOptions, Tessellator};
use egui::{
    Align, Align2, Color32, Context, CursorIcon, Id, Layout, Order, PointerButton, Pos2, Rect,
    ResizeDirection, Sense, Stroke, Ui, Vec2, ViewportCommand,
};

/// Bar heights of the mark, as fractions of its tile, centre bar tallest.
const BARS: [f32; 5] = [0.204, 0.367, 0.530, 0.367, 0.204];
const BAR_WIDTH: f32 = 0.068;
const BAR_STEP: f32 = 0.119;
const TILE_ROUNDING: f32 = 0.219;
const MARK_SIZE: f32 = 22.0;
/// The square at each corner of the window that resizes it.
const CORNER: f32 = 8.0;
const EDGE: f32 = theme::EDGE_CLEARANCE;

/// The Hush mark: the five-bar waveform on a violet tile.
pub fn mark(ui: &Ui, rect: Rect) {
    let size = rect.width();
    let tile = RectShape::filled(rect, size * TILE_ROUNDING, Color32::WHITE);
    let fonts = ui.fonts(|fonts| fonts.font_image_size());
    let mut tessellator = Tessellator::new(
        ui.ctx().pixels_per_point(),
        TessellationOptions::default(),
        fonts,
        Vec::new(),
    );
    let mut mesh = Mesh::default();
    tessellator.tessellate_rect(&tile, &mut mesh);
    for vertex in &mut mesh.vertices {
        let along = ((vertex.pos.x - rect.left()) + (vertex.pos.y - rect.top()))
            / (rect.width() + rect.height());
        let colour = theme::MARK_FROM.lerp_to_gamma(theme::MARK_TO, along.clamp(0.0, 1.0));
        vertex.color = colour.linear_multiply(f32::from(vertex.color.a()) / 255.0);
    }
    ui.painter().add(mesh);
    let first = rect.center().x - 2.0 * BAR_STEP * size;
    for (index, height) in BARS.iter().enumerate() {
        let bar = Rect::from_center_size(
            Pos2::new(first + index as f32 * BAR_STEP * size, rect.center().y),
            Vec2::new(BAR_WIDTH * size, height * size),
        );
        ui.painter()
            .rect_filled(bar, BAR_WIDTH * size / 2.0, Color32::WHITE);
    }
}

/// Toggle maximize on a double click, and move the window on a drag.
fn drag_region(ui: &Ui, rect: Rect, id: &str) {
    let response = ui.interact(rect, Id::new(id), Sense::click_and_drag());
    if response.double_clicked() {
        let maximized = ui.input(|input| input.viewport().maximized.unwrap_or(false));
        ui.ctx()
            .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    } else if response.drag_started_by(PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
}

/// The page title, and the window buttons at the right.
pub fn title_bar(ui: &mut Ui, title: &str) {
    let colours = palette(ui.ctx());
    drag_region(ui, ui.max_rect(), "title_bar");
    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
        ui.label(
            egui::RichText::new(title)
                .font(theme::semibold(theme::TITLE))
                .color(colours.ink),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if window_button(ui, icons::X, theme::ICON, "Close", true).clicked() {
                ui.ctx().send_viewport_cmd(ViewportCommand::Close);
            }
            let maximized = ui.input(|input| input.viewport().maximized.unwrap_or(false));
            let (icon, label) = if maximized {
                (icons::COPY, "Restore")
            } else {
                (icons::SQUARE, "Maximize")
            };
            if window_button(ui, icon, 13.0, label, false).clicked() {
                ui.ctx()
                    .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
            }
            if window_button(ui, icons::MINUS, theme::ICON, "Minimize", false).clicked() {
                ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
            }
            ui.add_space(6.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 32.0), Sense::hover());
            ui.painter().vline(
                rect.center().x,
                rect.y_range(),
                Stroke::new(1.0_f32, colours.line),
            );
        });
    });
}

fn window_button(ui: &mut Ui, icon: char, size: f32, label: &str, close: bool) -> egui::Response {
    let colours = palette(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    let (fill, ink) = match (response.hovered(), close) {
        (true, true) => (colours.bad, colours.on_primary),
        (true, false) => (colours.hover, colours.ink),
        (false, _) => (Color32::TRANSPARENT, colours.muted),
    };
    ui.painter().rect_filled(rect, theme::RADIUS_SM, fill);
    widgets::paint_icon(ui, rect.center(), icon, size, ink);
    if response.has_focus() {
        widgets::focus_ring(ui, rect.shrink(2.0), theme::RADIUS_SM);
    }
    response.on_hover_text(label)
}

/// The bands along the edges and corners that resize the window, while it
/// is not maximized. Without the desktop's frame, nothing else resizes it.
pub fn resize_edges(ctx: &Context) {
    if ctx.input(|input| input.viewport().maximized.unwrap_or(false)) {
        return;
    }
    let screen = ctx.screen_rect();
    for (index, (direction, rect, cursor)) in edges(screen).into_iter().enumerate() {
        egui::Area::new(Id::new(("resize", index)))
            .order(Order::Foreground)
            .fixed_pos(rect.min)
            .interactable(true)
            .show(ctx, |ui| {
                let (_, response) = ui.allocate_exact_size(rect.size(), Sense::drag());
                if response.hovered() {
                    ctx.set_cursor_icon(cursor);
                    if ctx.input(|input| input.pointer.primary_pressed()) {
                        ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction));
                    }
                }
            });
    }
}

/// Each resize band: its direction, its rectangle, and its cursor.
fn edges(screen: Rect) -> [(ResizeDirection, Rect, CursorIcon); 8] {
    let (left, right, top, bottom) = (screen.left(), screen.right(), screen.top(), screen.bottom());
    let span = |x0: f32, y0: f32, x1: f32, y1: f32| {
        Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1))
    };
    [
        (
            ResizeDirection::North,
            span(left + CORNER, top, right - CORNER, top + EDGE),
            CursorIcon::ResizeNorth,
        ),
        (
            ResizeDirection::South,
            span(left + CORNER, bottom - EDGE, right - CORNER, bottom),
            CursorIcon::ResizeSouth,
        ),
        (
            ResizeDirection::West,
            span(left, top + CORNER, left + EDGE, bottom - CORNER),
            CursorIcon::ResizeWest,
        ),
        (
            ResizeDirection::East,
            span(right - EDGE, top + CORNER, right, bottom - CORNER),
            CursorIcon::ResizeEast,
        ),
        (
            ResizeDirection::NorthWest,
            span(left, top, left + CORNER, top + CORNER),
            CursorIcon::ResizeNorthWest,
        ),
        (
            ResizeDirection::NorthEast,
            span(right - CORNER, top, right, top + CORNER),
            CursorIcon::ResizeNorthEast,
        ),
        (
            ResizeDirection::SouthWest,
            span(left, bottom - CORNER, left + CORNER, bottom),
            CursorIcon::ResizeSouthWest,
        ),
        (
            ResizeDirection::SouthEast,
            span(right - CORNER, bottom - CORNER, right, bottom),
            CursorIcon::ResizeSouthEast,
        ),
    ]
}

/// A one-point line around the window, which has no frame of the desktop's.
pub fn window_border(ctx: &Context) {
    if ctx.input(|input| input.viewport().maximized.unwrap_or(false)) {
        return;
    }
    let colours = palette(ctx);
    ctx.layer_painter(egui::LayerId::new(
        Order::Foreground,
        Id::new("window_border"),
    ))
    .rect_stroke(
        ctx.screen_rect().shrink(0.5),
        0.0,
        Stroke::new(1.0_f32, colours.line),
    );
}

/// The mark and the name, then the pages in two groups.
pub fn sidebar(gui: &mut Gui, ui: &mut Ui) {
    let colours = palette(ui.ctx());
    ui.spacing_mut().item_spacing.y = 0.0;
    let (header, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), theme::BAR_HEIGHT),
        Sense::hover(),
    );
    drag_region(ui, header, "sidebar_header");
    let tile = Rect::from_center_size(
        Pos2::new(header.left() + 16.0 + MARK_SIZE / 2.0, header.center().y),
        Vec2::splat(MARK_SIZE),
    );
    mark(ui, tile);
    ui.painter().text(
        Pos2::new(tile.right() + 8.0, header.center().y),
        Align2::LEFT_CENTER,
        "Hush",
        theme::semibold(18.0),
        colours.ink,
    );

    let ready = gui.machine.ready(&gui.config.transcription);
    egui::Frame::none()
        .inner_margin(egui::Margin {
            left: 8.0,
            right: 8.0,
            top: 8.0,
            bottom: 8.0,
        })
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for (index, (label, pages)) in GROUPS.iter().enumerate() {
                if index > 0 {
                    ui.add_space(14.0);
                }
                group_label(ui, label);
                for page in *pages {
                    let alert = *page == Page::Overview && !ready;
                    if nav_item(ui, page.icon(), page.title(), gui.page == *page, alert).clicked() {
                        gui.page = *page;
                    }
                }
            }
        });
    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
        egui::Frame::none()
            .inner_margin(egui::Margin::same(8.0))
            .show(ui, |ui| {
                if commands_item(ui).clicked() {
                    gui.palette.open(ui.ctx());
                }
            });
    });
}

/// The sidebar item that opens the command palette, with its key.
fn commands_item(ui: &mut Ui) -> egui::Response {
    let colours = palette(ui.ctx());
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Commands"));
    let hovered = response.hovered();
    if hovered {
        ui.painter()
            .rect_filled(rect, theme::RADIUS_SM, colours.hover);
    }
    let ink = if hovered { colours.ink } else { colours.muted };
    widgets::paint_icon(
        ui,
        Pos2::new(rect.left() + 8.0 + theme::ICON / 2.0, rect.center().y),
        icons::SEARCH,
        theme::ICON,
        ink,
    );
    ui.painter().text(
        Pos2::new(rect.left() + 32.0, rect.center().y),
        Align2::LEFT_CENTER,
        "Commands",
        theme::sans(theme::BODY),
        ink,
    );
    widgets::key_caps(ui, rect.right() - 8.0, rect.center().y, &["Ctrl", "K"]);
    if response.has_focus() {
        widgets::focus_ring(ui, rect.shrink(2.0), theme::RADIUS_SM);
    }
    response.on_hover_text("Find a setting, a page, or an action")
}

const GROUPS: [(&str, &[Page]); 2] = [
    (
        "Settings",
        &[
            Page::Overview,
            Page::Hotkeys,
            Page::Speech,
            Page::Text,
            Page::Output,
            Page::Desktop,
        ],
    ),
    ("Activity", &[Page::History]),
];

fn group_label(ui: &mut Ui, label: &str) {
    let colours = palette(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
    ui.painter().text(
        Pos2::new(rect.left() + 8.0, rect.top() + 7.0),
        Align2::LEFT_CENTER,
        label,
        theme::medium(theme::LABEL),
        colours.muted,
    );
}

fn nav_item(ui: &mut Ui, icon: char, label: &str, current: bool, alert: bool) -> egui::Response {
    let colours = palette(ui.ctx());
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, current, label)
    });
    let fill = if current {
        colours.selection
    } else if response.hovered() {
        colours.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, theme::RADIUS_SM, fill);
    let icon_colour = if current || response.hovered() {
        colours.ink
    } else {
        colours.muted
    };
    widgets::paint_icon(
        ui,
        Pos2::new(rect.left() + 8.0 + theme::ICON / 2.0, rect.center().y),
        icon,
        theme::ICON,
        icon_colour,
    );
    let font = if current {
        theme::medium(theme::BODY)
    } else {
        theme::sans(theme::BODY)
    };
    ui.painter().text(
        Pos2::new(rect.left() + 32.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font,
        colours.ink,
    );
    if alert {
        widgets::paint_icon(
            ui,
            Pos2::new(rect.right() - 16.0, rect.center().y),
            icons::CIRCLE_ALERT,
            14.0,
            colours.warn,
        );
    }
    if response.has_focus() {
        widgets::focus_ring(ui, rect.shrink(2.0), theme::RADIUS_SM);
    }
    if alert {
        response.on_hover_text("Hush is not ready yet")
    } else {
        response
    }
}

/// What Hush is doing, or the last message, at the left; how the last save
/// went at the right.
pub fn footer(gui: &mut Gui, ui: &mut Ui) {
    let colours = palette(ui.ctx());
    let small = || theme::sans(theme::SMALL);
    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if let Some(label) = gui.undo.as_ref().map(|undo| undo.label.clone()) {
            widgets::icon(ui, icons::TRASH, colours.muted);
            widgets::text(ui, &label, small(), colours.ink);
            let undo = Button::new("Undo")
                .icon(icons::ROTATE_CCW)
                .variant(Variant::Ghost)
                .size(Size::Small);
            if ui.add(undo).clicked() {
                gui.undo();
            }
        } else if let Some((message, _)) = &gui.toast {
            widgets::icon(ui, icons::INFO, colours.muted);
            let message = message.clone();
            widgets::text(ui, &message, small(), colours.ink);
        } else if gui.applying() {
            ui.add(egui::Spinner::new().size(14.0).color(colours.muted));
            widgets::text(ui, "Applying…", small(), colours.muted);
        } else if gui.restart_waits_for_file() {
            widgets::icon(ui, icons::CIRCLE_ALERT, colours.warn);
            widgets::text(
                ui,
                "Hush applies the change when the configuration file loads again",
                small(),
                colours.muted,
            );
        } else if gui.restart_waits_for_take() {
            widgets::icon(ui, icons::MIC, colours.muted);
            widgets::text(
                ui,
                "Hush applies the change when the take ends",
                small(),
                colours.muted,
            );
        } else if let Some(reason) = gui
            .machine
            .safe_mode
            .clone()
            .filter(|_| gui.machine.running())
        {
            widgets::icon(ui, icons::CIRCLE_ALERT, colours.warn);
            widgets::text(ui, "Hush runs in safe mode", small(), colours.muted)
                .on_hover_text(super::pages::overview::safe_mode_text(&reason));
        } else if gui.machine.running() {
            widgets::icon(ui, icons::CIRCLE_CHECK, colours.good);
            widgets::text(ui, "Hush is running", small(), colours.muted);
        } else {
            widgets::icon(ui, icons::CIRCLE_ALERT, colours.warn);
            widgets::text(ui, "Hush is not running", small(), colours.muted);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            match gui.save_feedback() {
                Some(Feedback::Saved(_)) => {
                    widgets::text(ui, "Saved", small(), colours.muted);
                    widgets::icon(ui, icons::CHECK, colours.good);
                },
                Some(Feedback::Failed(problem)) => {
                    let problem = problem.clone();
                    let room = (ui.available_width() - theme::ICON - 24.0).max(0.0);
                    ui.scope(|ui| {
                        ui.set_max_width(room);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&problem)
                                    .font(small())
                                    .color(colours.bad),
                            )
                            .truncate(),
                        )
                        .on_hover_text(&problem);
                    });
                    widgets::icon(ui, icons::CIRCLE_X, colours.bad);
                },
                None => {},
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_resize_bands_ring_the_window_without_overlapping() {
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 720.0));
        let bands = edges(screen);
        for (i, (_, a, _)) in bands.iter().enumerate() {
            assert!(screen.contains_rect(*a), "{a:?}");
            assert!(a.width() > 0.0 && a.height() > 0.0);
            for (_, b, _) in &bands[i + 1..] {
                let shared = a.intersect(*b);
                assert!(
                    shared.width() <= 0.0 || shared.height() <= 0.0,
                    "{a:?} and {b:?}"
                );
            }
        }
        let corner = |p: Pos2| bands.iter().find(|(_, r, _)| r.contains(p)).map(|b| b.0);
        assert_eq!(
            corner(Pos2::new(1.0, 1.0)),
            Some(ResizeDirection::NorthWest)
        );
        assert_eq!(
            corner(Pos2::new(999.0, 719.0)),
            Some(ResizeDirection::SouthEast)
        );
        assert_eq!(corner(Pos2::new(500.0, 1.0)), Some(ResizeDirection::North));
        assert_eq!(
            corner(Pos2::new(500.0, 360.0)),
            None,
            "the middle is the page"
        );
    }

    #[test]
    fn the_mark_bars_are_symmetric_and_inside_the_tile() {
        for (left, right) in BARS.iter().zip(BARS.iter().rev()) {
            assert_eq!(left, right);
        }
        let span = 4.0 * BAR_STEP + BAR_WIDTH;
        assert!(span < 1.0 - TILE_ROUNDING);
        assert!(BARS.iter().all(|height| *height < 1.0));
    }
}
