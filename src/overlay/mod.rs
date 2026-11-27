//! The recording overlay: a pill at the bottom of the screen with the
//! waveform, the time, and badges while a take records, dots while it is
//! transcribed, and a line for an error.
//!
//! It runs as its own process, `hush overlay`, which the daemon starts with
//! a socket pair on fd 3 ([`supervisor`]). The process watches the daemon's
//! state over that socket (`link`), turns it into a scene ([`view`]), draws
//! the scene on the CPU ([`render`]) in the colours of [`theme`], and shows
//! it on a layer-shell surface or an X11 window ([`present`]). A fault in it
//! cannot stop dictation, and it needs no GPU.

mod link;
pub mod present;
pub mod render;
pub mod supervisor;
pub mod theme;
pub mod view;

use crate::ipc::protocol::DaemonCommand;
use crate::portal::appearance::{self, Appearance};
use present::{Failure, Pointer};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{error, info};

/// The exit code of an overlay that found neither layer-shell nor X11, so
/// that the daemon does not start it again. Only the daemon reads it.
pub const NO_SURFACE: i32 = 10;

/// `hush overlay`: runs until the daemon closes the socket.
pub fn run() -> ! {
    let code = match show_until_closed() {
        Ok(()) => 0,
        Err(Exit::NoSurface) => {
            info!("No surface for the overlay: the session has neither layer-shell nor X11");
            NO_SURFACE
        },
        Err(Exit::Presenter(Failure::Lost)) => {
            info!("The overlay's display connection ended");
            0
        },
        Err(Exit::Presenter(Failure::Broken(e))) => {
            error!("The overlay stopped: {:#}", e);
            1
        },
    };
    std::process::exit(code)
}

enum Exit {
    NoSurface,
    Presenter(Failure),
}

impl From<Failure> for Exit {
    fn from(failure: Failure) -> Self {
        Exit::Presenter(failure)
    }
}

fn show_until_closed() -> Result<(), Exit> {
    let Some(mut link) = link::Link::open().map_err(|e| Exit::Presenter(Failure::Broken(e)))?
    else {
        info!("The daemon closed the overlay's socket before the overlay started");
        return Ok(());
    };
    let mut presenter = present::open().ok_or(Exit::NoSurface)?;
    info!(presenter = presenter.name(), "The overlay runs");
    // A daemon restart follows each settings change, and restarts the overlay.
    let choice = crate::config::Config::load()
        .map(|config| config.overlay.theme)
        .unwrap_or_default();
    let mut theme = theme::Theme::new(choice, theme::file_path());
    let desktop = Arc::new(Mutex::new(Appearance::default()));
    let latest = Arc::clone(&desktop);
    appearance::watch(move |appearance| {
        if let Ok(mut shared) = latest.lock() {
            *shared = appearance;
        }
    });
    let clock = Instant::now();
    let mut view = view::View::default();
    let mut shown = false;
    let mut hovered = false;
    loop {
        let now = Instant::now();
        let scene = view.scene(now);
        let recording = matches!(scene, Some(view::Scene::Recording { .. }));
        if !recording {
            hovered = false;
        }
        presenter.set_input(scene.as_ref().filter(|_| recording).map(render::pill_rect))?;
        match scene {
            Some(scene) => {
                let seconds = now.duration_since(clock).as_secs_f32();
                let appearance = desktop.lock().map(|shared| *shared).unwrap_or_default();
                let look = theme.look(&appearance, now);
                let frame = render::render(&scene, &look, presenter.scale(), seconds, hovered);
                presenter.present(frame.as_ref())?;
                shown = true;
            },
            None if shown => {
                presenter.present(None)?;
                shown = false;
            },
            None => {},
        }
        let wake = presenter.wait(link.fd(), view.next_change(now))?;
        for event in wake.pointer {
            match event {
                Pointer::Enter => hovered = true,
                Pointer::Leave => hovered = false,
                Pointer::Primary if recording => link::ask(&DaemonCommand::Stop { print: false }),
                Pointer::Secondary if recording => link::ask(&DaemonCommand::Cancel),
                _ => {},
            }
        }
        if wake.link {
            for line in link.read() {
                view.apply(line, Instant::now());
            }
            if link.closed() {
                return Ok(());
            }
        }
    }
}
