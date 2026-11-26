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

// Floating window that shows the recording, transcription, and insertion state
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

/// Desktop notifications
pub mod notify;

/// Opt-in check for a newer release
pub mod updates;

/// The shape of the Hush mark, shared by the tray icon and the settings window
pub mod mark;

/// Panel status icon
pub mod tray;

/// Setup and settings window
pub mod gui;

/// Launcher and autostart entries
pub mod desktop;

/// The embedded fonts
pub mod fonts;

/// The systemd user unit that the packages ship
pub mod service;

/// Desktop portals for a sandboxed Hush
#[cfg(target_os = "linux")]
pub mod portal;

#[cfg(test)]
mod test_support;

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
