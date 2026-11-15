//! The shape of the Hush mark: three bars of sound that end in a text cursor,
//! on a plate. The numbers are in the units of the 128-pixel application icon,
//! `packaging/io.github.andymai.hush.svg`, so the tray icon and the settings
//! sidebar draw the same shape as the icon. Change both together.

/// A rectangle whose short ends are half circles, by its centre and size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pill {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Pill {
    pub fn radius(&self) -> f32 {
        self.width.min(self.height) / 2.0
    }

    /// Signed distance from the point to the pill: negative inside.
    pub fn distance(&self, x: f32, y: f32) -> f32 {
        let r = self.radius();
        let dx = ((x - self.x).abs() - (self.width / 2.0 - r)).max(0.0);
        let dy = ((y - self.y).abs() - (self.height / 2.0 - r)).max(0.0);
        (dx * dx + dy * dy).sqrt() - r
    }

    pub fn top(&self) -> f32 {
        self.y - self.height / 2.0
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height / 2.0
    }
}

/// The sound bars, from left to right.
pub const BARS: [Pill; 3] = [
    Pill {
        x: 33.0,
        y: 61.0,
        width: 10.0,
        height: 28.0,
    },
    Pill {
        x: 50.0,
        y: 61.0,
        width: 10.0,
        height: 50.0,
    },
    Pill {
        x: 67.0,
        y: 61.0,
        width: 10.0,
        height: 34.0,
    },
];

/// The text cursor: the stem, then the serifs at its top and bottom.
pub const CURSOR: [Pill; 3] = [
    Pill {
        x: 89.0,
        y: 61.0,
        width: 8.0,
        height: 66.0,
    },
    Pill {
        x: 89.0,
        y: 31.5,
        width: 20.0,
        height: 7.0,
    },
    Pill {
        x: 89.0,
        y: 90.5,
        width: 20.0,
        height: 7.0,
    },
];

/// Left, top, right, and bottom of the bars and the cursor together.
pub const GLYPH: [f32; 4] = [28.0, 28.0, 99.0, 94.0];

/// Left, top, right, and bottom of the plate, with its edge.
pub const PLATE: [f32; 4] = [12.0, 12.0, 116.0, 116.0];
/// How far the darker edge shows below the face of the plate.
pub const PLATE_EDGE: f32 = 6.0;
pub const PLATE_ROUNDING: f32 = 24.0;

/// Signed distance from the point to the bars and the cursor.
pub fn glyph_distance(x: f32, y: f32) -> f32 {
    BARS.iter()
        .chain(CURSOR.iter())
        .map(|pill| pill.distance(x, y))
        .fold(f32::MAX, f32::min)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_glyph_box_holds_every_part_exactly() {
        let parts: Vec<&Pill> = BARS.iter().chain(CURSOR.iter()).collect();
        let left = parts
            .iter()
            .map(|p| p.x - p.width / 2.0)
            .fold(f32::MAX, f32::min);
        let right = parts
            .iter()
            .map(|p| p.x + p.width / 2.0)
            .fold(f32::MIN, f32::max);
        let top = parts.iter().map(|p| p.top()).fold(f32::MAX, f32::min);
        let bottom = parts.iter().map(|p| p.bottom()).fold(f32::MIN, f32::max);
        assert_eq!([left, top, right, bottom], GLYPH);
    }

    #[test]
    fn the_glyph_is_centred_on_the_face() {
        let [left, top, right, bottom] = GLYPH;
        let face_x = (PLATE[0] + PLATE[2]) / 2.0;
        let face_y = (PLATE[1] + PLATE[3] - PLATE_EDGE) / 2.0;
        assert!(((left + right) / 2.0 - face_x).abs() <= 0.5);
        assert_eq!((top + bottom) / 2.0, face_y);
    }

    #[test]
    fn the_cursor_is_the_tallest_part() {
        let stem = CURSOR[0];
        assert!(BARS.iter().all(|bar| bar.height < stem.height));
        assert_eq!(CURSOR[1].top(), stem.top(), "the top serif caps the stem");
        assert_eq!(
            CURSOR[2].bottom(),
            stem.bottom(),
            "the bottom serif caps the stem"
        );
    }

    #[test]
    fn a_pill_is_negative_inside_and_positive_outside() {
        let bar = BARS[1];
        assert!(bar.distance(bar.x, bar.y) < 0.0);
        assert!(bar.distance(bar.x, bar.top() + 0.5) < 0.0);
        assert!(bar.distance(bar.x, bar.top() - 0.5) > 0.0);
        assert!(bar.distance(bar.x + bar.width, bar.y) > 0.0);
        let corner = bar.distance(bar.x - bar.width / 2.0 + 0.2, bar.top() + 0.2);
        assert!(corner > 0.0, "the ends are round");
    }
}
