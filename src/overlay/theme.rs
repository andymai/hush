//! The pill's colours: a preset, the desktop's appearance, or a theme file
//! that the overlay reads again after each change.

use super::render::{Look, Palette};
use crate::config::settings::OverlayTheme;
use crate::portal::appearance::{Appearance, Scheme};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};
use tiny_skia::Color;
use tracing::{debug, info, warn};

/// How often the overlay looks at the theme file while the pill shows.
const RECHECK: Duration = Duration::from_secs(1);
/// Below these, text on the pill is hard to read over a busy window.
const MIN_BACKGROUND_ALPHA: u8 = 128;
const MIN_FOREGROUND_ALPHA: u8 = 160;

/// `overlay-theme.toml`, next to the config file in use.
pub fn file_path() -> PathBuf {
    crate::config::paths::config_path().with_file_name("overlay-theme.toml")
}

/// A preset's colours. `System` and `File` have none of their own, so they
/// give the dark preset.
pub fn preset(theme: OverlayTheme) -> Palette {
    match theme {
        OverlayTheme::System | OverlayTheme::File | OverlayTheme::Dark => Palette {
            background: rgba(14, 14, 17, 222),
            border: rgba(255, 255, 255, 36),
            foreground: rgba(242, 240, 234, 235),
            level: rgba(242, 240, 234, 235),
            recording: rgba(229, 72, 77, 255),
            warning: rgba(245, 165, 36, 255),
            error: rgba(204, 41, 54, 255),
        },
        OverlayTheme::Light => Palette {
            background: rgba(250, 250, 249, 230),
            border: rgba(0, 0, 0, 28),
            foreground: rgba(24, 24, 27, 235),
            level: rgba(24, 24, 27, 235),
            recording: rgba(220, 38, 38, 255),
            warning: rgba(180, 83, 9, 255),
            error: rgba(185, 28, 28, 255),
        },
        OverlayTheme::Nord => Palette {
            background: rgba(46, 52, 64, 235),
            border: rgba(216, 222, 233, 40),
            foreground: rgba(236, 239, 244, 240),
            level: rgba(136, 192, 208, 255),
            recording: rgba(191, 97, 106, 255),
            warning: rgba(235, 203, 139, 255),
            error: rgba(191, 97, 106, 255),
        },
        OverlayTheme::Catppuccin => Palette {
            background: rgba(30, 30, 46, 235),
            border: rgba(205, 214, 244, 36),
            foreground: rgba(205, 214, 244, 240),
            level: rgba(203, 166, 247, 255),
            recording: rgba(243, 139, 168, 255),
            warning: rgba(249, 226, 175, 255),
            error: rgba(243, 139, 168, 255),
        },
        OverlayTheme::Dracula => Palette {
            background: rgba(40, 42, 54, 235),
            border: rgba(248, 248, 242, 36),
            foreground: rgba(248, 248, 242, 240),
            level: rgba(189, 147, 249, 255),
            recording: rgba(255, 85, 85, 255),
            warning: rgba(241, 250, 140, 255),
            error: rgba(255, 85, 85, 255),
        },
        OverlayTheme::Gruvbox => Palette {
            background: rgba(40, 40, 40, 235),
            border: rgba(235, 219, 178, 36),
            foreground: rgba(235, 219, 178, 240),
            level: rgba(142, 192, 124, 255),
            recording: rgba(251, 73, 52, 255),
            warning: rgba(250, 189, 47, 255),
            error: rgba(204, 36, 29, 255),
        },
        OverlayTheme::HighContrast => Palette {
            background: rgba(0, 0, 0, 255),
            border: rgba(255, 255, 255, 255),
            foreground: rgba(255, 255, 255, 255),
            level: rgba(255, 255, 255, 255),
            recording: rgba(255, 80, 80, 255),
            warning: rgba(255, 221, 0, 255),
            error: rgba(255, 80, 80, 255),
        },
    }
}

/// The desktop's look: high contrast when it asks for that, else light or
/// dark, with the bars in its accent colour.
fn system(appearance: &Appearance) -> Palette {
    if appearance.high_contrast {
        return preset(OverlayTheme::HighContrast);
    }
    let mut palette = match appearance.scheme {
        Scheme::Light => preset(OverlayTheme::Light),
        Scheme::Dark | Scheme::NoPreference => preset(OverlayTheme::Dark),
    };
    if let Some([r, g, b]) = appearance.accent {
        palette.level = rgba(r, g, b, 255);
    }
    palette
}

fn resolve(theme: OverlayTheme, appearance: &Appearance) -> Palette {
    match theme {
        OverlayTheme::System | OverlayTheme::File => system(appearance),
        preset_theme => preset(preset_theme),
    }
}

/// The theme setting, and the theme file when the setting names it.
pub struct Theme {
    choice: OverlayTheme,
    file: Option<ThemeFile>,
}

impl Theme {
    pub fn new(choice: OverlayTheme, path: PathBuf) -> Self {
        let file = (choice == OverlayTheme::File).then(|| ThemeFile::new(path));
        Self { choice, file }
    }

