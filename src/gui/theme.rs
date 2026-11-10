//! Colours, type, and measurements shared by every part of the window, in a
//! light and a dark scheme.
//!
//! The neutrals, the spacing, and the type scale follow one design system:
//! a narrow column of settings rows on a white or near-black surface, with a
//! sidebar on a slightly darker ground. The accent is Hush's violet, from the
//! application icon, and marks focus, progress, and the level meter.

use crate::config::settings::Appearance;
use egui::epaint::Shadow;
use egui::{
    Color32, Context, FontData, FontDefinitions, FontFamily, FontId, Rounding, Stroke, TextStyle,
    Theme,
};

/// The colours of one scheme.
#[derive(Debug, PartialEq, Eq)]
pub struct Palette {
    /// The sidebar ground
    pub bg: Color32,
    /// The page, the title bar, and the footer
    pub surface: Color32,
    /// Menus and tooltips
    pub raised: Color32,
    /// The ground of an empty switch or meter
    pub sunken: Color32,
    /// The open page in the sidebar and the chosen part of a segmented control
    pub selection: Color32,
    pub hover: Color32,
    /// Dividers between rows and panels; never the only edge of a control
    pub line: Color32,
    /// The edge of a field, a button, or a switch
    pub control: Color32,
    pub ink: Color32,
    /// Descriptions, hints, and placeholders
    pub muted: Color32,
    /// Disabled labels and decorative icons only
    pub faint: Color32,
    /// Focus, progress, and the level meter
    pub accent: Color32,
    pub on_accent: Color32,
    /// The fill of a primary button and of a switch that is on
    pub primary: Color32,
    pub on_primary: Color32,
    pub good: Color32,
    pub warn: Color32,
    pub bad: Color32,
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

pub const LIGHT: Palette = Palette {
    bg: hex(0xF7F8FA),
    surface: hex(0xFFFFFF),
    raised: hex(0xFFFFFF),
    sunken: hex(0xF0F2F5),
    selection: hex(0xEEF2F8),
    hover: hex(0xF3F5F8),
    line: hex(0xE3E7ED),
    control: hex(0x8A93A3),
    ink: hex(0x141821),
    muted: hex(0x5B6475),
    faint: hex(0x8A93A3),
    accent: hex(0x5B4BDB),
    on_accent: hex(0xFFFFFF),
    primary: hex(0x141821),
    on_primary: hex(0xFFFFFF),
    good: hex(0x1F7A3D),
    warn: hex(0x8A5A00),
    bad: hex(0xC42B2B),
};

pub const DARK: Palette = Palette {
    bg: hex(0x0D1017),
    surface: hex(0x141821),
    raised: hex(0x1A1F2A),
    sunken: hex(0x0A0C12),
    selection: hex(0x1E2533),
    hover: hex(0x181D27),
    line: hex(0x252B37),
    control: hex(0x6B7486),
    ink: hex(0xECEFF4),
    muted: hex(0x9AA3B2),
    faint: hex(0x6B7486),
    accent: hex(0x8B7CF0),
    on_accent: hex(0x141821),
    primary: hex(0xECEFF4),
    on_primary: hex(0x141821),
    good: hex(0x4CC27A),
    warn: hex(0xE8A84A),
    bad: hex(0xF4736B),
};

/// The two ends of the gradient on the Hush mark's tile.
pub const MARK_FROM: Color32 = hex(0x5B4BDB);
pub const MARK_TO: Color32 = hex(0x8B3EE0);

pub fn palette(ctx: &Context) -> &'static Palette {
    match ctx.theme() {
        Theme::Dark => &DARK,
        Theme::Light => &LIGHT,
    }
}

/// The scheme to draw: the setting, else the desktop's preference, else
/// light.
pub fn resolve(appearance: Appearance, desktop: Option<Theme>) -> Theme {
    match appearance {
        Appearance::Light => Theme::Light,
        Appearance::Dark => Theme::Dark,
        Appearance::System => desktop.unwrap_or(Theme::Light),
    }
}

