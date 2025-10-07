/// Adapters for existing implementations to new trait-based architecture
///
/// These adapters wrap existing concrete types to implement the new traits,
/// enabling gradual migration without breaking existing code.

pub mod audio;
pub mod transcription;
pub mod text;
pub mod hotkey;

pub use audio::CpalAudioAdapter;
pub use transcription::WhisperAdapter;
pub use text::X11TextAdapter;
pub use hotkey::HotkeyTriggerAdapter;
