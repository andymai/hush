# T-042: Fix Test Binaries Compilation Errors

**Priority:** Medium
**Effort:** Low-Medium (2-3 hours)
**Type:** Bug Fix - Technical Debt
**Status:** Available
**Created:** 2025-11-19

---

## Problem Statement

Several test and example binaries fail to compile due to outdated imports and API mismatches after codebase refactoring. These are not part of the main application but prevent full `cargo test` and `cargo build --all-targets` from succeeding.

### Failing Binaries

1. **`src/bin/hush-new.rs`** - Import error
   ```
   error[E0432]: unresolved import `hush::text::TextInserter`
   ```

2. **`src/bin/test-overlay.rs`** - Multiple import/API errors
   ```
   error[E0432]: unresolved import `hush::adapters::X11TextAdapter`
   error[E0599]: no method named `insert_text` found for enum `Result<T, E>`
   ```

3. **`src/bin/test-full-integration.rs`** - parking_lot API misuse
   ```
   error[E0599]: no method named `unwrap` found for struct `MutexGuard`
   ```

4. **`src/bin/test-intelligent-editing.rs`** - Multiple API errors
   ```
   error[E0599]: no method named `unwrap` found for struct `MutexGuard`
   (4 occurrences)
   ```

---

## Goals

1. ✅ Fix all compilation errors in test binaries
2. ✅ Update imports to use new adapter factory pattern
3. ✅ Fix parking_lot MutexGuard usage (no `.unwrap()` method)
4. ✅ Update API calls to match current codebase structure
5. ✅ Ensure `cargo build --all-targets` succeeds
6. ✅ Consider moving these to `examples/` directory if they're demonstrations

---

## Root Causes

### 1. Removed `TextInserter` Type
The old `TextInserter` was replaced with the trait-based `TextOutput` approach and platform adapters.

**Solution:** Use factory functions like `create_text_adapter()` instead.

### 2. parking_lot MutexGuard API
`parking_lot::Mutex` guards don't have an `.unwrap()` method. The code appears to be trying to use `std::sync::Mutex` API.

**Problem code:**
```rust
*state_handle.lock().unwrap() = new_state;
```

**Should be:**
```rust
*state_handle.lock() = new_state;
```

### 3. Direct Adapter Imports
Code imports platform-specific adapters directly instead of using factory pattern.

**Problem code:**
```rust
use hush::adapters::X11TextAdapter;
```

**Should be:**
```rust
use hush::adapters::text::create_text_adapter;
let adapter = create_text_adapter()?;
```

---

## Implementation Steps

### 1. Fix `src/bin/hush-new.rs`

**Search for:**
```bash
rg "TextInserter" src/bin/hush-new.rs
```

**Update imports:**
```rust
// OLD:
use hush::text::TextInserter;

// NEW:
use hush::adapters::text::create_text_adapter;
use hush::core::traits::TextOutput;
```

**Update usage:**
```rust
// OLD:
let text_inserter = TextInserter::new()?;

// NEW:
let mut text_output = create_text_adapter()?;
text_output.insert_text("Hello").await?;
```

### 2. Fix `src/bin/test-overlay.rs`

**Update imports:**
```rust
// Remove:
use hush::adapters::X11TextAdapter;
use hush::text::TextInserter;

// Add:
use hush::adapters::text::create_text_adapter;
use hush::core::traits::TextOutput;
```

**Fix API calls:**
```rust
// OLD:
text_adapter.insert_text("test").await
    .unwrap()
    .insert_text("more text");

// NEW:
text_adapter.insert_text("test").await?;
text_adapter.insert_text("more text").await?;
```

### 3. Fix `src/bin/test-full-integration.rs` and `test-intelligent-editing.rs`

**Find all `.lock().unwrap()` patterns:**
```bash
rg "\.lock\(\)\.unwrap\(\)" src/bin/test-*.rs
```

**Fix parking_lot usage:**
```rust
// OLD (std::sync::Mutex pattern):
*state_handle.lock().unwrap() = OverlayState::Recording;

// NEW (parking_lot::Mutex):
*state_handle.lock() = OverlayState::Recording;
```

**Explanation:** `parking_lot::Mutex::lock()` returns `MutexGuard` directly, never panics (no poisoning), so no `.unwrap()` is needed or available.

### 4. Verify All Binaries Compile

```bash
# Test each binary individually
cargo build --bin hush-new
cargo build --bin test-overlay
cargo build --bin test-full-integration
cargo build --bin test-intelligent-editing

# Test all targets
cargo build --all-targets
```

### 5. Consider Moving to Examples

If these are demonstration/test programs, consider:

```bash
# Move to examples directory
mkdir -p examples
git mv src/bin/test-*.rs examples/
git mv src/bin/hush-new.rs examples/

# Update Cargo.toml
# Remove [[bin]] sections, add [[example]] sections
```

---

## Success Criteria

- [ ] `cargo build --bin hush-new` succeeds
- [ ] `cargo build --bin test-overlay` succeeds
- [ ] `cargo build --bin test-full-integration` succeeds
- [ ] `cargo build --bin test-intelligent-editing` succeeds
- [ ] `cargo build --all-targets` succeeds
- [ ] No compilation errors remain
- [ ] All imports use current codebase structure

---

## Verification Steps

```bash
# Clean build to ensure no cached artifacts
cargo clean

# Build all targets
cargo build --all-targets

# Verify no errors
echo $?  # Should be 0

# Test main binary still works
cargo build --bin hush
./target/debug/hush --version
```

---

## Files to Modify

- `src/bin/hush-new.rs`
- `src/bin/test-overlay.rs`
- `src/bin/test-full-integration.rs`
- `src/bin/test-intelligent-editing.rs`
- Optionally: `Cargo.toml` (if moving to examples)

---

## Dependencies

**Blocks:**
- Clean CI builds
- Full test suite execution

**Blocked by:**
- None (can be done immediately)

---

## References

- **parking_lot docs**: https://docs.rs/parking_lot/latest/parking_lot/
  - Key difference: No poisoning, `lock()` returns guard directly
- **Trait-based architecture**: `.ai/knowledge/architecture.md`
- **Factory pattern**: `src/adapters/text/mod.rs`, `src/adapters/tray/mod.rs`

---

## Notes

These binaries appear to be:
- **`hush-new.rs`**: Experimental new main entry point (?)
- **`test-overlay.rs`**: Manual test for overlay UI
- **`test-full-integration.rs`**: Integration test with real components
- **`test-intelligent-editing.rs`**: Test for text processing features

Consider whether these should be:
1. Fixed and maintained as examples
2. Moved to `tests/` as proper integration tests
3. Removed if no longer needed
4. Converted to unit tests

Discuss with maintainer before removing.

---

## Estimated Impact

- **Time to fix**: 2-3 hours
- **Risk**: Low (isolated to test binaries)
- **Benefit**: Clean builds, better CI, maintainable codebase
