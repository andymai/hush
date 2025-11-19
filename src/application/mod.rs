pub mod builder;
/// Application layer - orchestrates components using trait abstractions
///
/// This module contains the refactored HushApp that uses dependency injection
/// and trait objects instead of concrete types.
pub mod hush_app;

pub use builder::HushAppBuilder;
pub use hush_app::{AppStats, HushApp};