/// With `HUSH_SETTINGS_SCREENSHOT`, `HUSH_SETTINGS_APPEARANCE=dark`
/// overrides the saved appearance.
pub fn appearance_from_env() -> Option<Appearance> {
    appearance_named(&std::env::var("HUSH_SETTINGS_APPEARANCE").ok()?)
}

fn appearance_named(name: &str) -> Option<Appearance> {
    Appearance::ALL
        .into_iter()
        .find(|appearance| appearance.name().eq_ignore_ascii_case(name.trim()))
}

pub const BODY: f32 = 13.5;
pub const SMALL: f32 = 12.0;
pub const LABEL: f32 = 11.0;
pub const TITLE: f32 = 15.0;
pub const MONO: f32 = 12.5;
pub const ICON: f32 = 16.0;
/// Line heights of the body and small styles.
pub const BODY_LINE: f32 = 20.0;
pub const SMALL_LINE: f32 = 16.0;

pub const SIDEBAR_WIDTH: f32 = 232.0;
/// The title bar, and the sidebar's header beside it
pub const BAR_HEIGHT: f32 = 48.0;
pub const FOOTER_HEIGHT: f32 = 48.0;
/// The widest the column of settings gets, padding included
pub const COLUMN_MAX: f32 = 672.0;
pub const PAGE_PADDING: f32 = 24.0;
pub const SECTION_GAP: f32 = 32.0;
pub const ROW_PADDING: f32 = 16.0;
/// Between the text of a row and its control
pub const ROW_GAP: f32 = 24.0;
/// A row's text never gets narrower than this; its control wraps below
pub const MIN_TEXT: f32 = 240.0;
pub const CONTROL_HEIGHT: f32 = 30.0;
/// An icon button, a stepper's buttons, and a segment
pub const SMALL_CONTROL: f32 = 24.0;
pub const FIELD_WIDTH: f32 = 200.0;
/// The band at each window edge that resizes the window, which the scroll
/// bar keeps clear of
pub const EDGE_CLEARANCE: f32 = 4.0;
pub const RADIUS_XS: f32 = 4.0;
pub const RADIUS_SM: f32 = 6.0;
pub const RADIUS_MD: f32 = 8.0;

pub const MEDIUM: &str = "geist-medium";
pub const SEMIBOLD: &str = "geist-semibold";
pub const MONO_MEDIUM: &str = "geist-mono-medium";
pub const ICONS: &str = "lucide";

pub fn sans(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MEDIUM.into()))
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}

pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

pub fn mono_medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MONO_MEDIUM.into()))
}

pub fn icon_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(ICONS.into()))
}

/// Geist for text, Geist Mono for values, and Lucide for icons, with egui's
/// own fonts behind them for scripts and emoji that Geist lacks.
pub fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let embedded: [(&str, &'static [u8]); 6] = [
        ("geist", include_bytes!("fonts/Geist-Regular.ttf")),
        (MEDIUM, include_bytes!("fonts/Geist-Medium.ttf")),
        (SEMIBOLD, include_bytes!("fonts/Geist-SemiBold.ttf")),
        ("geist-mono", include_bytes!("fonts/GeistMono-Regular.ttf")),
        (MONO_MEDIUM, include_bytes!("fonts/GeistMono-Medium.ttf")),
        (ICONS, include_bytes!("fonts/lucide.ttf")),
    ];
    for (name, bytes) in embedded {
        fonts
            .font_data
            .insert(name.to_string(), FontData::from_static(bytes));
    }
    let fallbacks = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let with_fallbacks = |first: &str| {
        std::iter::once(first.to_string())
            .chain(fallbacks.iter().cloned())
            .collect::<Vec<_>>()
    };
    let families = [
        (FontFamily::Proportional, with_fallbacks("geist")),
        (FontFamily::Name(MEDIUM.into()), with_fallbacks(MEDIUM)),
        (FontFamily::Name(SEMIBOLD.into()), with_fallbacks(SEMIBOLD)),
        (FontFamily::Monospace, with_fallbacks("geist-mono")),
        (
            FontFamily::Name(MONO_MEDIUM.into()),
            with_fallbacks(MONO_MEDIUM),
        ),
        (FontFamily::Name(ICONS.into()), vec![ICONS.to_string()]),
    ];
    for (family, names) in families {
        fonts.families.insert(family, names);
    }
    fonts
}

