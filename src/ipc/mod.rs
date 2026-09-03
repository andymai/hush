//! Control channel between the running daemon and `hush toggle`, `hush stop`,
//! and friends: newline-delimited JSON over a Unix socket in the runtime
//! directory, plus the PID file that keeps a single daemon per user.

pub mod client;
pub mod lock;
pub mod paths;
pub mod protocol;
pub mod server;

pub use protocol::{DaemonCommand, DaemonResponse, DaemonState};
pub use server::{SessionCommand, SharedState};
