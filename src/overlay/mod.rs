//! Floating overlay window with visual feedback for recording, transcription,
//! and text insertion.
//!
//! ```no_run
//! use hush::overlay::{OverlayState, OverlayWindowBuilder};
//! use std::time::Duration;
//!
//! let overlay = OverlayWindowBuilder::new()
//!     .width(300.0)
//!     .height(100.0)
//!     .show_button_when_idle(true)
//!     .build();
//! let state = overlay.state();
//!
//! // The window loop blocks, so it gets its own thread.
//! std::thread::spawn(move || {
//!     let _ = overlay.run();
//! });
//!
//! *state.lock() = OverlayState::start_recording();
//! std::thread::sleep(Duration::from_secs(3));
//! *state.lock() = OverlayState::processing("Transcribing...");
//! std::thread::sleep(Duration::from_secs(2));
//! *state.lock() = OverlayState::success("Hello world", Duration::from_secs(2));
//! ```

mod state;
mod ui;
mod window;

pub use state::{OverlayConfig, OverlayPosition, OverlayState, OverlayTheme};
pub use window::{OverlayWindow, OverlayWindowBuilder};
