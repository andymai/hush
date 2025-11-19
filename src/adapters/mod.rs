/// Adapters for existing implementations to new trait-based architecture
///
/// These adapters wrap existing concrete types to implement the new traits,
/// enabling gradual migration without breaking existing code.
pub mod audio;
pub mod hotkey;
pub mod text;
pub mod transcription;

pub use audio::CpalAudioAdapter;
pub use hotkey::HotkeyTriggerAdapter;
pub use text::X11TextAdapter;
pub use transcription::WhisperAdapter;
