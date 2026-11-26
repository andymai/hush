//! Where the pill appears: a Wayland layer-shell surface when the compositor
//! offers one, else an X11 window (GNOME, X11 sessions, and sandboxes whose
//! compositor withholds layer-shell).

mod layer_shell;
mod x11;

use std::os::fd::{AsRawFd, BorrowedFd};
use std::time::Duration;
use tiny_skia::Pixmap;
use tracing::{debug, info};

/// Why a presenter stopped.
#[derive(Debug)]
pub enum Failure {
    /// The display connection ended, as when the compositor restarts
    Lost,
    Broken(anyhow::Error),
}

impl From<anyhow::Error> for Failure {
    fn from(error: anyhow::Error) -> Self {
        Failure::Broken(error)
    }
}

/// What the pointer did on the pill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pointer {
    Enter,
    Leave,
    /// The left button
    Primary,
    /// The right button
    Secondary,
}

/// What woke the loop.
#[derive(Debug, Default)]
pub struct Wake {
    /// The daemon's socket has lines to read
    pub link: bool,
    pub pointer: Vec<Pointer>,
}

/// A rectangle in the surface, in logical pixels: left, top, width, height.
pub type Rect = (f32, f32, f32, f32);

pub trait Presenter {
    fn name(&self) -> &'static str;

    /// The factor between logical and buffer pixels.
    fn scale(&self) -> f32;

    /// Show `frame`, a surface drawn at [`Self::scale`], or hide the pill.
    fn present(&mut self, frame: Option<&Pixmap>) -> Result<(), Failure>;

    /// Take clicks inside `rect` only; `None` lets every click through.
    fn set_input(&mut self, rect: Option<Rect>) -> Result<(), Failure>;

    /// Handle display events until `link` is readable, the display changes
    /// something, or `timeout` passes.
    fn wait(&mut self, link: BorrowedFd<'_>, timeout: Option<Duration>) -> Result<Wake, Failure>;
}

/// Whole pixels for a rectangle at `scale`.
pub(crate) fn pixels(rect: Rect, scale: f32) -> (i32, i32, i32, i32) {
    let (x, y, width, height) = rect;
    (
        (x * scale).floor() as i32,
        (y * scale).floor() as i32,
        (width * scale).ceil() as i32,
        (height * scale).ceil() as i32,
    )
}

/// The first presenter that this session offers.
pub fn open() -> Option<Box<dyn Presenter>> {
    match layer_shell::LayerShellPresenter::connect() {
        Ok(Some(presenter)) => return Some(Box::new(presenter)),
        Ok(None) => info!("The compositor offers no layer-shell, so the overlay tries X11"),
        Err(e) => debug!("No Wayland display for the overlay: {:#}", e),
    }
    match x11::X11Presenter::connect() {
        Ok(presenter) => Some(Box::new(presenter)),
        Err(e) => {
            debug!("No X11 display for the overlay: {:#}", e);
            None
        },
    }
}

/// Which of `fds` are readable after up to `timeout`.
pub(crate) fn poll(
    fds: &[BorrowedFd<'_>],
    timeout: Option<Duration>,
) -> std::io::Result<Vec<bool>> {
    let mut polled: Vec<libc::pollfd> = fds
        .iter()
        .map(|fd| libc::pollfd {
            fd: fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        })
        .collect();
    let millis = timeout.map_or(-1, |timeout| {
        i32::try_from(timeout.as_millis().max(1)).unwrap_or(i32::MAX)
    });
    // SAFETY: `polled` is a valid array of `polled.len()` pollfd entries.
    let ready = unsafe { libc::poll(polled.as_mut_ptr(), polled.len() as libc::nfds_t, millis) };
    if ready < 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::Interrupted {
            return Ok(vec![false; fds.len()]);
        }
        return Err(error);
    }
    Ok(polled
        .iter()
        .map(|entry| entry.revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::fd::AsFd;
    use std::os::unix::net::UnixStream;

    #[test]
    fn poll_reports_the_readable_fds() {
        let (a, mut a_peer) = UnixStream::pair().unwrap();
        let (b, _b_peer) = UnixStream::pair().unwrap();
        assert_eq!(
            poll(&[a.as_fd(), b.as_fd()], Some(Duration::from_millis(5))).unwrap(),
            [false, false]
        );
        a_peer.write_all(b"x").unwrap();
        assert_eq!(
            poll(&[a.as_fd(), b.as_fd()], Some(Duration::from_millis(5))).unwrap(),
            [true, false]
        );
    }
}
