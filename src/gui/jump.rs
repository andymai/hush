//! A jump to a settings row, as the command palette starts one: when the
//! row next draws, the page scrolls it into view, its first control takes
//! the keyboard focus, and the row glows for a moment.

use super::theme::{palette, RADIUS_SM};
use egui::{Align, Context, Id, Rect, Response, Ui};

/// How long the row stays fully lit, then how long it fades, in seconds.
const HOLD: f64 = 0.8;
const FADE: f64 = 1.2;

/// The strength of the glow behind the row, as a fraction of the accent.
const GLOW: f32 = 0.16;

#[derive(Clone, Debug, PartialEq)]
struct Jump {
    key: String,
    since: f64,
    scrolled: bool,
    focused: bool,
}

/// The controls of the row that can take the focus, as they draw.
#[derive(Clone, Debug, Default)]
struct Candidates(Vec<(Id, Rect)>);

fn jump_id() -> Id {
    Id::new("hush-jump")
}

fn collecting_id() -> Id {
    Id::new("hush-jump-collecting")
}

fn candidates_id() -> Id {
    Id::new("hush-jump-candidates")
}

/// Jump to the row `key` when it next draws.
pub fn start(ctx: &Context, key: &str) {
    let since = ctx.input(|input| input.time);
    let jump = Jump {
        key: key.to_owned(),
        since,
        scrolled: false,
        focused: false,
    };
    ctx.data_mut(|data| data.insert_temp(jump_id(), jump));
    ctx.request_repaint();
}

/// Forget a jump once its glow is over, also when its row never drew.
pub fn expire(ctx: &Context) {
    let now = ctx.input(|input| input.time);
    ctx.data_mut(|data| {
        let over = data
            .get_temp::<Jump>(jump_id())
            .is_some_and(|jump| now - jump.since > HOLD + FADE);
        if over {
            data.remove::<Jump>(jump_id());
        }
    });
}

/// The key of the row that a jump waits for, if any.
#[cfg(test)]
pub fn target(ctx: &Context) -> Option<String> {
    ctx.data(|data| data.get_temp::<Jump>(jump_id()))
        .map(|jump| jump.key)
}

/// The glow at `elapsed` seconds after the jump: full, then fading to none.
fn glow(elapsed: f64) -> f32 {
    if elapsed < HOLD {
        1.0
    } else {
        (1.0 - (elapsed - HOLD) / FADE).clamp(0.0, 1.0) as f32
    }
}

/// A row that a jump targets, from before its controls draw until after.
pub struct Landing {
    jump: Jump,
    background: egui::layers::ShapeIdx,
}

/// The landing for the row `key`, when a jump targets it. Call before the
/// row paints anything, so the glow goes under the row.
pub fn arrive(ui: &Ui, key: &str) -> Option<Landing> {
    let jump = ui.data(|data| data.get_temp::<Jump>(jump_id()))?;
    if jump.key != key {
        return None;
    }
    if !jump.focused {
        ui.data_mut(|data| {
            data.insert_temp(collecting_id(), true);
            data.insert_temp(candidates_id(), Candidates::default());
        });
    }
    let background = ui.painter().add(egui::Shape::Noop);
    Some(Landing { jump, background })
}

/// A control that can take the focus offers itself to a row that a jump
/// targets.
pub fn offer(ui: &Ui, response: &Response) {
    if !response.sense.focusable || !response.enabled() {
        return;
    }
    let collecting = ui.data(|data| data.get_temp::<bool>(collecting_id()).unwrap_or(false));
    if collecting {
        ui.data_mut(|data| {
            data.get_temp_mut_or_default::<Candidates>(candidates_id())
                .0
                .push((response.id, response.rect));
        });
    }
}

impl Landing {
    /// After the row drew in `rect`: focus its first control from the left,
    /// scroll it into view once, and paint the glow.
    pub fn land(mut self, ui: &Ui, rect: Rect) {
        let ctx = ui.ctx();
        if !self.jump.focused {
            let candidates = ctx.data_mut(|data| {
                data.remove::<bool>(collecting_id());
                data.remove_temp::<Candidates>(candidates_id())
                    .unwrap_or_default()
            });
            if let Some(first) = first_control(&candidates.0) {
                ctx.memory_mut(|memory| memory.request_focus(first));
            }
            self.jump.focused = true;
        }
        if !self.jump.scrolled {
            ui.scroll_to_rect(rect, Some(Align::Center));
            self.jump.scrolled = true;
        }
        let elapsed = ui.input(|input| input.time) - self.jump.since;
        let strength = glow(elapsed);
        if strength > 0.0 {
            let colour = palette(ctx).accent.gamma_multiply(GLOW * strength);
            ui.painter().set(
                self.background,
                egui::epaint::RectShape::filled(rect, RADIUS_SM, colour),
            );
            ctx.request_repaint();
        }
        ctx.data_mut(|data| data.insert_temp(jump_id(), self.jump));
    }
}

