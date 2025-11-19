# Task: Reduce unwrap()/expect() Usage in Business Logic Modules

## Description

Replace `unwrap()` and `expect()` calls with proper error handling in business logic modules (`src/transcription/`, `src/text/`, `src/text_processing/`, `src/audio/`). This continues the error handling improvements from T-007.

## Requirements

- [ ] Audit `src/transcription/` for unwrap/expect usage
- [ ] Audit `src/text/` for unwrap/expect usage
- [ ] Audit `src/text_processing/` for unwrap/expect usage
- [ ] Audit `src/audio/` for unwrap/expect usage
- [ ] Replace with `?` operator where possible
- [ ] Add proper error handling with context
- [ ] Use appropriate `HushError` variants
- [ ] Document any remaining justified unwrap/expect calls
- [ ] No functionality changes - pure error handling improvement

## Success Criteria

- `cargo check` passes
- `cargo test` passes
- `cargo clippy -- -D warnings` passes
- `cargo clippy -- -W clippy::unwrap_used -W clippy::expect_used` shows significant reduction
- `cargo fmt` applied
- Critical paths (transcription, text insertion) are panic-free

## Context

Architecture analysis found **150+ occurrences** of `unwrap()/expect()`. After T-007 handles core modules, this task addresses business logic modules.

**Focus areas:**
- `src/transcription/whisper.rs` - 2 occurrences
- `src/transcription/models.rs` - 8 occurrences
- `src/transcription/mod.rs` - 2 occurrences
- `src/text/mod.rs` - 2 occurrences
- `src/text_processing/` modules - 35+ occurrences
- `src/audio/mod.rs` - 3 occurrences

**Note:** Test code can keep `unwrap()`.

## Commands to Run

```bash
# Find all unwrap/expect in business logic
rg "\.unwrap\(\)|\.expect\(" src/transcription src/text src/text_processing src/audio --type rust

# Verify fixes
cargo clippy -- -W clippy::unwrap_used -W clippy::expect_used
cargo test
```

## Files to Check

- `src/transcription/models.rs` - Model loading unwraps
- `src/text_processing/executor.rs` - 7 occurrences
- `src/text_processing/intent.rs` - 8 occurrences
- `src/text_processing/filler_words.rs` - 8 occurrences
- `.ai/knowledge/conventions.md` - Error handling patterns
- `src/core/error.rs` - Available error variants

## Dependencies

- **Requires:** T-007 completed first (establishes pattern in core modules)
- **Blocks:** T-010 (documentation examples need clean error handling)

## Estimated Complexity

**Medium-High** - More unwraps than T-007, some complex error propagation in text processing pipeline. Requires understanding business logic to choose appropriate error messages.

## Related Tasks

- Continuation of T-007
- Follow-up: T-009 will handle CLI modules
- Part of Priority 1 recommendations from architecture analysis
