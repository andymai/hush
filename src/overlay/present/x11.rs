//! The pill in an X11 override-redirect window: on GNOME and other
//! compositors without layer-shell (through Xwayland), on X11 sessions, and
//! in sandboxes whose compositor withholds layer-shell. The window takes no
//! input, and it is unmapped while the pill is hidden.

use super::{poll, Failure, Presenter};
use crate::overlay::render::{to_bgra, BOTTOM_MARGIN, SURFACE_HEIGHT, SURFACE_WIDTH};
use anyhow::Context;
use std::os::fd::{AsFd, BorrowedFd};
use std::time::Duration;
use tiny_skia::{Pixmap, PremultipliedColorU8};
use x11rb::connection::Connection;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::shape::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{
    AtomEnum, ClipOrdering, ColormapAlloc, ConfigureWindowAux, ConnectionExt as _, CreateGCAux,
    CreateWindowAux, EventMask, ImageFormat, PropMode, Rectangle, StackMode, VisualClass,
    WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::NONE;

pub struct X11Presenter {
    conn: RustConnection,
    root: u32,
    window: u32,
    gc: u32,
    depth: u8,
    scale: f32,
    width: u16,
    height: u16,
    screen: (u16, u16),
    /// Without an alpha channel that a compositing manager blends, the
    /// window's shape follows the pill instead.
    shaped: bool,
    mapped: bool,
}

impl X11Presenter {
    pub fn connect() -> anyhow::Result<Self> {
        let (conn, screen_number) = x11rb::connect(None).context("No X11 display")?;
        let screen = conn.setup().roots[screen_number].clone();
        let root = screen.root;
        let scale = xft_scale(&conn, root).unwrap_or(1.0);
        let width = (SURFACE_WIDTH as f32 * scale).round() as u16;
        let height = (SURFACE_HEIGHT as f32 * scale).round() as u16;
        let argb = screen
            .allowed_depths
            .iter()
            .filter(|depth| depth.depth == 32)
            .flat_map(|depth| depth.visuals.iter())
            .find(|visual| visual.class == VisualClass::TRUE_COLOR)
            .map(|visual| visual.visual_id);
        let blended = composited(&conn, screen_number);
        let (depth, visual, shaped) = match argb {
            Some(visual) => (32, visual, !blended),
            None => (screen.root_depth, screen.root_visual, true),
        };
        let colormap = conn.generate_id()?;
        conn.create_colormap(ColormapAlloc::NONE, colormap, root, visual)?;
        let window = conn.generate_id()?;
        conn.create_window(
            depth,
            window,
            root,
            0,
            0,
            width,
            height,
            0,
            WindowClass::INPUT_OUTPUT,
            visual,
            &CreateWindowAux::new()
                .override_redirect(1)
                .border_pixel(0)
                .background_pixel(0)
                .colormap(colormap)
                .event_mask(EventMask::EXPOSURE),
        )?;
        // An empty input shape lets every click through to the window below.
        conn.shape_rectangles(
            shape::SO::SET,
            shape::SK::INPUT,
            ClipOrdering::UNSORTED,
            window,
            0,
            0,
            &[],
        )?;
        conn.change_property8(
            PropMode::REPLACE,
            window,
            AtomEnum::WM_CLASS,
            AtomEnum::STRING,
            b"hush-overlay\0Hush\0",
        )?;
        let window_type = atom(&conn, b"_NET_WM_WINDOW_TYPE")?;
        let notification = atom(&conn, b"_NET_WM_WINDOW_TYPE_NOTIFICATION")?;
        conn.change_property32(
            PropMode::REPLACE,
            window,
            window_type,
            AtomEnum::ATOM,
            &[notification],
        )?;
        let gc = conn.generate_id()?;
        conn.create_gc(gc, window, &CreateGCAux::new())?;
        conn.flush()?;
        Ok(Self {
            conn,
            root,
            window,
            gc,
            depth,
            scale,
            width,
            height,
            screen: (screen.width_in_pixels, screen.height_in_pixels),
            shaped,
            mapped: false,
        })
    }

    /// The monitor of the focused window, else the primary one, else the
    /// whole screen. On GNOME under Wayland, Xwayland sees only X11 windows,
    /// so the pill then sits on the primary monitor.
    fn monitor(&self) -> (i32, i32, i32, i32) {
        let whole = (0, 0, self.screen.0 as i32, self.screen.1 as i32);
        let Ok(monitors) = self
            .conn
            .randr_get_monitors(self.root, true)
            .map_err(anyhow::Error::from)
            .and_then(|cookie| cookie.reply().map_err(anyhow::Error::from))
            .map(|reply| reply.monitors)
        else {
            return whole;
        };
        let rect = |monitor: &x11rb::protocol::randr::MonitorInfo| {
            (
                monitor.x as i32,
                monitor.y as i32,
                monitor.width as i32,
                monitor.height as i32,
            )
        };
        if let Some((cx, cy)) = self.focused_center() {
            if let Some(found) = monitors
                .iter()
                .map(rect)
                .find(|(x, y, w, h)| (*x..x + w).contains(&cx) && (*y..y + h).contains(&cy))
            {
                return found;
            }
        }
        monitors
            .iter()
            .find(|monitor| monitor.primary)
            .or(monitors.first())
            .map_or(whole, rect)
    }

    fn focused_center(&self) -> Option<(i32, i32)> {
        let active = atom(&self.conn, b"_NET_ACTIVE_WINDOW").ok()?;
        let window = self
            .conn
            .get_property(false, self.root, active, AtomEnum::WINDOW, 0, 1)
            .ok()?
            .reply()
            .ok()?
            .value32()?
            .next()
            .filter(|window| *window != NONE)?;
        let geometry = self.conn.get_geometry(window).ok()?.reply().ok()?;
        let origin = self
            .conn
            .translate_coordinates(window, self.root, 0, 0)
            .ok()?
            .reply()
            .ok()?;
        Some((
            origin.dst_x as i32 + geometry.width as i32 / 2,
            origin.dst_y as i32 + geometry.height as i32 / 2,
        ))
    }

    fn draw(&mut self, pixmap: &Pixmap) -> anyhow::Result<()> {
        if pixmap.width() != self.width as u32 || pixmap.height() != self.height as u32 {
            return Ok(());
        }
        let (x, y, width, height) = self.monitor();
        let left = x + (width - self.width as i32) / 2;
        let top =
            y + height - self.height as i32 - (BOTTOM_MARGIN as f32 * self.scale).round() as i32;
        self.conn.configure_window(
            self.window,
            &ConfigureWindowAux::new()
                .x(left)
                .y(top)
                .stack_mode(StackMode::ABOVE),
        )?;
        if self.shaped {
            self.conn.shape_rectangles(
                shape::SO::SET,
                shape::SK::BOUNDING,
                ClipOrdering::UNSORTED,
                self.window,
                0,
                0,
                &opaque_rows(pixmap),
            )?;
        }
        if !self.mapped {
            self.conn.map_window(self.window)?;
            self.mapped = true;
        }
        self.conn.put_image(
            ImageFormat::Z_PIXMAP,
            self.window,
            self.gc,
            self.width,
            self.height,
            0,
            0,
            0,
            self.depth,
            &to_bgra(pixmap),
        )?;
        self.conn.flush()?;
        Ok(())
    }

    fn drain_events(&self) -> Result<(), Failure> {
        loop {
            match self.conn.poll_for_event() {
                Ok(Some(_)) => continue,
                Ok(None) => return Ok(()),
                Err(_) => return Err(Failure::Lost),
            }
        }
    }
}

impl Presenter for X11Presenter {
    fn name(&self) -> &'static str {
        "X11"
    }

    fn scale(&self) -> f32 {
        self.scale
    }

    fn present(&mut self, frame: Option<&Pixmap>) -> Result<(), Failure> {
        match frame {
            Some(pixmap) => self.draw(pixmap).map_err(lost_or_broken),
            None => {
                if self.mapped {
                    self.mapped = false;
                    self.conn
                        .unmap_window(self.window)
                        .map_err(|e| lost_or_broken(e.into()))?;
                    self.conn.flush().map_err(|e| lost_or_broken(e.into()))?;
                }
                Ok(())
            },
        }
    }

    fn wait(&mut self, link: BorrowedFd<'_>, timeout: Option<Duration>) -> Result<bool, Failure> {
        self.drain_events()?;
        let ready = poll(&[self.conn.stream().as_fd(), link], timeout)
            .map_err(|e| Failure::Broken(e.into()))?;
        self.drain_events()?;
        Ok(ready[1])
    }
}

