# Task: Add Documentation Checks to CI

## Description

Add `cargo doc` checks to CI to catch documentation warnings, broken links, and missing docs before they reach production.

## Requirements

- [ ] Add documentation check step to test job in CI
- [ ] Use `RUSTDOCFLAGS="-D warnings"` to fail on warnings
- [ ] Check all features with `--all-features`
- [ ] Include private items with `--document-private-items` (optional but recommended)
- [ ] Use `--no-deps` to avoid checking dependencies

## Success Criteria

- Documentation builds without warnings
- Step added to CI workflow
- Broken doc links are caught
- Missing doc comments are flagged (if using `-D missing_docs`)
- Job completes in reasonable time

## Implementation

Add to `.github/workflows/ci.yml` in the `test` job, after the "Run clippy" step:

```yaml
      - name: Check documentation
        run: cargo doc --no-deps --all-features --document-private-items
        env:
          RUSTDOCFLAGS: "-D warnings"
```

Optional stricter version (may require many doc additions):

```yaml
      - name: Check documentation
        run: cargo doc --no-deps --all-features --document-private-items
        env:
          RUSTDOCFLAGS: "-D warnings -D missing_docs"
```

## Context

**Current state:** No documentation validation in CI
**Impact:** Broken links, doc warnings can slip through
**Priority:** Medium - improves documentation quality

## Files to Modify

- `.github/workflows/ci.yml` - Add step to test job (after clippy, before build)

## Verification Steps

1. Test locally: `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --document-private-items`
2. Fix any existing warnings if needed
3. Add the step to CI
4. Verify YAML syntax
5. Trigger CI and confirm docs build cleanly

## Estimated Complexity

**Low to Medium** - Simple addition, but may reveal existing doc warnings that need fixing, 5-30 minutes

## Dependencies

None - can be done in parallel with other CI tasks

## Notes

- Start with just `-D warnings`, add `-D missing_docs` later if desired
- `--document-private-items` is good for internal documentation
- Consider adding `--no-deps` to avoid checking dependency docs
- This catches broken intra-doc links with `[SomeType]` syntax
- May want to exclude this check from some jobs to save time
