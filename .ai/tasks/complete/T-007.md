# Task: Reduce unwrap()/expect() Usage in Core Modules

## Description

Replace `unwrap()` and `expect()` calls with proper error handling in core modules (`src/core/`, `src/adapters/`, `src/application/`). This is a critical code quality improvement to prevent potential panics in production.

## Requirements

- [ ] Audit `src/core/` for unwrap/expect usage
- [ ] Audit `src/adapters/` for unwrap/expect usage
- [ ] Audit `src/application/` for unwrap/expect usage
- [ ] Replace with `?` operator where possible
- [ ] Add proper error handling with context (`.context()` or `.with_context()`)
- [ ] Use appropriate `HushError` variants
- [ ] Document any remaining justified unwrap/expect calls
- [ ] No functionality changes - pure error handling improvement

## Success Criteria

- `cargo check` passes
- `cargo test` passes (all existing tests still work)
- `cargo clippy -- -D warnings` passes
- `cargo clippy -- -W clippy::unwrap_used -W clippy::expect_used` shows significant reduction in core modules
- `cargo fmt` applied
- All production code paths use `Result<T, E>` or documented panic behavior

## Context

Architecture analysis found **150+ occurrences** of `unwrap()/expect()` across the codebase. This violates Rust best practices and can cause panics. The codebase has excellent structured error handling with `HushError` and `thiserror`, but needs to apply it more consistently.

**Focus areas:** Core modules first (highest impact on reliability)
- `src/core/state.rs` - 6 occurrences
- `src/core/mocks.rs` - 10 occurrences
- `src/adapters/audio/cpal_adapter.rs` - 2 occurrences
- `src/application/builder.rs` - 4 occurrences
- `src/application/hush_app.rs` - 2 occurrences

**Note:** Test code (`#[cfg(test)]`) can keep `unwrap()` as it's acceptable in tests.

## Commands to Run

```bash
# Find all unwrap/expect in core modules
rg "\.unwrap\(\)|\.expect\(" src/core src/adapters src/application --type rust

# Check after fixes
cargo clippy -- -W clippy::unwrap_used -W clippy::expect_used

# Ensure no regressions
cargo test
```

## Files to Check

- `src/core/state.rs` - RwLock unwraps (use proper error handling or document panics)
- `src/core/mocks.rs` - Mock implementations (some unwraps may be acceptable)
- `src/adapters/audio/cpal_adapter.rs` - Audio adapter unwraps
- `src/application/builder.rs` - Builder validation unwraps
- `.ai/knowledge/conventions.md` - Error handling patterns

## Error Handling Pattern

```rust
// Before:
let value = some_result.unwrap();

// After:
let value = some_result
    .context("Failed to get value")?;

// Or with specific error:
let value = some_result
    .map_err(|e| HushError::Audio(AudioError::StreamError(e.to_string())))?;
```

## Estimated Complexity

**Medium** - Requires careful analysis of each unwrap/expect, understanding error propagation, and choosing appropriate error types. Focus on production code paths first.

## Related Tasks

- Part of Priority 1 recommendations from architecture analysis
- Should be done before expanding to CLI modules
- Follow-up task will address unwraps in CLI/text/transcription modules