/// A broken X11 connection means the display went away.
fn lost_or_broken(error: anyhow::Error) -> Failure {
    if error
        .downcast_ref::<x11rb::errors::ConnectionError>()
        .is_some()
    {
        Failure::Lost
    } else {
        Failure::Broken(error)
    }
}

fn atom(conn: &RustConnection, name: &[u8]) -> anyhow::Result<u32> {
    Ok(conn.intern_atom(false, name)?.reply()?.atom)
}

/// A compositing manager owns `_NET_WM_CM_S<screen>` and blends the alpha.
fn composited(conn: &RustConnection, screen: usize) -> bool {
    let name = format!("_NET_WM_CM_S{}", screen);
    atom(conn, name.as_bytes())
        .ok()
        .and_then(|selection| conn.get_selection_owner(selection).ok()?.reply().ok())
        .is_some_and(|reply| reply.owner != NONE)
}

/// `Xft.dpi` from the root window's resources, over 96.
fn xft_scale(conn: &RustConnection, root: u32) -> Option<f32> {
    let resources = conn
        .get_property(
            false,
            root,
            AtomEnum::RESOURCE_MANAGER,
            AtomEnum::STRING,
            0,
            u32::MAX / 4,
        )
        .ok()?
        .reply()
        .ok()?;
    scale_from_resources(&String::from_utf8_lossy(&resources.value))
}

