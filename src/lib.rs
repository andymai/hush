// Core trait-based architecture (new)
pub mod adapters;
pub mod application;
pub mod core;

// Existing modules (legacy - will be migrated)
pub mod audio;
pub mod config;
pub mod hotkey;
pub mod text;
pub mod transcription;

// Overlay module (Wispr Flow-style floating window)
pub mod overlay;

// Text processing module (intelligent auto-editing)
pub mod text_processing;

// CLI module (new unified command system)
pub mod cli;
pub mod cli_main;

// Logging system
pub mod logging;

// Device permissions for hotkeys and text insertion
pub mod permissions;

// Daemon control socket
pub mod ipc;

// Re-export main types for convenience (legacy)
pub use audio::{AudioCapture, AudioFeedback};
pub use config::Config;
pub use hotkey::{HotkeyEvent, HotkeyManager};

#[cfg(target_os = "linux")]
pub use text::TextInserter;

pub use transcription::{TranscriptionResult as LegacyTranscriptionResult, WhisperTranscriber};

// Re-export new core types
pub use core::{
    error::HushError,
    mocks::*,
    state::{AppState, StateMachine, StateObserver},
    traits::*,
};

// Common result type
pub type Result<T> = anyhow::Result<T>;
