//! The Hush mark, drawn in code so the tray looks the same whether Hush was
//! installed from a package or run from a checkout.
//!
//! The tray draws the bars and the cursor of the application icon without the
//! plate. The fill carries the state, in mid tones that hold their own against
//! light and dark panels alike; a contrasting outline is not an option because
//! at 22 pixels it closes the gaps between the bars.

use crate::mark;
use ksni::Icon;

const SIZES: [i32; 4] = [22, 24, 32, 48];
const SAMPLES: i32 = 3;

/// The share of the icon width that the mark fills.
const SPAN: f32 = 0.875;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconState {
    Idle,
    Recording,
    Busy,
}

impl IconState {
    fn fill(self) -> [u8; 3] {
        match self {
            IconState::Idle => [0x6b, 0x72, 0x80],
            IconState::Recording => [0xd9, 0x3c, 0x33],
            IconState::Busy => [0xd9, 0x8f, 0x28],
        }
    }
}

/// Mark units across the whole icon.
fn scale() -> f32 {
    let [left, _, right, _] = mark::GLYPH;
    (right - left) / SPAN
}

/// The point of the unit square, in mark units, with the glyph centred.
fn to_mark(x: f32, y: f32) -> (f32, f32) {
    let [left, top, right, bottom] = mark::GLYPH;
    let scale = scale();
    (
        (left + right) / 2.0 + (x - 0.5) * scale,
        (top + bottom) / 2.0 + (y - 0.5) * scale,
    )
}

/// Signed distance to the mark in a unit square.
fn glyph(x: f32, y: f32) -> f32 {
    let (mx, my) = to_mark(x, y);
    mark::glyph_distance(mx, my) / scale()
}

fn render(size: i32, state: IconState) -> Icon {
    let colour = state.fill();
    let mut data = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let mut covered = 0u32;
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let fx = (x as f32 + (sx as f32 + 0.5) / SAMPLES as f32) / size as f32;
                    let fy = (y as f32 + (sy as f32 + 0.5) / SAMPLES as f32) / size as f32;
                    if glyph(fx, fy) < 0.0 {
                        covered += 1;
                    }
                }
            }
            if covered == 0 {
                continue;
            }
            let alpha = (covered as f32 / (SAMPLES * SAMPLES) as f32 * 255.0) as u8;
            let i = ((y * size + x) * 4) as usize;
            // ARGB32, network byte order, with premultiplied colour channels.
            let premultiply = |c: u8| ((c as u32 * alpha as u32) / 255) as u8;
            data[i] = alpha;
            data[i + 1] = premultiply(colour[0]);
            data[i + 2] = premultiply(colour[1]);
            data[i + 3] = premultiply(colour[2]);
        }
    }
    Icon {
        width: size,
        height: size,
        data,
    }
}

pub fn pixmaps(state: IconState) -> Vec<Icon> {
    SIZES.iter().map(|size| render(*size, state)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha_at(icon: &Icon, x: i32, y: i32) -> u8 {
        icon.data[((y * icon.width + x) * 4) as usize]
    }

    #[test]
    fn every_size_is_rendered_with_the_right_buffer_length() {
        let icons = pixmaps(IconState::Idle);
        assert_eq!(icons.len(), SIZES.len());
        for icon in &icons {
            assert_eq!(icon.width, icon.height);
            assert_eq!(icon.data.len(), (icon.width * icon.height * 4) as usize);
        }
    }

    /// Opaque pixels in the column through the centre of `part`.
    fn column_height(icon: &Icon, part: &mark::Pill) -> usize {
        let [left, _, right, _] = mark::GLYPH;
        let fx = 0.5 + (part.x - (left + right) / 2.0) / scale();
        let x = (fx * icon.width as f32) as i32;
        (0..icon.height)
            .filter(|y| alpha_at(icon, x, *y) > 200)
            .count()
    }

    #[test]
    fn the_cursor_is_tallest_and_the_edges_stay_clear() {
        let icon = &pixmaps(IconState::Recording)[3];
        let bars: Vec<usize> = mark::BARS
            .iter()
            .map(|bar| column_height(icon, bar))
            .collect();
        let cursor = column_height(icon, &mark::CURSOR[0]);
        assert!(bars.iter().all(|h| *h > 0), "every bar is drawn: {bars:?}");
        assert!(
            bars[1] > bars[0] && bars[1] > bars[2],
            "the middle bar is the tallest bar: {bars:?}"
        );
        assert!(
            bars.iter().all(|h| *h < cursor),
            "the cursor is taller than the bars: {bars:?} < {cursor}"
        );

        let size = icon.width;
        for i in 0..size {
            for (x, y) in [(i, 0), (i, size - 1), (0, i), (size - 1, i)] {
                assert_eq!(alpha_at(icon, x, y), 0, "the edge is clear at ({x}, {y})");
            }
        }
    }

    #[test]
    fn the_mark_is_symmetric_top_to_bottom() {
        for icon in pixmaps(IconState::Idle) {
            let size = icon.width;
            for y in 0..size {
                for x in 0..size {
                    assert_eq!(
                        alpha_at(&icon, x, y),
                        alpha_at(&icon, x, size - 1 - y),
                        "mirrored at ({x}, {y}) in the {size} pixel icon"
                    );
                }
            }
        }
    }

    #[test]
    fn states_differ_in_colour_only() {
        let idle = &pixmaps(IconState::Idle)[0];
        let recording = &pixmaps(IconState::Recording)[0];
        let alphas_match = idle
            .data
            .chunks(4)
            .zip(recording.data.chunks(4))
            .all(|(a, b)| a[0] == b[0]);
        assert!(alphas_match, "the shape does not change with state");
        assert_ne!(idle.data, recording.data, "the colour does");
    }
}
