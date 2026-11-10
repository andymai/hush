//! `HUSH_SETTINGS_SCREENSHOT=page.png hush settings` draws the window, saves
//! one frame as a PNG, and exits, for the documentation and for review. With
//! `HUSH_SETTINGS_PAGE` and `HUSH_SETTINGS_APPEARANCE` it picks the page and
//! the scheme, and with `HUSH_SETTINGS_SIZE=1000x2400` the window size, to
//! show a whole page. `HUSH_SETTINGS_PALETTE=model` opens the command
//! palette with that query; an empty value opens it with none.

use anyhow::{Context as _, Result};
use std::path::PathBuf;
use tracing::{error, info};

/// Frames drawn before the capture, so fonts, layout, and the first machine
/// read have settled.
const SETTLE_FRAMES: u32 = 6;

pub struct Screenshot {
    path: PathBuf,
    frames: u32,
    requested: bool,
}

impl Screenshot {
    pub fn from_env() -> Option<Self> {
        let path = std::env::var_os("HUSH_SETTINGS_SCREENSHOT")?;
        Some(Self {
            path: PathBuf::from(path),
            frames: 0,
            requested: false,
        })
    }

    /// The query of the command palette that `HUSH_SETTINGS_PALETTE` asks
    /// for, when it asks for the palette.
    pub fn palette_query() -> Option<String> {
        std::env::var("HUSH_SETTINGS_PALETTE").ok()
    }

    /// The window size that `HUSH_SETTINGS_SIZE` asks for.
    pub fn size(&self) -> Option<[f32; 2]> {
        parse_size(&std::env::var("HUSH_SETTINGS_SIZE").ok()?)
    }

    /// Ask for the frame once the window settles, then save it and close.
    pub fn step(&mut self, ctx: &egui::Context) {
        let image = ctx.input(|input| {
            input.raw.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            match save(&self.path, &image) {
                Ok(()) => info!("Saved {}", self.path.display()),
                Err(e) => error!("{:#}", e),
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        self.frames += 1;
        if self.frames >= SETTLE_FRAMES && !self.requested {
            self.requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
        }
        ctx.request_repaint();
    }
}

/// `WIDTHxHEIGHT` in points.
fn parse_size(text: &str) -> Option<[f32; 2]> {
    let (width, height) = text.trim().split_once('x')?;
    let size = [width.trim().parse().ok()?, height.trim().parse().ok()?];
    size.iter().all(|side: &f32| *side >= 100.0).then_some(size)
}

fn save(path: &std::path::Path, image: &egui::ColorImage) -> Result<()> {
    let [width, height] = image.size;
    image::save_buffer(
        path,
        image.as_raw(),
        u32::try_from(width).context("The frame is too wide")?,
        u32::try_from(height).context("The frame is too tall")?,
        image::ColorType::Rgba8,
    )
    .with_context(|| format!("Could not save {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_size_is_width_by_height_in_points() {
        assert_eq!(parse_size("1000x2400"), Some([1000.0, 2400.0]));
        assert_eq!(parse_size(" 800 x 600 "), Some([800.0, 600.0]));
        assert_eq!(parse_size("1000"), None);
        assert_eq!(parse_size("1000x20"), None, "too small to draw the window");
        assert_eq!(parse_size("widexhigh"), None);
    }
}
