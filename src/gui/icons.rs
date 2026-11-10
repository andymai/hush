//! Lucide icons by name, as code points in the embedded icon font. The font
//! holds only these icons: `scripts/icon-font.sh` makes it.

pub const APP_WINDOW: char = '\u{e426}';
pub const CHECK: char = '\u{e06c}';
pub const CHEVRON_DOWN: char = '\u{e06d}';
pub const CIRCLE_ALERT: char = '\u{e077}';
pub const CIRCLE_CHECK: char = '\u{e226}';
pub const CIRCLE_STOP: char = '\u{e083}';
pub const CIRCLE_X: char = '\u{e084}';
pub const COPY: char = '\u{e09e}';
pub const DOWNLOAD: char = '\u{e0b2}';
pub const FILE_INPUT: char = '\u{e0c5}';
pub const FILE_OUTPUT: char = '\u{e0c8}';
pub const FILE_TEXT: char = '\u{e0cc}';
pub const HISTORY: char = '\u{e1f5}';
pub const HOUSE: char = '\u{e0f5}';
pub const INFO: char = '\u{e0f9}';
pub const KEYBOARD: char = '\u{e284}';
pub const LANGUAGES: char = '\u{e0fe}';
pub const MIC: char = '\u{e118}';
pub const MINUS: char = '\u{e11c}';
pub const MONITOR: char = '\u{e11d}';
pub const PEN_LINE: char = '\u{e130}';
pub const PENCIL: char = '\u{e1f9}';
pub const PLAY: char = '\u{e13c}';
pub const PLUS: char = '\u{e13d}';
pub const POWER: char = '\u{e140}';
pub const REFRESH_CW: char = '\u{e145}';
pub const ROTATE_CCW: char = '\u{e148}';
pub const ROTATE_CW: char = '\u{e149}';
pub const SEARCH: char = '\u{e151}';
pub const SHIELD_CHECK: char = '\u{e1ff}';
pub const SQUARE: char = '\u{e167}';
pub const TEXT_CURSOR_INPUT: char = '\u{e265}';
pub const TRASH: char = '\u{e18e}';
pub const TRIANGLE_ALERT: char = '\u{e193}';
pub const TYPE: char = '\u{e198}';
pub const WHOLE_WORD: char = '\u{e3df}';
pub const X: char = '\u{e1b2}';

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::theme;

    #[test]
    fn the_font_has_every_icon() {
        let fonts = egui::epaint::text::Fonts::new(1.0, 2048, theme::fonts());
        let font = theme::icon_font(theme::ICON);
        for icon in [
            APP_WINDOW,
            CHECK,
            CHEVRON_DOWN,
            CIRCLE_ALERT,
            CIRCLE_CHECK,
            CIRCLE_STOP,
            CIRCLE_X,
            COPY,
            DOWNLOAD,
            FILE_INPUT,
            FILE_OUTPUT,
            FILE_TEXT,
            HISTORY,
            HOUSE,
            INFO,
            KEYBOARD,
            LANGUAGES,
            MIC,
            MINUS,
            MONITOR,
            PEN_LINE,
            PENCIL,
            PLAY,
            PLUS,
            POWER,
            REFRESH_CW,
            ROTATE_CCW,
            ROTATE_CW,
            SEARCH,
            SHIELD_CHECK,
            SQUARE,
            TEXT_CURSOR_INPUT,
            TRASH,
            TRIANGLE_ALERT,
            TYPE,
            WHOLE_WORD,
            X,
        ] {
            assert!(fonts.has_glyph(&font, icon), "{:x}", icon as u32);
        }
    }
}