fn scale_from_resources(resources: &str) -> Option<f32> {
    let dpi: f32 = resources
        .lines()
        .find_map(|line| line.strip_prefix("Xft.dpi:"))?
        .trim()
        .parse()
        .ok()?;
    Some((dpi / 96.0).clamp(1.0, 4.0))
}

/// The pixels with most of their alpha, as one rectangle for each run in
/// each row, for a window shape.
fn opaque_rows(pixmap: &Pixmap) -> Vec<Rectangle> {
    let width = pixmap.width() as usize;
    let mut rectangles = Vec::new();
    for (y, row) in pixmap.pixels().chunks(width).enumerate() {
        let mut start = None;
        for (x, pixel) in row
            .iter()
            .chain(std::iter::once(&PremultipliedColorU8::TRANSPARENT))
            .enumerate()
        {
            match (start, pixel.alpha() >= 128) {
                (None, true) => start = Some(x),
                (Some(from), false) => {
                    rectangles.push(Rectangle {
                        x: from as i16,
                        y: y as i16,
                        width: (x - from) as u16,
                        height: 1,
                    });
                    start = None;
                },
                _ => {},
            }
        }
    }
    rectangles
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::{Color, Rect, Transform};

    #[test]
    fn xft_dpi_gives_the_scale() {
        assert_eq!(
            scale_from_resources("Xft.antialias:\t1\nXft.dpi:\t192\n"),
            Some(2.0)
        );
        assert_eq!(scale_from_resources("Xft.dpi: 96"), Some(1.0));
        assert_eq!(scale_from_resources("Xft.dpi: 60"), Some(1.0));
        assert_eq!(scale_from_resources("Xcursor.size: 24"), None);
    }

    #[test]
    fn the_shape_covers_the_opaque_runs() {
        let mut pixmap = Pixmap::new(6, 2).unwrap();
        let mut paint = tiny_skia::Paint::default();
        paint.set_color(Color::WHITE);
        pixmap.fill_rect(
            Rect::from_xywh(1.0, 0.0, 3.0, 1.0).unwrap(),
            &paint,
            Transform::identity(),
            None,
        );
        let rows: Vec<(i16, i16, u16, u16)> = opaque_rows(&pixmap)
            .iter()
            .map(|rect| (rect.x, rect.y, rect.width, rect.height))
            .collect();
        assert_eq!(rows, [(1, 0, 3, 1)]);
    }
}
