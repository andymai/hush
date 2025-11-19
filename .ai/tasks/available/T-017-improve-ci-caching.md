# Task: Improve CI Caching Strategy

## Description

Replace manual Cargo cache configuration with `Swatinem/rust-cache` which provides more intelligent caching, better handling of Cargo.lock changes, and automatic cache cleanup.

## Requirements

- [ ] Remove manual cache steps from test job (lines ~25-41)
- [ ] Add `Swatinem/rust-cache@v2` action
- [ ] Configure with appropriate settings
- [ ] Verify cache hits are working after change
- [ ] Consider adding to other jobs (security, coverage) if they build

## Success Criteria

- Manual cache steps removed
- `rust-cache` action configured correctly
- CI runs show cache hits in logs
- Build times remain same or improve (target: ~2 min faster)
- No cache-related failures

## Implementation

In `.github/workflows/ci.yml`, replace lines ~25-41:

```yaml
# REMOVE:
- name: Cache cargo registry
  uses: actions/cache@v4
  with:
    path: ~/.cargo/registry
    key: ${{ runner.os }}-cargo-registry-${{ hashFiles('**/Cargo.lock') }}

- name: Cache cargo index
  uses: actions/cache@v4
  with:
    path: ~/.cargo/git
    key: ${{ runner.os }}-cargo-git-${{ hashFiles('**/Cargo.lock') }}

- name: Cache cargo build
  uses: actions/cache@v4
  with:
    path: target
    key: ${{ runner.os }}-cargo-build-target-${{ hashFiles('**/Cargo.lock') }}

# ADD:
- uses: Swatinem/rust-cache@v2
  with:
    shared-key: "ci"
    cache-on-failure: true
```

## Context

**Current state:** Manual cache management with multiple steps
**Impact:** Less optimal caching, more verbose configuration
**Priority:** Medium - improves cache efficiency and maintainability

## Files to Modify

- `.github/workflows/ci.yml` - Lines ~25-41 in test job

## Verification Steps

1. Replace manual cache steps with rust-cache action
2. Verify YAML syntax
3. Trigger CI run and check logs for "Cache restored" messages
4. Compare build times before/after
5. Verify cache keys are appropriate

## Estimated Complexity

**Low** - Replacement of existing code, 5 minutes

## Dependencies

None - can be done in parallel with other CI tasks

## Notes

- Consider applying to security and coverage jobs if they do significant building
- Monitor first few runs to ensure caching works correctly
- `shared-key: "ci"` allows cache sharing across jobs if needed