    pub fn look(&mut self, appearance: &Appearance, now: Instant) -> Look {
        let palette = match self.file.as_mut() {
            Some(file) => {
                file.refresh(now);
                file.theme.palette(appearance)
            },
            None => resolve(self.choice, appearance),
        };
        Look {
            palette,
            reduced_motion: appearance.reduced_motion,
        }
    }
}

struct ThemeFile {
    path: PathBuf,
    /// The file's time at the last read; `None` when it was absent
    modified: Option<SystemTime>,
    next_check: Option<Instant>,
    /// The last colours that parsed
    theme: FileTheme,
}

impl ThemeFile {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            modified: None,
            next_check: None,
            theme: FileTheme::default(),
        }
    }

    fn refresh(&mut self, now: Instant) {
        let first = self.next_check.is_none();
        if self.next_check.is_some_and(|next| now < next) {
            return;
        }
        self.next_check = Some(now + RECHECK);
        let modified = std::fs::metadata(&self.path)
            .and_then(|meta| meta.modified())
            .ok();
        if !first && modified == self.modified {
            return;
        }
        self.modified = modified;
        self.theme = read(&self.path, &self.theme);
    }
}

/// The file's colours; the last good ones while the file has an error.
fn read(path: &Path, last: &FileTheme) -> FileTheme {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => {
            debug!("No overlay theme file at {}: {}", path.display(), e);
            return FileTheme::default();
        },
    };
    match parse(&text) {
        Ok(theme) => {
            info!("The overlay reads its colours from {}", path.display());
            theme
        },
        Err(e) => {
            warn!(
                "The overlay theme file {} has an error, so the pill keeps its colours: {}",
                path.display(),
                e
            );
            *last
        },
    }
}

/// The text of `overlay-theme.toml`.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileText {
    base: Option<OverlayTheme>,
    background: Option<String>,
    border: Option<String>,
    foreground: Option<String>,
    level: Option<String>,
    recording: Option<String>,
    warning: Option<String>,
    error: Option<String>,
}

/// A base theme, and the colours that the file changes.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct FileTheme {
    base: OverlayTheme,
    background: Option<Color>,
    border: Option<Color>,
    foreground: Option<Color>,
    level: Option<Color>,
    recording: Option<Color>,
    warning: Option<Color>,
    error: Option<Color>,
}

impl FileTheme {
    fn palette(&self, appearance: &Appearance) -> Palette {
        let base = resolve(self.base, appearance);
        let mut palette = Palette {
            background: self.background.unwrap_or(base.background),
            border: self.border.unwrap_or(base.border),
            foreground: self.foreground.unwrap_or(base.foreground),
            level: self.level.unwrap_or(base.level),
            recording: self.recording.unwrap_or(base.recording),
            warning: self.warning.unwrap_or(base.warning),
            error: self.error.unwrap_or(base.error),
        };
        clamp_alpha(&mut palette.background, MIN_BACKGROUND_ALPHA);
        clamp_alpha(&mut palette.foreground, MIN_FOREGROUND_ALPHA);
        palette
    }
}

fn parse(text: &str) -> Result<FileTheme, String> {
    let file: FileText = toml::from_str(text).map_err(|e| e.message().to_string())?;
    let mut bad = Vec::new();
    let mut colour_of = |name: &'static str, value: &Option<String>| {
        let text = value.as_deref()?;
        let colour = colour(text);
        if colour.is_none() {
            bad.push(name);
        }
        colour
    };
    let theme = FileTheme {
        base: file.base.unwrap_or_default(),
        background: colour_of("background", &file.background),
        border: colour_of("border", &file.border),
        foreground: colour_of("foreground", &file.foreground),
        level: colour_of("level", &file.level),
        recording: colour_of("recording", &file.recording),
        warning: colour_of("warning", &file.warning),
        error: colour_of("error", &file.error),
    };
    if bad.is_empty() {
        Ok(theme)
    } else {
        Err(format!(
            "{} must be \"#rrggbb\" or \"#rrggbbaa\"",
            bad.join(", ")
        ))
    }
}

/// `#rrggbb`, or `#rrggbbaa` with an alpha.
fn colour(text: &str) -> Option<Color> {
    let hex = text.trim().strip_prefix('#')?;
    if !matches!(hex.len(), 6 | 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).ok();
    let alpha = if hex.len() == 8 { byte(6)? } else { 255 };
    Some(rgba(byte(0)?, byte(2)?, byte(4)?, alpha))
}