/// Install the fonts and both schemes' styles.
pub fn install(ctx: &Context) {
    ctx.set_fonts(fonts());
    ctx.set_style_of(Theme::Light, style(&LIGHT, false));
    ctx.set_style_of(Theme::Dark, style(&DARK, true));
}

fn style(palette: &Palette, dark: bool) -> egui::Style {
    let mut style = egui::Style {
        text_styles: [
            (TextStyle::Small, sans(SMALL)),
            (TextStyle::Body, sans(BODY)),
            (TextStyle::Button, medium(BODY)),
            (TextStyle::Heading, semibold(TITLE)),
            (TextStyle::Monospace, mono(MONO)),
        ]
        .into(),
        ..Default::default()
    };

    style.interaction.selectable_labels = false;
    let spacing = &mut style.spacing;
    spacing.item_spacing = egui::vec2(8.0, 8.0);
    spacing.button_padding = egui::vec2(12.0, 0.0);
    spacing.interact_size = egui::vec2(SMALL_CONTROL, CONTROL_HEIGHT);
    spacing.combo_width = FIELD_WIDTH;
    spacing.icon_width = 16.0;
    spacing.icon_width_inner = 10.0;
    spacing.icon_spacing = 8.0;
    spacing.menu_margin = egui::Margin::same(4.0);
    spacing.indent = 0.0;
    spacing.tooltip_width = 320.0;
    spacing.scroll = egui::style::ScrollStyle {
        bar_width: 8.0,
        floating_width: 4.0,
        bar_inner_margin: 2.0,
        bar_outer_margin: EDGE_CLEARANCE,
        ..egui::style::ScrollStyle::floating()
    };

    let visuals = &mut style.visuals;
    *visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = palette.surface;
    visuals.window_fill = palette.raised;
    visuals.window_stroke = Stroke::new(1.0_f32, palette.line);
    visuals.extreme_bg_color = palette.surface;
    visuals.faint_bg_color = palette.hover;
    visuals.code_bg_color = palette.sunken;
    visuals.hyperlink_color = palette.accent;
    visuals.warn_fg_color = palette.warn;
    visuals.error_fg_color = palette.bad;
    visuals.selection.bg_fill = palette.accent.gamma_multiply(0.3);
    visuals.selection.stroke = Stroke::new(2.0_f32, palette.accent);
    visuals.text_cursor.stroke = Stroke::new(1.5_f32, palette.ink);
    visuals.window_rounding = Rounding::same(RADIUS_MD);
    visuals.menu_rounding = Rounding::same(RADIUS_MD);
    visuals.window_highlight_topmost = false;
    visuals.indent_has_left_vline = false;
    visuals.striped = false;
    let shadow = if dark {
        Color32::from_black_alpha(128)
    } else {
        Color32::from_rgba_unmultiplied(20, 24, 33, 31)
    };
    visuals.popup_shadow = Shadow {
        offset: egui::vec2(0.0, 8.0),
        blur: 24.0,
        spread: 0.0,
        color: shadow,
    };
    visuals.window_shadow = visuals.popup_shadow;

    let rounding = Rounding::same(RADIUS_SM);
    let state = |fill: Color32, edge: Color32| egui::style::WidgetVisuals {
        bg_fill: fill,
        weak_bg_fill: fill,
        bg_stroke: Stroke::new(1.0_f32, edge),
        rounding,
        fg_stroke: Stroke::new(1.0_f32, palette.ink),
        expansion: 0.0,
    };
    let widgets = &mut visuals.widgets;
    widgets.noninteractive = state(palette.surface, palette.line);
    widgets.inactive = state(palette.surface, palette.control);
    widgets.hovered = state(palette.hover, palette.control);
    widgets.active = state(palette.selection, palette.control);
    widgets.open = state(palette.hover, palette.control);
    style
}

/// The left edge and the width of the settings column inside a page `width`
/// wide: at most [`COLUMN_MAX`] with its padding, centred.
pub fn column(width: f32) -> (f32, f32) {
    let outer = width.clamp(0.0, COLUMN_MAX);
    let inner = (outer - 2.0 * PAGE_PADDING).max(0.0);
    let left = ((width - outer) / 2.0).max(0.0) + (outer - inner) / 2.0;
    (left, inner)
}

