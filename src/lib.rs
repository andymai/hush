// Core trait-based architecture (new)
pub mod core;
pub mod adapters;
pub mod application;

// Existing modules (legacy - will be migrated)
pub mod audio;
pub mod config;
pub mod error;
pub mod hotkey;
pub mod model_downloader;
pub mod text;
pub mod transcription;
pub mod wakeword;

// TUI module
pub mod tui;

// System tray integration (temporarily disabled due to compilation issues)
// pub mod tray;

// CLI module (new unified command system)
pub mod cli_main;
pub mod cli;

// Logging system
pub mod logging;

// Re-export main types for convenience (legacy)
pub use audio::{AudioCapture, AudioFeedback};
pub use config::{Config, ConfigWatcher};
pub use error::{ErrorHandler, HushError as LegacyHushError};
pub use hotkey::{HotkeyEvent, HotkeyManager};
pub use text::TextInserter;
pub use transcription::{WhisperTranscriber, TranscriptionResult as LegacyTranscriptionResult};
pub use wakeword::WakeWordDetector;

// Re-export new core types
pub use core::{
    traits::*,
    error::HushError,
    state::{AppState, StateMachine, StateObserver},
    mocks::*,
};

// Common result type
pub type Result<T> = anyhow::Result<T>;