//! The fonts that Hush embeds, shared by the settings window and the
//! overlay, so the binary carries each file once.

pub const GEIST_REGULAR: &[u8] = include_bytes!("gui/fonts/Geist-Regular.ttf");
pub const GEIST_MEDIUM: &[u8] = include_bytes!("gui/fonts/Geist-Medium.ttf");
pub const GEIST_SEMIBOLD: &[u8] = include_bytes!("gui/fonts/Geist-SemiBold.ttf");
pub const GEIST_MONO_REGULAR: &[u8] = include_bytes!("gui/fonts/GeistMono-Regular.ttf");
pub const GEIST_MONO_MEDIUM: &[u8] = include_bytes!("gui/fonts/GeistMono-Medium.ttf");
/// The Lucide icons that `gui::icons` names
pub const LUCIDE: &[u8] = include_bytes!("gui/fonts/lucide.ttf");
