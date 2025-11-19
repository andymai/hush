# Task: Optimize CI Tool Installation

## Description

Replace slow `cargo install` commands with `taiki-e/install-action` for faster CI runs. This will save ~7-13 minutes per CI run by using pre-built binaries instead of compiling from source.

## Requirements

- [ ] Replace `cargo install cargo-audit` with `taiki-e/install-action` (line ~69-70)
- [ ] Replace `cargo install cargo-tarpaulin` with `taiki-e/install-action` (line ~89-90)
- [ ] Ensure both tools still work correctly after the change
- [ ] Update step names if needed for clarity

## Success Criteria

- CI runs complete 7-13 minutes faster
- `cargo-audit` still runs successfully in security job
- `cargo-tarpaulin` still runs successfully in coverage job
- No functionality changes, only installation method

## Implementation

Replace in `.github/workflows/ci.yml`:

```yaml
# OLD (lines ~69-70):
- name: Install cargo-audit
  run: cargo install cargo-audit

# NEW:
- name: Install cargo-audit
  uses: taiki-e/install-action@v2
  with:
    tool: cargo-audit
```

```yaml
# OLD (lines ~89-90):
- name: Install tarpaulin
  run: cargo install cargo-tarpaulin

# NEW:
- name: Install tarpaulin
  uses: taiki-e/install-action@v2
  with:
    tool: cargo-tarpaulin
```

## Context

**Current state:** Tools are compiled from source every CI run
**Impact:** Slow CI (5-10 min for tarpaulin, 2-3 min for cargo-audit)
**Priority:** High - major CI performance improvement

## Files to Modify

- `.github/workflows/ci.yml` - Lines ~69-70, ~89-90

## Verification Steps

1. Update both installation steps
2. Verify YAML syntax
3. Trigger CI run and monitor installation times
4. Confirm both jobs (security, coverage) complete successfully

## Estimated Complexity

**Low** - Two simple replacements, 5 minutes

## Dependencies

None - can be done in parallel with other CI tasks
