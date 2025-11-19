// Core trait-based architecture (new)
pub mod adapters;
pub mod application;
pub mod core;

// Existing modules (legacy - will be migrated)
pub mod audio;
pub mod config;
pub mod hotkey;
pub mod model_downloader;
pub mod text;
pub mod transcription;
pub mod wakeword;

// Overlay module (Wispr Flow-style floating window)
pub mod overlay;

// Text processing module (intelligent auto-editing)
pub mod text_processing;

// System tray integration (temporarily disabled due to compilation issues)
// pub mod tray;

// CLI module (new unified command system)
pub mod cli;
pub mod cli_main;

// Logging system
pub mod logging;

// Re-export main types for convenience (legacy)
pub use audio::{AudioCapture, AudioFeedback};
pub use config::{Config, ConfigWatcher};
pub use hotkey::{HotkeyEvent, HotkeyManager};
pub use text::TextInserter;
pub use transcription::{TranscriptionResult as LegacyTranscriptionResult, WhisperTranscriber};
pub use wakeword::WakeWordDetector;

// Re-export new core types
pub use core::{
    error::HushError,
    mocks::*,
    state::{AppState, StateMachine, StateObserver},
    traits::*,
};

// Common result type
pub type Result<T> = anyhow::Result<T>;