/// The control that a reader meets first: the one farthest left, then the
/// highest.
fn first_control(candidates: &[(Id, Rect)]) -> Option<Id> {
    candidates
        .iter()
        .min_by(|(_, a), (_, b)| {
            a.left()
                .total_cmp(&b.left())
                .then(a.top().total_cmp(&b.top()))
        })
        .map(|(id, _)| *id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::{theme, widgets};

    const ROWS: usize = 40;

    /// A page of rows with a switch each, in a scroll area.
    struct Page {
        ctx: Context,
        switches: Vec<bool>,
        ids: Vec<Option<Id>>,
        rects: Vec<Rect>,
        viewport: Rect,
        time: f64,
    }

    impl Page {
        fn new() -> Self {
            let ctx = Context::default();
            theme::install(&ctx);
            Self {
                ctx,
                switches: vec![false; ROWS],
                ids: vec![None; ROWS],
                rects: vec![Rect::NOTHING; ROWS],
                viewport: Rect::NOTHING,
                time: 0.0,
            }
        }

        fn frame(&mut self) {
            self.time += 0.05;
            let input = egui::RawInput {
                time: Some(self.time),
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 400.0),
                )),
                ..Default::default()
            };
            let ctx = self.ctx.clone();
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let shown = egui::ScrollArea::vertical().show(ui, |ui| {
                        for index in 0..ROWS {
                            let key = format!("test.{index}");
                            let response = widgets::switch_row(
                                ui,
                                &key,
                                "Title",
                                "About",
                                &mut self.switches[index],
                            );
                            self.ids[index] = Some(response.id);
                            self.rects[index] = response.rect;
                        }
                    });
                    self.viewport = shown.inner_rect;
                });
            });
        }
    }

    #[test]
    fn a_jump_scrolls_its_row_into_view_and_focuses_its_control() {
        let mut page = Page::new();
        page.frame();
        assert!(
            !page.viewport.intersects(page.rects[30]),
            "row 30 starts out of view"
        );
        start(&page.ctx, "test.30");
        for _ in 0..12 {
            page.frame();
        }
        assert!(
            page.viewport.contains_rect(page.rects[30]),
            "{:?} in {:?}",
            page.rects[30],
            page.viewport
        );
        assert_eq!(page.ctx.memory(|memory| memory.focused()), page.ids[30]);
        assert_eq!(target(&page.ctx).as_deref(), Some("test.30"));
        page.time += HOLD + FADE;
        page.frame();
        expire(&page.ctx);
        assert_eq!(target(&page.ctx), None, "the jump ends with its glow");
    }

    #[test]
    fn a_jump_to_a_row_that_never_draws_ends_with_its_glow() {
        let mut page = Page::new();
        page.frame();
        start(&page.ctx, "test.missing");
        page.frame();
        expire(&page.ctx);
        assert_eq!(target(&page.ctx).as_deref(), Some("test.missing"));
        page.time += HOLD + FADE;
        page.frame();
        expire(&page.ctx);
        assert_eq!(target(&page.ctx), None);
        assert_eq!(page.ctx.memory(|memory| memory.focused()), None);
    }

    #[test]
    fn the_glow_holds_then_fades_to_nothing() {
        assert_eq!(glow(0.0), 1.0);
        assert_eq!(glow(HOLD - 0.01), 1.0);
        let halfway = glow(HOLD + FADE / 2.0);
        assert!((halfway - 0.5).abs() < 0.01, "{halfway}");
        assert_eq!(glow(HOLD + FADE), 0.0);
        assert_eq!(glow(HOLD + FADE + 5.0), 0.0);
    }

    #[test]
    fn the_first_control_is_the_one_farthest_left() {
        let at = |x: f32, y: f32| Rect::from_min_size(egui::pos2(x, y), egui::vec2(20.0, 20.0));
        let (field, clear, below) = (Id::new("field"), Id::new("clear"), Id::new("below"));
        assert_eq!(
            first_control(&[(clear, at(300.0, 0.0)), (field, at(100.0, 0.0))]),
            Some(field)
        );
        assert_eq!(
            first_control(&[(below, at(100.0, 40.0)), (field, at(100.0, 0.0))]),
            Some(field),
            "the higher of two at the same left edge"
        );
        assert_eq!(first_control(&[]), None);
    }
}
