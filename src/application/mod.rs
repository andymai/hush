/// Application layer - orchestrates components using trait abstractions
///
/// This module contains the refactored HushApp that uses dependency injection
/// and trait objects instead of concrete types.

pub mod hush_app;
pub mod builder;

pub use hush_app::{HushApp, AppStats};
pub use builder::HushAppBuilder;
