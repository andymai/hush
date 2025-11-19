pub mod manual;
pub mod record;
pub mod status;
/// Modular command implementations
///
/// This module contains individual command handlers, each in their own file
/// for better organization and maintainability.
///
/// # Architecture
///
/// Each command is implemented as a separate module with its own handler function.
/// This allows for:
/// - Better code organization
/// - Easier testing of individual commands
/// - Clearer separation of concerns
/// - Simpler code review and maintenance
///
/// # Adding New Commands
///
/// To add a new command:
/// 1. Create a new file in `src/cli/commands/`
/// 2. Implement the command handler function
/// 3. Add the module declaration here
/// 4. Export the handler function
/// 5. Update the dispatcher to use the new handler
pub mod utils;

// Re-export command handlers
pub use manual::handle_manual;
pub use record::handle_record;
pub use status::handle_status;

// Re-export utilities for use by other commands
pub use utils::*;

// TODO: Extract remaining commands from dispatcher.rs (REFACTORING_PLAN.md)
// High Priority (frequently used):
// - handle_listen (most complex, 400+ lines)
// - handle_setup
//
// Medium Priority:
// - handle_test
// - handle_models
//
// Low Priority (less frequently used):
// - handle_install
// - handle_uninstall
// - handle_start
