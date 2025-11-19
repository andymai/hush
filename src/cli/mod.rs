pub mod commands;
pub mod dispatcher;

pub use dispatcher::CommandDispatcher;

// Re-export CLI types from cli_main.rs
pub use crate::cli_main::{Cli, Commands, ModelCommands, SetupCommands, TestCommands};
