// Text output adapters (Linux only)

mod x11_adapter;
pub use x11_adapter::X11TextAdapter;

use crate::core::traits::TextOutput;
use crate::Result;

/// Create the text output adapter (X11 with UInput)
pub fn create_text_adapter() -> Result<Box<dyn TextOutput>> {
    Ok(Box::new(X11TextAdapter::new()?))
}
