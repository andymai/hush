# T-043: Fix Library Test Compilation Errors

**Priority:** Medium
**Effort:** Low (1-2 hours)
**Type:** Bug Fix - Technical Debt
**Status:** Available
**Created:** 2025-11-19

---

## Problem Statement

The library test suite (`cargo test --lib`) fails to compile due to:
1. Private field access in `application/builder.rs` tests
2. Outdated field names in `text_processing/mod.rs` tests
3. Some unused variable warnings

### Compilation Errors

**Error 1: Private field access (3 occurrences)**
```rust
error[E0616]: field `mode` of struct `hush_app::HushApp` is private
  --> src/application/builder.rs:162:30
   |
162 |         assert!(matches!(app.mode, AppMode::Daemon));
    |                              ^^^^ private field
```

**Error 2: Nonexistent config fields (2 occurrences)**
```rust
error[E0560]: struct `text_processing::config::ProcessingConfig` has no field named `use_llm`
  --> src/text_processing/mod.rs:268:13
   |
268 |             use_llm: false,
    |             ^^^^^^^ `ProcessingConfig` does not have this field
    |
    = note: available fields are: `llm_provider`, `max_tokens`, `temperature`
```

---

## Goals

1. ✅ Fix private field access in HushApp tests
2. ✅ Update ProcessingConfig test to use current field names
3. ✅ Ensure `cargo test --lib` compiles and runs
4. ✅ Fix or acknowledge unused variable warnings

---

## Root Causes

### 1. HushApp Private Fields
The `mode` field in `HushApp` was made private but tests are trying to access it directly.

**Solution:** Add a public getter method or make assertions via public API.

### 2. ProcessingConfig API Changed
The config struct was refactored from `use_llm: bool` + `llm_model_path` to `llm_provider` enum.

**Solution:** Update tests to use new field names.

---

## Implementation Steps

### 1. Fix HushApp Private Field Access

**File:** `src/application/builder.rs`

**Option A: Add public getter (recommended)**

Add to `src/application/hush_app.rs`:
```rust
impl HushApp {
    /// Get the application mode
    pub fn mode(&self) -> &AppMode {
        &self.mode
    }
}
```

Update tests in `builder.rs`:
```rust
// OLD:
assert!(matches!(app.mode, AppMode::Daemon));

// NEW:
assert!(matches!(app.mode(), AppMode::Daemon));
```

**Option B: Make field pub(crate)**

In `src/application/hush_app.rs`:
```rust
pub struct HushApp {
    pub(crate) mode: AppMode,  // Visible within crate
    // ...
}
```

**Recommendation:** Use Option A (public getter) for better encapsulation.

### 2. Fix ProcessingConfig Field Names

**File:** `src/text_processing/mod.rs` (lines 268-269)

**Current config definition:**
```bash
rg "pub struct ProcessingConfig" -A 10 src/text_processing/
```

**Expected fields:** `llm_provider`, `max_tokens`, `temperature`

**Update test:**
```rust
// OLD:
let config = ProcessingConfig {
    use_llm: false,
    llm_model_path: Default::default(),
};

// NEW:
let config = ProcessingConfig {
    llm_provider: None,  // or LlmProvider::None
    max_tokens: 100,
    temperature: 0.7,
};
```

**Or use default:**
```rust
let config = ProcessingConfig::default();
```

### 3. Fix Unused Variable Warnings

**Files with warnings:**
- `src/cli/dispatcher.rs:600` - `all_methods`
- `src/cli/dispatcher.rs:601` - `uinput`
- `src/cli/dispatcher.rs:709` - `text_inserter`
- `src/application/hush_app.rs:438` - `urgency`

**Fix by prefixing with underscore:**
```rust
// OLD:
fn foo(urgency: NotificationUrgency) { ... }

// NEW:
fn foo(_urgency: NotificationUrgency) { ... }
```

Or remove if truly unused.

### 4. Verify Tests Compile and Run

```bash
# Build library tests
cargo test --lib --no-run

# Run library tests
cargo test --lib

# Check for remaining warnings
cargo clippy --lib -- -D warnings
```

---

## Success Criteria

- [ ] `cargo test --lib --no-run` compiles without errors
- [ ] `cargo test --lib` runs successfully
- [ ] All test assertions pass
- [ ] No unused variable warnings (or all marked with `_` prefix)
- [ ] HushApp mode field properly encapsulated
- [ ] ProcessingConfig tests use current API

---

## Verification Steps

```bash
# Clean build
cargo clean

# Compile library tests
cargo test --lib --no-run
echo "Exit code: $?"  # Should be 0

# Run library tests
cargo test --lib
echo "Exit code: $?"  # Should be 0

# Check for warnings
cargo clippy --lib -- -D warnings
```

---

## Files to Modify

### Required Fixes
1. `src/application/hush_app.rs` - Add `mode()` getter method
2. `src/application/builder.rs:162,174,186` - Update test assertions
3. `src/text_processing/mod.rs:268-269` - Fix ProcessingConfig construction

### Optional Cleanup
4. `src/cli/dispatcher.rs:600-601,709` - Prefix unused variables with `_`
5. `src/application/hush_app.rs:438` - Prefix `urgency` with `_`

---

## Dependencies

**Blocks:**
- Clean test runs
- CI test phase
- Developer confidence in test suite

**Blocked by:**
- None (can be done immediately)

---

## Related Issues

- ProcessingConfig API change likely happened during LLM provider refactoring
- HushApp encapsulation improved but tests weren't updated
- These are test-only issues, production code unaffected

---

## Testing Strategy

After fixes:

```bash
# Unit tests should pass
cargo test --lib

# Specific test files
cargo test --lib application::builder::tests
cargo test --lib text_processing::tests

# Integration tests (separate from this task)
cargo test --test '*'
```

---

## Notes

**Why were these not caught earlier?**
- Tests may not have been run after recent refactorings
- CI may be running `cargo build` but not `cargo test --lib`
- Consider adding `cargo test --lib` to CI if not present

**Impact:**
- Low risk: Only affects tests, not production code
- Quick fix: All issues are straightforward API updates

---

## Estimated Impact

- **Time to fix**: 1-2 hours
- **Risk**: Very low (test code only)
- **Benefit**: Reliable test suite, better CI
