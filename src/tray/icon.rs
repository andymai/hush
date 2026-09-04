//! The Hush mark, drawn in code so the tray looks the same whether Hush was
//! installed from a package or run from a checkout.
//!
//! The mark is the five-bar waveform the overlay animates while recording, so
//! the tray, the overlay, and the application icon read as one thing. The fill
//! carries the state, in mid tones that hold their own against light and dark
//! panels alike; a contrasting outline is not an option because at 22 pixels
//! it closes the gaps between the bars.

use ksni::Icon;

const SIZES: [i32; 4] = [22, 24, 32, 48];
const SAMPLES: i32 = 3;

/// Bar heights as a fraction of the icon, centre bar tallest.
const BARS: [f32; 5] = [0.30, 0.54, 0.78, 0.54, 0.30];
const BAR_WIDTH: f32 = 0.095;
const BAR_GAP: f32 = 0.085;

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

/// Distance from `p` to the segment `a`-`b`, minus `r`.
fn capsule(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32, r: f32) -> f32 {
    let (pax, pay) = (px - ax, py - ay);
    let (bax, bay) = (bx - ax, by - ay);
    let denom = bax * bax + bay * bay;
    let h = if denom == 0.0 {
        0.0
    } else {
        ((pax * bax + pay * bay) / denom).clamp(0.0, 1.0)
    };
    let (dx, dy) = (pax - bax * h, pay - bay * h);
    (dx * dx + dy * dy).sqrt() - r
}

/// Signed distance to the waveform in a unit square.
fn waveform(x: f32, y: f32) -> f32 {
    let radius = BAR_WIDTH / 2.0;
    let span = BARS.len() as f32 * BAR_WIDTH + (BARS.len() - 1) as f32 * BAR_GAP;
    let left = (1.0 - span) / 2.0 + radius;
    let mut d = f32::MAX;
    for (i, height) in BARS.iter().enumerate() {
        let cx = left + i as f32 * (BAR_WIDTH + BAR_GAP);
        let half = (height / 2.0 - radius).max(0.0);
        d = d.min(capsule(x, y, cx, 0.5 - half, cx, 0.5 + half, radius));
    }
    d
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
                    if waveform(fx, fy) < 0.0 {
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

    /// Opaque pixels in the column through the centre of bar `index`.
    fn bar_height(icon: &Icon, index: usize) -> usize {
        let centre = BARS.len() as f32 * BAR_WIDTH + (BARS.len() - 1) as f32 * BAR_GAP;
        let left = (1.0 - centre) / 2.0 + BAR_WIDTH / 2.0;
        let x = ((left + index as f32 * (BAR_WIDTH + BAR_GAP)) * icon.width as f32) as i32;
        (0..icon.height)
            .filter(|y| alpha_at(icon, x, *y) > 200)
            .count()
    }

    #[test]
    fn bars_rise_towards_the_centre_and_the_edges_stay_clear() {
        let icon = &pixmaps(IconState::Recording)[3];
        let heights: Vec<usize> = (0..BARS.len()).map(|i| bar_height(icon, i)).collect();
        assert!(heights[0] > 0, "every bar is drawn: {:?}", heights);
        assert!(heights[1] > heights[0], "bars grow inwards: {:?}", heights);
        assert!(
            heights[2] > heights[1],
            "the centre is tallest: {:?}",
            heights
        );

        let size = icon.width;
        for x in 0..size {
            assert_eq!(alpha_at(icon, x, 0), 0, "the top row is clear at {x}");
            assert_eq!(
                alpha_at(icon, x, size - 1),
                0,
                "the bottom row is clear at {x}"
            );
        }
        assert_eq!(alpha_at(icon, 0, size / 2), 0, "the left edge is clear");
    }

    #[test]
    fn the_mark_is_symmetric() {
        let icon = &pixmaps(IconState::Idle)[2];
        let size = icon.width;
        for y in 0..size {
            for x in 0..size {
                assert_eq!(
                    alpha_at(icon, x, y),
                    alpha_at(icon, size - 1 - x, y),
                    "mirrored at ({x}, {y})"
                );
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
