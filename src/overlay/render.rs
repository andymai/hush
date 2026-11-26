//! Draws a scene into a premultiplied RGBA pixmap on the CPU. It needs no
//! GPU and no display, so tests and the showcase draw frames with it too.
//! Shapes are laid out in logical pixels and drawn at the output's scale.

use super::view::{Scene, BARS};
use ab_glyph::{point, Font, FontRef, GlyphId, PxScale, ScaleFont};
use std::sync::LazyLock;
use std::time::Duration;
use tiny_skia::{
    Color, FillRule, LineCap, Paint, Path, PathBuilder, Pixmap, PremultipliedColorU8, Stroke,
    Transform,
};

/// The surface in logical pixels: room for the widest pill, so a badge
/// never resizes the surface. The pill sits in its middle.
pub const SURFACE_WIDTH: u32 = 360;
/// The pill at the bottom, with room above it for the hover hint.
pub const SURFACE_HEIGHT: u32 = 76;
/// The gap between the pill and the surface's bottom edge.
const PILL_BOTTOM: f32 = 8.0;
const HINT_HEIGHT: f32 = 22.0;
const HINT_SIZE: f32 = 11.0;
pub const HINT_TEXT: &str = "Click stops, right-click discards";

/// The gap between the surface and the bottom edge of the screen.
pub const BOTTOM_MARGIN: u32 = 36;

const PILL_HEIGHT: f32 = 32.0;
const PADDING: f32 = 12.0;
const GAP: f32 = 8.0;
const BADGE_GAP: f32 = 6.0;
const DOT_RADIUS: f32 = 4.0;
const BAR_WIDTH: f32 = 3.0;
const BAR_GAP: f32 = 2.0;
const BAR_MIN: f32 = 3.0;
const BAR_MAX: f32 = 18.0;
const TEXT_SIZE: f32 = 12.5;
const BADGE_TEXT_SIZE: f32 = 11.0;
const LOCK_SIZE: f32 = 10.0;
const WORK_DOT_RADIUS: f32 = 2.5;
const WORK_DOT_GAP: f32 = 5.0;
const MAX_MESSAGE: f32 = SURFACE_WIDTH as f32 - 2.0 * PADDING - 2.0 * DOT_RADIUS - GAP - 24.0;

pub const LOADING_TEXT: &str = "Loading the model";
pub const COMMAND_TEXT: &str = "cmd";

/// The colours of the pill.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub background: Color,
    pub border: Color,
    pub foreground: Color,
    pub recording: Color,
    pub warning: Color,
    pub error: Color,
}

/// The default pill: light marks on dark ink.
pub fn dark() -> Palette {
    Palette {
        background: rgba(14, 14, 17, 222),
        border: rgba(255, 255, 255, 36),
        foreground: rgba(242, 240, 234, 235),
        recording: rgba(229, 72, 77, 255),
        warning: rgba(245, 165, 36, 255),
        error: rgba(204, 41, 54, 255),
    }
}

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::from_rgba8(r, g, b, a)
}

static FONT: LazyLock<Option<FontRef<'static>>> =
    LazyLock::new(|| FontRef::try_from_slice(crate::fonts::GEIST_MEDIUM).ok());

/// The pill's rectangle in the surface, in logical pixels: left, top,
/// width, height. Only this part takes clicks, and only while a take records.
pub fn pill_rect(scene: &Scene) -> (f32, f32, f32, f32) {
    let width = pill_width(scene);
    (
        (SURFACE_WIDTH as f32 - width) / 2.0,
        SURFACE_HEIGHT as f32 - PILL_BOTTOM - PILL_HEIGHT,
        width,
        PILL_HEIGHT,
    )
}

