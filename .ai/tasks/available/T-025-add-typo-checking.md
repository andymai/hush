# Task: Add Typo Checking to CI (Optional Enhancement)

## Description

Add automated typo checking using `typos` to catch spelling errors in code, comments, and documentation before they reach production.

## Requirements

- [ ] Add typo check step to CI workflow
- [ ] Use `crate-ci/typos` GitHub Action
- [ ] Configure to check relevant files (rs, md, toml, etc.)
- [ ] Create `.typos.toml` config if needed for false positives
- [ ] Make non-blocking initially

## Success Criteria

- Typo checking runs in CI
- Catches spelling errors
- False positives are minimal
- Configuration is reasonable
- Job completes quickly (<1 min)

## Implementation

Add to `.github/workflows/ci.yml` in the `test` job:

```yaml
      - name: Check for typos
        uses: crate-ci/typos@master
        with:
          files: .
```

If false positives occur, create `.typos.toml`:

```toml
[default]
extend-ignore-re = [
  # Technical terms, abbreviations
  "MSRV",
  "uinput",
]

[files]
extend-exclude = [
  "target/",
  "*.lock",
]
```

## Context

**Current state:** No automated typo checking
**Impact:** Spelling errors in docs, comments, messages
**Priority:** Low - quality of life improvement

## Files to Modify

- `.github/workflows/ci.yml` - Add typo check step

## Files to Create (if needed)

- `.typos.toml` - Configuration for false positives

## Verification Steps

1. Test locally: Install typos with `cargo install typos-cli` and run `typos`
2. Note any false positives
3. Create `.typos.toml` if needed
4. Add step to CI
5. Verify YAML syntax
6. Run in CI and review results

## Estimated Complexity

**Low** - Simple addition, may need minor config tweaking, 10-15 minutes

## Dependencies

None - can be done in parallel with other tasks

## Notes

- Very lightweight check (<1 min runtime)
- Good for catching embarrassing typos in user-facing text
- May catch variable name typos that could be bugs
- Can be made non-blocking with `continue-on-error: true`
- Consider running only on changed files for PRs
