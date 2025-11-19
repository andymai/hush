# Task: Extract handle_models Command

## Description

Extract the `handle_models` command from `src/cli/dispatcher.rs` to `src/cli/commands/models.rs`, following the pattern established by the `record` and `status` commands.

## Requirements

- [ ] Create `src/cli/commands/models.rs` with the `handle_models` function
- [ ] Move the implementation from `dispatcher.rs` (lines ~741-793)
- [ ] Add proper documentation with doc comments
- [ ] Export the function from `src/cli/commands/mod.rs`
- [ ] Update `dispatcher.rs` to use the extracted function
- [ ] Ensure all imports are correct (especially `ModelCommands` enum)
- [ ] No functionality changes - pure refactoring

## Success Criteria

- `cargo check` passes
- `cargo test` passes (all existing tests still work)
- `cargo clippy -- -D warnings` passes
- `cargo fmt` applied
- Code follows pattern in `src/cli/commands/record.rs`
- Function has comprehensive doc comments

## Context

This is part of a larger refactoring to modularize CLI commands. The `handle_models` function handles model management (list, download, etc.).

**Current state:**
- Pattern: `src/cli/commands/record.rs`
- Exports: `src/cli/commands/mod.rs`
- Dispatcher: `src/cli/dispatcher.rs` (lines ~741-793)

## Files to Check

- `src/cli/dispatcher.rs` - Source of handle_models
- `src/cli/commands/record.rs` - Pattern to follow
- `src/cli/commands/mod.rs` - Add exports here
- `src/cli/mod.rs` - Check ModelCommands enum definition
- `.ai/knowledge/conventions.md` - Coding standards

## Estimated Complexity

**Medium** - Moderate size, model management logic, but follows established pattern
