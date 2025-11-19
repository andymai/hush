# Task: Add Feature Flag Matrix Testing

## Description

Add a CI job that tests different feature flag combinations to ensure the crate builds correctly with optional features enabled/disabled.

## Requirements

- [ ] Add new `test-features` job to CI workflow
- [ ] Create test matrix for feature combinations
- [ ] Test with no default features
- [ ] Test each optional feature individually (`notifications`, `system-tray`)
- [ ] Verify all combinations build and pass tests

## Success Criteria

- New job runs in parallel with other CI jobs
- All feature combinations build successfully
- Tests pass for each combination
- Matrix is efficient (no redundant combinations)
- Job completes in reasonable time

## Implementation

Add to `.github/workflows/ci.yml` after the `test` job:

```yaml
  test-features:
    name: Test Feature Combinations
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        features:
          - ""  # Minimal build - no default features
          - "notifications"
          - "system-tray"
          - "notifications,system-tray"  # Same as default
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y libasound2-dev pkg-config libx11-dev libdbus-1-dev

      - uses: Swatinem/rust-cache@v2
        with:
          shared-key: "features-${{ matrix.features }}"

      - name: Build with features
        run: |
          if [ -z "${{ matrix.features }}" ]; then
            cargo build --no-default-features --verbose
          else
            cargo build --no-default-features --features "${{ matrix.features }}" --verbose
          fi

      - name: Test with features
        run: |
          if [ -z "${{ matrix.features }}" ]; then
            cargo test --no-default-features --verbose
          else
            cargo test --no-default-features --features "${{ matrix.features }}" --verbose
          fi
```

## Context

**Current state:** Only default features tested in CI
**Impact:** Minimal builds or feature combinations may break without detection
**Priority:** Medium - ensures flexibility for users who don't want all features

## Files to Modify

- `.github/workflows/ci.yml` - Add new job

## Verification Steps

1. Check current feature flags in Cargo.toml (default, notifications, system-tray)
2. Add the matrix job to CI
3. Verify YAML syntax
4. Test locally: `cargo build --no-default-features` and with each feature
5. Trigger CI and confirm all matrix combinations pass
6. Monitor job run time (should be <10 min total with caching)

## Estimated Complexity

**Medium** - Matrix configuration requires testing, 20 minutes

## Dependencies

- Benefits from T-023 (rust-cache) for faster runs
- Otherwise will work but be slower

## Notes

- `fail-fast: false` ensures all combinations are tested even if one fails
- Empty string "" in matrix means no features (--no-default-features only)
- Cache keys per feature set to avoid cache conflicts
- Consider adding `--all-features` test for comprehensive coverage
- If job becomes too slow, can reduce matrix or run less frequently
- Currently: default = ["notifications", "system-tray"] per Cargo.toml
