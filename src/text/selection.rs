//! The primary selection: whatever text is highlighted right now, on Wayland
//! (through the data-control protocol) and X11 alike.

use arboard::{Clipboard, GetExtLinux, LinuxClipboardKind};
use tracing::debug;

pub fn read_primary() -> Option<String> {
    let mut clipboard = match Clipboard::new() {
        Ok(clipboard) => clipboard,
        Err(e) => {
            debug!("Selection unavailable: {}", e);
            return None;
        },
    };
    let text = match clipboard
        .get()
        .clipboard(LinuxClipboardKind::Primary)
        .text()
    {
        Ok(text) => text,
        Err(e) => {
            debug!("No primary selection: {}", e);
            return None;
        },
    };
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