/// Where a row's control goes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Split {
    /// Beside the text, which gets `text` points
    Beside { text: f32 },
    /// Under the text
    Stacked,
}

/// The control sits beside the text while the text keeps [`MIN_TEXT`].
pub fn split(width: f32, control: f32) -> Split {
    let text = width - control - ROW_GAP;
    if text >= MIN_TEXT {
        Split::Beside { text }
    } else {
        Split::Stacked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(colour: Color32) -> f64 {
        let channel = |c: u8| {
            let c = f64::from(c) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(colour.r()) + 0.7152 * channel(colour.g()) + 0.0722 * channel(colour.b())
    }

    fn contrast(a: Color32, b: Color32) -> f64 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn the_setting_wins_and_system_follows_the_desktop() {
        for desktop in [None, Some(Theme::Light), Some(Theme::Dark)] {
            assert_eq!(resolve(Appearance::Light, desktop), Theme::Light);
            assert_eq!(resolve(Appearance::Dark, desktop), Theme::Dark);
        }
        assert_eq!(resolve(Appearance::System, Some(Theme::Dark)), Theme::Dark);
        assert_eq!(
            resolve(Appearance::System, Some(Theme::Light)),
            Theme::Light
        );
        assert_eq!(
            resolve(Appearance::System, None),
            Theme::Light,
            "no preference is light"
        );
    }

    #[test]
    fn text_meets_wcag_aa_on_every_ground() {
        for palette in [&LIGHT, &DARK] {
            let grounds = [
                palette.bg,
                palette.surface,
                palette.raised,
                palette.selection,
                palette.hover,
            ];
            for ground in grounds {
                for text in [
                    palette.ink,
                    palette.muted,
                    palette.good,
                    palette.warn,
                    palette.bad,
                ] {
                    assert!(
                        contrast(text, ground) >= 4.5,
                        "{text:?} on {ground:?}: {:.2}",
                        contrast(text, ground)
                    );
                }
                assert!(
                    contrast(palette.accent, ground) >= 3.0,
                    "{:?} on {ground:?}",
                    palette.accent
                );
            }
            for ground in [palette.surface, palette.raised] {
                assert!(
                    contrast(palette.control, ground) >= 3.0,
                    "{:?} on {ground:?}",
                    palette.control
                );
            }
            assert!(contrast(palette.on_primary, palette.primary) >= 4.5);
            assert!(contrast(palette.on_accent, palette.accent) >= 4.5);
            assert!(contrast(palette.on_primary, palette.bad) >= 4.5);
        }
    }

    #[test]
    fn the_column_is_centred_and_capped() {
        assert_eq!(column(1000.0), (164.0 + PAGE_PADDING, COLUMN_MAX - 48.0));
        assert_eq!(column(COLUMN_MAX), (PAGE_PADDING, COLUMN_MAX - 48.0));
        assert_eq!(column(500.0), (PAGE_PADDING, 452.0));
        assert_eq!(column(30.0), (15.0, 0.0), "never negative");
    }

    #[test]
    fn a_control_wraps_under_text_that_would_get_too_narrow() {
        assert_eq!(split(624.0, 200.0), Split::Beside { text: 400.0 });
        assert_eq!(
            split(MIN_TEXT + ROW_GAP + 100.0, 100.0),
            Split::Beside { text: MIN_TEXT }
        );
        assert_eq!(split(MIN_TEXT + ROW_GAP + 99.0, 100.0), Split::Stacked);
        assert_eq!(split(300.0, 420.0), Split::Stacked);
    }

    #[test]
    fn the_screenshot_override_reads_an_appearance_in_any_case() {
        assert_eq!(appearance_named(" dark "), Some(Appearance::Dark));
        assert_eq!(appearance_named("LIGHT"), Some(Appearance::Light));
        assert_eq!(appearance_named("system"), Some(Appearance::System));
        assert_eq!(appearance_named("blue"), None);
    }
}
