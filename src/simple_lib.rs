// Simple version of lib.rs that only includes the TUI module for demonstration
pub mod tui;

// Common result type
pub type Result<T> = anyhow::Result<T>;