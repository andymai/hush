/// Text output adapters
///
/// Platform-specific implementations of the TextOutput trait.
/// - Linux: X11TextAdapter
/// - macOS: MacOSTextAdapter

// Linux adapter
#[cfg(target_os = "linux")]
mod x11_adapter;
#[cfg(target_os = "linux")]
pub use x11_adapter::X11TextAdapter;

// macOS adapter
#[cfg(target_os = "macos")]
mod macos_adapter;
#[cfg(target_os = "macos")]
pub use macos_adapter::MacOSTextAdapter;

use crate::core::traits::TextOutput;
use crate::Result;

/// Create a platform-appropriate text output adapter
///
/// # Platform Support
///
/// - **Linux**: Uses X11TextAdapter with UInput for universal text insertion
/// - **macOS**: Uses MacOSTextAdapter with CGEvent API for keyboard simulation
///
/// # Returns
///
/// - `Ok(Box<dyn TextOutput>)` - Platform-specific adapter
/// - `Err` if adapter creation fails or platform is unsupported
///
/// # Example
///
/// ```ignore
/// use hush::adapters::text::create_text_adapter;
///
/// let text_output = create_text_adapter()?;
/// text_output.insert_text("Hello, world!").await?;
/// ```
pub fn create_text_adapter() -> Result<Box<dyn TextOutput>> {
    #[cfg(target_os = "linux")]
    {
        Ok(Box::new(X11TextAdapter::new()?))
    }

    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(MacOSTextAdapter::new()?))
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Err(anyhow::anyhow!(
            "Unsupported platform for text insertion. Supported: Linux, macOS"
        ))
    }
}
