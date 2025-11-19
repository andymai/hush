# Task: Extract handle_setup Command

## Description

Extract the `handle_setup` command from `src/cli/dispatcher.rs` to `src/cli/commands/setup.rs`, following the pattern established by the `record` and `status` commands.

## Requirements

- [ ] Create `src/cli/commands/setup.rs` with the `handle_setup` function
- [ ] Move the implementation from `dispatcher.rs` (lines ~692-711, ~20 lines)
- [ ] Add proper documentation with doc comments
- [ ] Export the function from `src/cli/commands/mod.rs`
- [ ] Update `dispatcher.rs` to use the extracted function
- [ ] Ensure all imports are correct (especially `SetupCommands` enum)
- [ ] No functionality changes - pure refactoring

## Success Criteria

- `cargo check` passes
- `cargo test` passes (all existing tests still work)
- `cargo clippy -- -D warnings` passes
- `cargo fmt` applied
- Code follows pattern in `src/cli/commands/record.rs`
- Function has comprehensive doc comments explaining setup subcommands

## Context

This is part of a larger refactoring to modularize CLI commands. The `handle_setup` function handles the `setup` subcommand which has multiple sub-options (like `setup uinput --quick`).

**Current state:**
- Pattern: `src/cli/commands/record.rs`
- Exports: `src/cli/commands/mod.rs`
- Dispatcher: `src/cli/dispatcher.rs`

## Files to Check

- `src/cli/dispatcher.rs` - Source of handle_setup (lines ~692-711)
- `src/cli/commands/record.rs` - Pattern to follow
- `src/cli/commands/mod.rs` - Add exports here
- `src/cli/mod.rs` - Check SetupCommands enum definition
- `.ai/knowledge/conventions.md` - Coding standards

## Estimated Complexity

**Low** - Simple function (~20 lines), established pattern, straightforward extraction
