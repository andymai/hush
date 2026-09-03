// Text output adapters (Linux only)

mod inserter_adapter;
pub use inserter_adapter::TextInserterAdapter;

use crate::core::traits::TextOutput;
use crate::Result;

/// Create the text output adapter (X11 with UInput)
pub fn create_text_adapter() -> Result<Box<dyn TextOutput>> {
    Ok(Box::new(TextInserterAdapter::new()?))
}