/// `None` when the scale makes an empty surface. With `hint`, a line above
/// the pill names what a click does.
pub fn render(
    scene: &Scene,
    palette: &Palette,
    scale: f32,
    seconds: f32,
    hint: bool,
) -> Option<Pixmap> {
    let width = (SURFACE_WIDTH as f32 * scale).round() as u32;
    let height = (SURFACE_HEIGHT as f32 * scale).round() as u32;
    let mut pixmap = Pixmap::new(width, height)?;
    let (left, top, pill, _) = pill_rect(scene);
    let mut canvas = Canvas {
        pixmap: &mut pixmap,
        scale,
    };
    if hint {
        let text = text_width(HINT_TEXT, HINT_SIZE);
        let bubble = text + 2.0 * PADDING;
        let bubble_left = (SURFACE_WIDTH as f32 - bubble) / 2.0;
        let bubble_top = top - HINT_HEIGHT - 4.0;
        canvas.fill(
            rounded_rect(
                bubble_left,
                bubble_top,
                bubble,
                HINT_HEIGHT,
                HINT_HEIGHT / 2.0,
            ),
            palette.background,
        );
        canvas.text(
            HINT_TEXT,
            bubble_left + PADDING,
            bubble_top + HINT_HEIGHT / 2.0,
            HINT_SIZE,
            palette.foreground,
        );
    }
    canvas.fill(
        rounded_rect(left, top, pill, PILL_HEIGHT, PILL_HEIGHT / 2.0),
        palette.background,
    );
    canvas.stroke(
        rounded_rect(
            left + 0.5,
            top + 0.5,
            pill - 1.0,
            PILL_HEIGHT - 1.0,
            PILL_HEIGHT / 2.0,
        ),
        palette.border,
        1.0,
    );
    let middle = top + PILL_HEIGHT / 2.0;
    let mut x = left + PADDING;
    match scene {
        Scene::Recording {
            elapsed,
            levels,
            locked,
            command,
            remaining,
        } => {
            canvas.fill(
                circle(x + DOT_RADIUS, middle, DOT_RADIUS),
                palette.recording,
            );
            x += 2.0 * DOT_RADIUS + GAP;
            for level in levels {
                let height = BAR_MIN + (BAR_MAX - BAR_MIN) * level.clamp(0.0, 1.0);
                canvas.fill(
                    rounded_rect(x, middle - height / 2.0, BAR_WIDTH, height, BAR_WIDTH / 2.0),
                    palette.foreground,
                );
                x += BAR_WIDTH + BAR_GAP;
            }
            x += GAP - BAR_GAP;
            let (time, colour) = match remaining {
                Some(left) => (clock(*left), palette.warning),
                None => (clock(*elapsed), palette.foreground),
            };
            x += canvas.text(&time, x, middle, TEXT_SIZE, colour);
            if *locked {
                x += BADGE_GAP;
                canvas.lock(x, middle, palette.foreground);
                x += LOCK_SIZE;
            }
            if *command {
                x += BADGE_GAP;
                canvas.text(COMMAND_TEXT, x, middle, BADGE_TEXT_SIZE, palette.warning);
            }
        },
        Scene::Working { loading } => {
            for dot in 0..3 {
                let wave = (seconds * std::f32::consts::TAU * 1.2 - dot as f32 * 0.7).sin();
                let alpha = 0.35 + 0.65 * (0.5 + 0.5 * wave);
                let mut colour = palette.foreground;
                colour.set_alpha(colour.alpha() * alpha);
                canvas.fill(circle(x + WORK_DOT_RADIUS, middle, WORK_DOT_RADIUS), colour);
                x += 2.0 * WORK_DOT_RADIUS + WORK_DOT_GAP;
            }
            if *loading {
                x += GAP - WORK_DOT_GAP;
                canvas.text(LOADING_TEXT, x, middle, TEXT_SIZE, palette.foreground);
            }
        },
        Scene::Error { message } => {
            canvas.fill(circle(x + DOT_RADIUS, middle, DOT_RADIUS), palette.error);
            x += 2.0 * DOT_RADIUS + GAP;
            canvas.text(&fit(message), x, middle, TEXT_SIZE, palette.foreground);
        },
    }
    Some(pixmap)
}

/// The pill's width for `scene`, in logical pixels.
pub fn pill_width(scene: &Scene) -> f32 {
    let content = match scene {
        Scene::Recording {
            elapsed,
            locked,
            command,
            remaining,
            ..
        } => {
            let bars = BARS as f32 * BAR_WIDTH + (BARS - 1) as f32 * BAR_GAP;
            let time = clock(remaining.unwrap_or(*elapsed));
            let mut width = 2.0 * DOT_RADIUS + GAP + bars + GAP + text_width(&time, TEXT_SIZE);
            if *locked {
                width += BADGE_GAP + LOCK_SIZE;
            }
            if *command {
                width += BADGE_GAP + text_width(COMMAND_TEXT, BADGE_TEXT_SIZE);
            }
            width
        },
        Scene::Working { loading } => {
            let dots = 3.0 * 2.0 * WORK_DOT_RADIUS + 2.0 * WORK_DOT_GAP;
            if *loading {
                dots + GAP + text_width(LOADING_TEXT, TEXT_SIZE)
            } else {
                dots
            }
        },
        Scene::Error { message } => 2.0 * DOT_RADIUS + GAP + text_width(&fit(message), TEXT_SIZE),
    };
    (content + 2.0 * PADDING).max(PILL_HEIGHT)
}

