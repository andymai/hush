/// Adapter implementations for system tray traits
///
/// Following the Adapter pattern from DESIGN_PATTERNS.md, these modules
/// contain concrete implementations of the system tray traits.

// Linux adapter
#[cfg(all(feature = "system-tray", target_os = "linux"))]
pub mod ksni_adapter;
#[cfg(all(feature = "system-tray", target_os = "linux"))]
pub use ksni_adapter::KsniSystemTray;

// macOS adapter
#[cfg(all(feature = "system-tray", target_os = "macos"))]
pub mod macos_tray_adapter;
#[cfg(all(feature = "system-tray", target_os = "macos"))]
pub use macos_tray_adapter::MacOSTrayAdapter;

pub mod json_history;
#[cfg(feature = "notifications")]
pub mod libnotify_adapter;

pub use json_history::JsonHistoryStore;
#[cfg(feature = "notifications")]
pub use libnotify_adapter::LibnotifyProvider;

use crate::core::traits::SystemTray;
use crate::Result;

/// Create a platform-appropriate system tray adapter
///
/// # Platform Support
///
/// - **Linux**: Uses KsniSystemTray with ksni crate (StatusNotifierItem/D-Bus)
/// - **macOS**: Uses MacOSTrayAdapter with tray-icon crate (NSStatusBar)
///
/// # Returns
///
/// - `Ok(Box<dyn SystemTray>)` - Platform-specific tray adapter
/// - `Err` if adapter creation fails or platform is unsupported
///
/// # Example
///
/// ```ignore
/// use hush::tray::adapters::create_system_tray;
///
/// let mut tray = create_system_tray()?;
/// tray.show().await?;
/// ```
#[cfg(feature = "system-tray")]
pub fn create_system_tray() -> Result<Box<dyn SystemTray>> {
    #[cfg(target_os = "linux")]
    {
        Ok(Box::new(KsniSystemTray::new()?))
    }

    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(MacOSTrayAdapter::new()?))
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Err(anyhow::anyhow!(
            "Unsupported platform for system tray. Supported: Linux, macOS"
        ))
    }
}