fn clamp_alpha(colour: &mut Color, minimum: u8) {
    let floor = f32::from(minimum) / 255.0;
    if colour.alpha() < floor {
        colour.set_alpha(floor);
    }
}

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::from_rgba8(r, g, b, a)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dark_desktop() -> Appearance {
        Appearance {
            scheme: Scheme::Dark,
            ..Appearance::default()
        }
    }

    #[test]
    fn the_system_theme_follows_the_desktop() {
        let light = Appearance {
            scheme: Scheme::Light,
            ..Appearance::default()
        };
        assert_eq!(system(&light), preset(OverlayTheme::Light));
        assert_eq!(system(&dark_desktop()), preset(OverlayTheme::Dark));
        assert_eq!(system(&Appearance::default()), preset(OverlayTheme::Dark));

        let accent = Appearance {
            accent: Some([61, 174, 233]),
            ..dark_desktop()
        };
        assert_eq!(system(&accent).level, rgba(61, 174, 233, 255));
        assert_eq!(
            system(&accent).foreground,
            preset(OverlayTheme::Dark).foreground
        );

        let contrast = Appearance {
            high_contrast: true,
            accent: Some([61, 174, 233]),
            ..light
        };
        assert_eq!(system(&contrast), preset(OverlayTheme::HighContrast));
    }

    #[test]
    fn a_preset_ignores_the_desktop_but_motion_follows_it() {
        let still = Appearance {
            high_contrast: true,
            reduced_motion: true,
            ..dark_desktop()
        };
        let mut theme = Theme::new(OverlayTheme::Nord, PathBuf::from("/nonexistent"));
        let look = theme.look(&still, Instant::now());
        assert_eq!(look.palette, preset(OverlayTheme::Nord));
        assert!(look.reduced_motion);
    }

    #[test]
    fn colours_are_six_or_eight_hex_digits() {
        assert_eq!(colour("#88c0d0"), Some(rgba(0x88, 0xc0, 0xd0, 255)));
        assert_eq!(colour(" #88C0D0 "), Some(rgba(0x88, 0xc0, 0xd0, 255)));
        assert_eq!(colour("#2e3440e6"), Some(rgba(0x2e, 0x34, 0x40, 0xe6)));
        assert_eq!(colour("88c0d0"), None);
        assert_eq!(colour("#88c0d"), None);
        assert_eq!(colour("#88c0dz"), None);
        assert_eq!(colour("#ééé"), None);
    }

    #[test]
    fn the_file_changes_its_base() {
        let theme = parse("base = \"nord\"\nlevel = \"#ff0000\"\n").unwrap();
        let palette = theme.palette(&dark_desktop());
        assert_eq!(palette.level, rgba(255, 0, 0, 255));
        assert_eq!(palette.background, preset(OverlayTheme::Nord).background);

        let without_base = parse("recording = \"#00ff00\"").unwrap();
        let light = Appearance {
            scheme: Scheme::Light,
            ..Appearance::default()
        };
        assert_eq!(
            without_base.palette(&light).background,
            preset(OverlayTheme::Light).background,
            "without a base, the file starts from the system theme"
        );
    }

    #[test]
    fn the_file_keeps_text_readable() {
        let theme = parse("background = \"#00000010\"\nforeground = \"#ffffff00\"").unwrap();
        let palette = theme.palette(&dark_desktop());
        assert_eq!(palette.background.alpha(), 128.0 / 255.0);
        assert_eq!(palette.foreground.alpha(), 160.0 / 255.0);
        let border = parse("border = \"#ffffff00\"")
            .unwrap()
            .palette(&dark_desktop());
        assert_eq!(
            border.border.alpha(),
            0.0,
            "only text and its ground have a floor"
        );
    }

    #[test]
    fn a_file_with_an_error_names_the_problem() {
        assert_eq!(
            parse("level = \"blue\"\nerror = \"#12\"").unwrap_err(),
            "level, error must be \"#rrggbb\" or \"#rrggbbaa\""
        );
        assert!(parse("levels = \"#ffffff\"").is_err(), "a misspelt key");
        assert!(parse("base = \"solarized\"").is_err());
        assert!(parse("level = ").is_err());
    }

    #[test]
    fn the_file_reloads_after_a_change_and_keeps_the_last_good_colours() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overlay-theme.toml");
        let start = Instant::now();
        let mut theme = Theme::new(OverlayTheme::File, path.clone());
        let desktop = dark_desktop();
        assert_eq!(
            theme.look(&desktop, start).palette,
            preset(OverlayTheme::Dark),
            "without the file, the system theme"
        );

        std::fs::write(&path, "level = \"#ff0000\"").unwrap();
        assert_eq!(
            theme
                .look(&desktop, start + Duration::from_millis(10))
                .palette
                .level,
            preset(OverlayTheme::Dark).level,
            "not before the next check"
        );
        let later = start + RECHECK;
        assert_eq!(
            theme.look(&desktop, later).palette.level,
            rgba(255, 0, 0, 255)
        );

        std::fs::write(&path, "level = \"#ff00\"").unwrap();
        bump(&path);
        let later = later + RECHECK;
        assert_eq!(
            theme.look(&desktop, later).palette.level,
            rgba(255, 0, 0, 255),
            "an edit with an error keeps the last good colours"
        );

        std::fs::remove_file(&path).unwrap();
        let later = later + RECHECK;
        assert_eq!(
            theme.look(&desktop, later).palette,
            preset(OverlayTheme::Dark)
        );
    }

    /// A write in the same instant can keep the file's time.
    fn bump(path: &Path) {
        let file = std::fs::File::options().write(true).open(path).unwrap();
        file.set_modified(SystemTime::now() + Duration::from_secs(5))
            .unwrap();
    }
}
