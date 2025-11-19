# Task: Reduce unwrap()/expect() Usage in CLI Modules

## Description

Replace `unwrap()` and `expect()` calls with proper error handling in CLI modules (`src/cli/`, `src/cli/commands/`). This completes the error handling improvements started in T-007 and T-008.

## Requirements

- [ ] Audit `src/cli/dispatcher.rs` for unwrap/expect usage
- [ ] Audit `src/cli/commands/` for unwrap/expect usage
- [ ] Replace with `?` operator where possible
- [ ] Add proper error handling with user-friendly messages
- [ ] Use appropriate `HushError` variants or anyhow for CLI
- [ ] Document any remaining justified unwrap/expect calls
- [ ] No functionality changes - pure error handling improvement

## Success Criteria

- `cargo check` passes
- `cargo test` passes
- `cargo clippy -- -D warnings` passes
- `cargo clippy -- -W clippy::unwrap_used -W clippy::expect_used` shows minimal unwraps in production CLI code
- `cargo fmt` applied
- CLI provides helpful error messages instead of panicking

## Context

Architecture analysis found **150+ occurrences** of `unwrap()/expect()`. After T-007 (core) and T-008 (business logic), this task completes the cleanup in CLI modules.

**Focus areas:**
- `src/cli/dispatcher.rs` - 14 occurrences
- `src/cli/commands/setup.rs` - 2 occurrences
- `src/cli/commands/record.rs` - 2 occurrences
- `src/cli_main.rs` - 1 occurrence

**Note:**
- Test code can keep `unwrap()`
- CLI code can use `anyhow` for quick error reporting
- Focus on user-facing error messages

## Commands to Run

```bash
# Find all unwrap/expect in CLI
rg "\.unwrap\(\)|\.expect\(" src/cli src/cli_main.rs --type rust

# Verify fixes
cargo clippy -- -W clippy::unwrap_used -W clippy::expect_used
cargo test
```

## Files to Check

- `src/cli/dispatcher.rs` - Main dispatcher unwraps
- `src/cli/commands/setup.rs` - Setup command unwraps
- `src/cli/commands/record.rs` - Record command unwraps
- `.ai/knowledge/conventions.md` - Error handling patterns
- `src/core/error.rs` - HushError::user_message() for friendly errors

## Error Handling Pattern for CLI

```rust
// Before:
let config = load_config().unwrap();

// After (CLI-friendly):
let config = load_config()
    .context("Failed to load configuration. Run 'hush setup' to create config.")?;

// Or use HushError user messages:
if let Err(e) = run_command() {
    eprintln!("Error: {}", e.user_message());
    std::process::exit(1);
}
```

## Dependencies

- **Requires:** T-007 completed (core modules cleaned)
- **Requires:** T-008 completed (business logic cleaned)
- **Blocks:** Full codebase error handling compliance

## Estimated Complexity

**Low-Medium** - CLI unwraps are often simple, but need user-friendly error messages. Fewer occurrences than T-008.

## Related Tasks

- Final task in unwrap/expect cleanup series (T-007, T-008, T-009)
- Part of Priority 1 recommendations from architecture analysis
- Completes critical reliability improvement