/// `m:ss`
pub fn clock(time: Duration) -> String {
    let seconds = time.as_secs();
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// The message, cut with an ellipsis where it would leave the surface.
fn fit(message: &str) -> String {
    if text_width(message, TEXT_SIZE) <= MAX_MESSAGE {
        return message.to_string();
    }
    let mut cut: String = message.to_string();
    while !cut.is_empty() {
        cut.pop();
        let candidate = format!("{}…", cut.trim_end());
        if text_width(&candidate, TEXT_SIZE) <= MAX_MESSAGE {
            return candidate;
        }
    }
    "…".to_string()
}

/// The advance of `text` at `size` logical pixels.
pub fn text_width(text: &str, size: f32) -> f32 {
    let Some(font) = FONT.as_ref() else {
        return 0.0;
    };
    let scaled = font.as_scaled(PxScale::from(size));
    let mut width = 0.0;
    let mut previous: Option<GlyphId> = None;
    for c in text.chars() {
        let id = font.glyph_id(c);
        if let Some(previous) = previous {
            width += scaled.kern(previous, id);
        }
        width += scaled.h_advance(id);
        previous = Some(id);
    }
    width
}

fn rounded_rect(x: f32, y: f32, width: f32, height: f32, radius: f32) -> Option<Path> {
    let r = radius.min(width / 2.0).min(height / 2.0);
    // The control-point distance that makes a cubic close to a quarter circle.
    let k = 0.552_284_8 * r;
    let (right, bottom) = (x + width, y + height);
    let mut path = PathBuilder::new();
    path.move_to(x + r, y);
    path.line_to(right - r, y);
    path.cubic_to(right - r + k, y, right, y + r - k, right, y + r);
    path.line_to(right, bottom - r);
    path.cubic_to(
        right,
        bottom - r + k,
        right - r + k,
        bottom,
        right - r,
        bottom,
    );
    path.line_to(x + r, bottom);
    path.cubic_to(x + r - k, bottom, x, bottom - r + k, x, bottom - r);
    path.line_to(x, y + r);
    path.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    path.close();
    path.finish()
}

fn circle(x: f32, y: f32, radius: f32) -> Option<Path> {
    PathBuilder::from_circle(x, y, radius)
}

struct Canvas<'a> {
    pixmap: &'a mut Pixmap,
    scale: f32,
}

impl Canvas<'_> {
    fn transform(&self) -> Transform {
        Transform::from_scale(self.scale, self.scale)
    }

    fn fill(&mut self, path: Option<Path>, colour: Color) {
        let Some(path) = path else {
            return;
        };
        let mut paint = Paint::default();
        paint.set_color(colour);
        paint.anti_alias = true;
        let transform = self.transform();
        self.pixmap
            .fill_path(&path, &paint, FillRule::Winding, transform, None);
    }

    fn stroke(&mut self, path: Option<Path>, colour: Color, width: f32) {
        let Some(path) = path else {
            return;
        };
        let mut paint = Paint::default();
        paint.set_color(colour);
        paint.anti_alias = true;
        let stroke = Stroke {
            width,
            line_cap: LineCap::Round,
            ..Stroke::default()
        };
        let transform = self.transform();
        self.pixmap
            .stroke_path(&path, &paint, &stroke, transform, None);
    }

    /// A padlock: a body and a shackle above it, `LOCK_SIZE` wide.
    fn lock(&mut self, x: f32, middle: f32, colour: Color) {
        let body_height = LOCK_SIZE * 0.6;
        let body_top = middle - body_height / 2.0 + 1.5;
        self.fill(
            rounded_rect(x, body_top, LOCK_SIZE, body_height, 1.5),
            colour,
        );
        let inset = LOCK_SIZE * 0.22;
        let (left, right) = (x + inset, x + LOCK_SIZE - inset);
        let top = body_top - LOCK_SIZE * 0.45;
        let mut shackle = PathBuilder::new();
        shackle.move_to(left, body_top);
        shackle.line_to(left, top + (right - left) / 2.0);
        shackle.quad_to(left, top, (left + right) / 2.0, top);
        shackle.quad_to(right, top, right, top + (right - left) / 2.0);
        shackle.line_to(right, body_top);
        self.stroke(shackle.finish(), colour, 1.5);
    }

    /// Draws `text` with its left edge at `x` and its middle at `middle`,
    /// and returns its width, all in logical pixels.
    fn text(&mut self, text: &str, x: f32, middle: f32, size: f32, colour: Color) -> f32 {
        let Some(font) = FONT.as_ref() else {
            return 0.0;
        };
        let scale = PxScale::from(size * self.scale);
        let scaled = font.as_scaled(scale);
        let baseline = middle * self.scale + (scaled.ascent() + scaled.descent()) / 2.0;
        let mut caret = x * self.scale;
        let mut previous: Option<GlyphId> = None;
        for c in text.chars() {
            let id = font.glyph_id(c);
            if let Some(previous) = previous {
                caret += scaled.kern(previous, id);
            }
            let glyph = id.with_scale_and_position(scale, point(caret, baseline));
            caret += scaled.h_advance(id);
            previous = Some(id);
            let Some(outline) = font.outline_glyph(glyph) else {
                continue;
            };
            let bounds = outline.px_bounds();
            outline.draw(|gx, gy, coverage| {
                blend(
                    self.pixmap,
                    bounds.min.x as i32 + gx as i32,
                    bounds.min.y as i32 + gy as i32,
                    colour,
                    coverage,
                );
            });
        }
        caret / self.scale - x
    }
}

