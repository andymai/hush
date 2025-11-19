pub mod dispatcher;
pub mod commands;

pub use dispatcher::CommandDispatcher;

// Re-export CLI types from cli_main.rs
pub use crate::cli_main::{Cli, Commands, SetupCommands, TestCommands, ModelCommands};
