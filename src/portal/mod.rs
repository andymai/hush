//! Desktop portals for a sandboxed Hush, as Flathub requires.
//!
//! Inside a Flatpak sandbox, Hush has no `/dev/uinput` and no `/dev/input`.
//! The RemoteDesktop portal types the text, the GlobalShortcuts portal reads
//! the hotkeys, and the Background portal starts the daemon at login. Native
//! installs keep evdev and uinput, which also reach VMs, games, and password
//! prompts that no portal covers.
//!
//! Each portal sits behind a trait, so the logic around it (consent, the
//! restore token, the gesture feed) runs in tests with a mock.

pub mod background;
pub mod keyboard;
pub mod keysym;
pub mod mode;
pub mod probe;
pub mod shortcuts;
pub mod token;

pub use mode::{sandboxed, InputMode};

/// Why a portal call did not work.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PortalError {
    /// The desktop has no such portal, or an older version of it.
    #[error("the desktop has no {0} portal")]
    Unavailable(String),
    /// The user said no in the consent dialog.
    #[error("the desktop did not allow it")]
    Refused,
    #[error("{0}")]
    Failed(String),
}

impl PortalError {
    /// Maps an ashpd error, with `portal` naming the interface.
    pub fn from_ashpd(portal: &str, error: ashpd::Error) -> Self {
        use ashpd::desktop::ResponseError;
        match error {
            ashpd::Error::Response(ResponseError::Cancelled) => Self::Refused,
            ashpd::Error::PortalNotFound(_) | ashpd::Error::RequiresVersion(..) => {
                Self::Unavailable(portal.to_string())
            },
            ashpd::Error::Zbus(zbus::Error::MethodError(name, ..))
                if name.as_str() == "org.freedesktop.DBus.Error.UnknownMethod"
                    || name.as_str() == "org.freedesktop.DBus.Error.ServiceUnknown" =>
            {
                Self::Unavailable(portal.to_string())
            },
            other => Self::Failed(format!("{} portal: {}", portal, other)),
        }
    }
}

/// The user refused keyboard access in the RemoteDesktop consent dialog.
/// The daemon shows a notice of its own for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the desktop did not allow Hush to type; Hush asks again after `hush daemon restart`")]
pub struct ConsentRefused;

/// Runs a future on a thread with a runtime of its own. A caller that
/// already drives a runtime, such as the daemon's insertion thread, cannot
/// block on another one.
pub(crate) fn block_on<F, T>(future: F) -> anyhow::Result<T>
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    std::thread::Builder::new()
        .name("hush-portal".into())
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map(|runtime| runtime.block_on(future))
        })
        .map_err(|e| anyhow::anyhow!("cannot start the portal thread: {}", e))?
        .join()
        .map_err(|_| anyhow::anyhow!("the portal thread stopped"))?
        .map_err(|e| anyhow::anyhow!("cannot start the portal runtime: {}", e))
}
