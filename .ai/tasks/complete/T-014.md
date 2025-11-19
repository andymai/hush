# Task: Enforce Clippy Warnings in CI

## Description

Add `-D warnings` flag to clippy in CI workflow to fail builds on any warnings, preventing code quality issues from being merged.

## Requirements

- [ ] Update `.github/workflows/ci.yml` clippy step (line ~51-52)
- [ ] Change from `cargo clippy --all-targets --all-features` to include `-- -D warnings`
- [ ] Verify the change doesn't break existing CI (check current clippy output first)
- [ ] Test locally: `cargo clippy --all-targets --all-features -- -D warnings`

## Success Criteria

- `cargo clippy --all-targets --all-features -- -D warnings` passes locally
- CI workflow updated correctly
- Workflow syntax is valid (check with `yamllint` or GitHub Actions validator)
- Commit message follows project conventions

## Context

**Current state:** Clippy runs but warnings don't fail the build (line 51-52 in ci.yml)
**Impact:** Code quality issues can slip through PR reviews
**Priority:** Critical - this is a code quality gate

## Files to Modify

- `.github/workflows/ci.yml` - Line ~51-52

## Verification Steps

1. Check current clippy status: `cargo clippy --all-targets --all-features`
2. If warnings exist, fix them first OR document them in the PR
3. Update the CI file
4. Verify YAML syntax is valid
5. Commit and push to trigger CI

## Estimated Complexity

**Low** - Single line change, 2 minutes

## Dependencies

None - can be done in parallel with other CI tasks
