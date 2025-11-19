# Task: Add cargo-deny Integration (Optional Enhancement)

## Description

Add `cargo-deny` to CI to check for license compliance, security advisories, and dependency issues automatically.

## Requirements

- [ ] Add cargo-deny check to CI workflow
- [ ] Use `EmbarkStudios/cargo-deny-action`
- [ ] Create `.cargo-deny.toml` configuration
- [ ] Configure license allowlist/denylist
- [ ] Enable security advisory checking
- [ ] Configure dependency graph checks

## Success Criteria

- cargo-deny runs in CI
- License compliance is verified
- Security advisories are checked
- Duplicate dependencies are flagged
- Configuration matches project needs
- Job completes in reasonable time

## Implementation

Add to `.github/workflows/ci.yml` as a new job or in security job:

```yaml
  deny:
    name: License and Security Check
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: EmbarkStudios/cargo-deny-action@v1
        with:
          log-level: warn
          command: check
          arguments: --all-features
```

Create `.cargo-deny.toml`:

```toml
[advisories]
version = 2
db-path = "~/.cargo/advisory-db"
db-urls = ["https://github.com/rustsec/advisory-db"]
ignore = []

[licenses]
version = 2
allow = [
  "MIT",
  "Apache-2.0",
  "Apache-2.0 WITH LLVM-exception",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "ISC",
  "Unicode-DFS-2016",
]
confidence-threshold = 0.8

[bans]
multiple-versions = "warn"
wildcards = "allow"
highlight = "all"

[sources]
unknown-registry = "warn"
unknown-git = "warn"
```

## Context

**Current state:** Only cargo-audit for security
**Impact:** License compliance, duplicate dependencies not checked
**Priority:** Low - additional safety net

## Files to Modify

- `.github/workflows/ci.yml` - Add cargo-deny job

## Files to Create

- `.cargo-deny.toml` - Configuration

## Verification Steps

1. Install cargo-deny: `cargo install cargo-deny`
2. Run locally: `cargo deny check`
3. Review output for any issues
4. Adjust `.cargo-deny.toml` configuration
5. Add to CI workflow
6. Verify YAML syntax
7. Run in CI and confirm passes

## Estimated Complexity

**Medium** - Configuration needs customization for project, 20-30 minutes

## Dependencies

None - can be done in parallel with other tasks

## Notes

- Overlaps with cargo-audit (both check advisories)
- Main value: license checking and duplicate detection
- May need to adjust allowed licenses based on dependencies
- Can start with warnings only, then make stricter
- Consider excluding this from PR CI (run only on main) to save time
- Duplicate dependencies can increase binary size