/// Source-over of `colour` at `coverage` onto one premultiplied pixel.
fn blend(pixmap: &mut Pixmap, x: i32, y: i32, colour: Color, coverage: f32) {
    let (width, height) = (pixmap.width() as i32, pixmap.height() as i32);
    if x < 0 || y < 0 || x >= width || y >= height {
        return;
    }
    let index = (y * width + x) as usize;
    let pixels = pixmap.pixels_mut();
    let below = pixels[index];
    let alpha = colour.alpha() * coverage.clamp(0.0, 1.0);
    let keep = 1.0 - alpha;
    let mix = |source: f32, below: u8| (source * alpha * 255.0 + below as f32 * keep).round();
    let a = (alpha * 255.0 + below.alpha() as f32 * keep)
        .round()
        .min(255.0);
    let channel = |value: f32| value.min(a) as u8;
    let mixed = PremultipliedColorU8::from_rgba(
        channel(mix(colour.red(), below.red())),
        channel(mix(colour.green(), below.green())),
        channel(mix(colour.blue(), below.blue())),
        a as u8,
    );
    if let Some(mixed) = mixed {
        pixels[index] = mixed;
    }
}

/// Premultiplied BGRA bytes, the layout of wl_shm ARGB8888 and of an X11
/// depth-32 image on a little-endian machine.
pub fn to_bgra(pixmap: &Pixmap) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(pixmap.data().len());
    for pixel in pixmap.pixels() {
        bytes.extend_from_slice(&[pixel.blue(), pixel.green(), pixel.red(), pixel.alpha()]);
    }
    bytes
}

