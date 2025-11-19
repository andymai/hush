# Task: Extract handle_listen Command

## Description

Extract the `handle_listen` command from `src/cli/dispatcher.rs` to `src/cli/commands/listen.rs`, following the pattern established by the `record` and `status` commands.

## Requirements

- [ ] Create `src/cli/commands/listen.rs` with the `handle_listen` function
- [ ] Move the implementation from `dispatcher.rs` (lines ~264-692, ~429 lines)
- [ ] Add proper documentation with doc comments
- [ ] Export the function from `src/cli/commands/mod.rs`
- [ ] Update `dispatcher.rs` to use the extracted function
- [ ] Ensure all imports are correct (many dependencies)
- [ ] Preserve all internal enums (AudioCommand, TranscriptionResult, etc.)
- [ ] No functionality changes - pure refactoring

## Success Criteria

- `cargo check` passes
- `cargo test` passes (all existing tests still work)
- `cargo clippy -- -D warnings` passes
- `cargo fmt` applied
- Code follows pattern in `src/cli/commands/record.rs`
- Function has comprehensive doc comments explaining listen mode
- All internal helper types properly organized

## Context

This is part of a larger refactoring to modularize CLI commands. The `handle_listen` function is the **most complex** command handler at ~429 lines, containing:
- Hotkey management
- Audio recording pipeline
- Transcription integration
- Text processing with LLM
- Overlay UI management
- Multi-threaded coordination

**Current state:**
- Pattern: `src/cli/commands/record.rs`
- Exports: `src/cli/commands/mod.rs`
- Dispatcher: `src/cli/dispatcher.rs` (lines 264-692)

## Dependencies to Verify

This function uses many imports. Before extraction, verify these exist:
- `HotkeyManager`, `HotkeyEvent` from `crate::hotkey`
- `OverlayState`, `OverlayWindowBuilder` from `crate::overlay`
- `TextProcessor`, `ProcessingConfig`, `EditingMode`, etc. from `crate::text_processing`
- `SimpleWhisperTranscriber` from `crate::transcription`
- Standard audio/config components

## Files to Check

- `src/cli/dispatcher.rs` - Source of handle_listen (lines ~264-692)
- `src/cli/commands/record.rs` - Pattern to follow
- `src/cli/commands/mod.rs` - Add exports here
- `.ai/knowledge/conventions.md` - Coding standards

## Estimated Complexity

**High** - Very complex function (~429 lines), many dependencies, multi-threaded, critical user-facing feature. Should be done **after** simpler extractions to validate the pattern.

## Recommendation

Extract T-001 (handle_manual) and T-002 (handle_setup) first to validate the pattern before tackling this complex function.
