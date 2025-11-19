# Task: Add Workflow Concurrency Control

## Description

Add concurrency control to CI workflow to automatically cancel outdated CI runs when new commits are pushed to the same PR/branch. This saves CI resources and provides faster feedback.

## Requirements

- [ ] Add `concurrency` section to `.github/workflows/ci.yml`
- [ ] Use branch/PR reference for grouping
- [ ] Enable `cancel-in-progress: true`
- [ ] Verify it doesn't affect main branch CI runs negatively

## Success Criteria

- Concurrency configuration is correct
- Pushing multiple commits to a PR cancels older runs
- Main branch runs are not cancelled inappropriately
- YAML syntax is valid

## Implementation

Add to `.github/workflows/ci.yml` after the `env:` section (after line ~11):

```yaml
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true
```

## Context

**Current state:** Multiple commits to PR trigger overlapping CI runs
**Impact:** Wasted CI minutes, slower feedback on latest commit
**Priority:** High - efficiency improvement, especially on active PRs

## Files to Modify

- `.github/workflows/ci.yml` - Add after line ~11

## Verification Steps

1. Add concurrency section after env block
2. Verify YAML syntax
3. Test by pushing 2 commits quickly to a test PR
4. Confirm first CI run gets cancelled
5. Verify main branch CI runs complete normally

## Estimated Complexity

**Low** - 3 lines added, 2 minutes

## Dependencies

None - can be done in parallel with other CI tasks