/// Straight (not premultiplied) RGBA bytes, for a PNG.
pub fn to_rgba(pixmap: &Pixmap) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(pixmap.data().len());
    for pixel in pixmap.pixels() {
        let colour = pixel.demultiply();
        bytes.extend_from_slice(&[colour.red(), colour.green(), colour.blue(), colour.alpha()]);
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recording(locked: bool, command: bool, remaining: Option<Duration>) -> Scene {
        Scene::Recording {
            elapsed: Duration::from_secs(7),
            levels: [0.0, 0.1, 0.3, 0.6, 1.0, 0.6, 0.3, 0.1, 0.0],
            locked,
            command,
            remaining,
        }
    }

    fn alpha_at(pixmap: &Pixmap, x: u32, y: u32) -> u8 {
        pixmap.pixel(x, y).map_or(0, |pixel| pixel.alpha())
    }

    #[test]
    fn the_font_loads_and_measures_text() {
        assert!(FONT.is_some());
        assert!(text_width("0:07", TEXT_SIZE) > 10.0);
        assert!(text_width("10:07", TEXT_SIZE) > text_width("0:07", TEXT_SIZE));
    }

    #[test]
    fn the_pill_sits_in_the_middle_with_clear_corners() {
        for scale in [1.0, 2.0] {
            let pixmap =
                render(&recording(false, false, None), &dark(), scale, 0.0, false).unwrap();
            assert_eq!(pixmap.width(), (SURFACE_WIDTH as f32 * scale) as u32);
            assert_eq!(pixmap.height(), (SURFACE_HEIGHT as f32 * scale) as u32);
            let (w, h) = (pixmap.width(), pixmap.height());
            assert_eq!(alpha_at(&pixmap, 0, 0), 0, "the corner is clear");
            assert_eq!(alpha_at(&pixmap, w / 2, 1), 0, "above the pill is clear");
            assert!(alpha_at(&pixmap, w / 2, h / 2) > 200, "the pill is opaque");
        }
    }

    #[test]
    fn badges_widen_the_pill_and_fit_the_surface() {
        let plain = pill_width(&recording(false, false, None));
        let locked = pill_width(&recording(true, false, None));
        let both = pill_width(&recording(true, true, Some(Duration::from_secs(45))));
        assert!(plain < locked && locked < both);
        assert!(both < SURFACE_WIDTH as f32);
        assert!(pill_width(&Scene::Working { loading: true }) < SURFACE_WIDTH as f32);
    }

    #[test]
    fn a_long_error_is_cut_to_fit() {
        let message = "The service refused the key: check GROQ_API_KEY in ~/.config/hush/.env \
                       and try again after the quota resets";
        let fitted = fit(message);
        assert!(fitted.ends_with('…'));
        assert!(text_width(&fitted, TEXT_SIZE) <= MAX_MESSAGE);
        assert!(
            pill_width(&Scene::Error {
                message: message.into()
            }) <= SURFACE_WIDTH as f32
        );
        assert_eq!(fit("Undo failed"), "Undo failed");
    }

    #[test]
    fn clocks_read_minutes_and_seconds() {
        assert_eq!(clock(Duration::from_millis(7_900)), "0:07");
        assert_eq!(clock(Duration::from_secs(605)), "10:05");
    }

    #[test]
    fn bgra_swaps_red_and_blue() {
        let mut pixmap = Pixmap::new(1, 1).unwrap();
        pixmap.fill(Color::from_rgba8(255, 0, 0, 255));
        assert_eq!(to_bgra(&pixmap), [0, 0, 255, 255]);
        assert_eq!(to_rgba(&pixmap), [255, 0, 0, 255]);
    }

    /// With `HUSH_OVERLAY_FRAMES` set to a folder, writes each scene as a
    /// PNG there, to look at the pill without a desktop.
    #[test]
    fn frames_can_be_written_for_a_look() {
        let Some(dir) = std::env::var_os("HUSH_OVERLAY_FRAMES") else {
            return;
        };
        let scenes = [
            ("recording", recording(false, false, None)),
            ("locked", recording(true, false, None)),
            ("command", recording(false, true, None)),
            ("cap", recording(true, false, Some(Duration::from_secs(45)))),
            ("working", Scene::Working { loading: false }),
            ("loading", Scene::Working { loading: true }),
            (
                "error",
                Scene::Error {
                    message: "The microphone did not start".into(),
                },
            ),
        ];
        for (name, scene) in scenes {
            let pixmap = render(&scene, &dark(), 2.0, 0.3, false).unwrap();
            let image =
                image::RgbaImage::from_raw(pixmap.width(), pixmap.height(), to_rgba(&pixmap))
                    .unwrap();
            image
                .save(std::path::Path::new(&dir).join(format!("{}.png", name)))
                .unwrap();
        }
    }

    #[test]
    fn the_hint_shows_above_the_pill_only_on_hover() {
        let scene = recording(true, false, None);
        let (_, top, _, height) = pill_rect(&scene);
        assert_eq!(top + height + PILL_BOTTOM, SURFACE_HEIGHT as f32);
        let above = |pixmap: &Pixmap| {
            let row = (top - HINT_HEIGHT / 2.0 - 4.0) as u32;
            (0..pixmap.width()).any(|x| alpha_at(pixmap, x, row) > 0)
        };
        assert!(!above(&render(&scene, &dark(), 1.0, 0.0, false).unwrap()));
        assert!(above(&render(&scene, &dark(), 1.0, 0.0, true).unwrap()));
        assert!(text_width(HINT_TEXT, HINT_SIZE) + 2.0 * PADDING < SURFACE_WIDTH as f32);
    }
}
