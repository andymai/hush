# Task: Add MSRV Testing to CI

## Description

Add Minimum Supported Rust Version (MSRV) testing to CI to validate the "Rust 1.70+" claim in README and ensure users on older toolchains can build the project.

## Requirements

- [ ] Add new `msrv` job to `.github/workflows/ci.yml`
- [ ] Test against Rust 1.70.0 (as claimed in README badge)
- [ ] Include system dependencies installation
- [ ] Use `cargo check --all-features` (faster than full build)
- [ ] Verify locally first with `rustup install 1.70.0`

## Success Criteria

- New job runs successfully in CI
- Rust 1.70.0 can compile the project
- Job runs in parallel with other CI jobs
- If MSRV fails, either fix code OR update README badge
- YAML syntax is valid

## Implementation

Add to `.github/workflows/ci.yml` after the `test` job:

```yaml
  msrv:
    name: Check MSRV (1.70)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust 1.70
        uses: dtolnay/rust-toolchain@1.70.0

      - name: Install system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y libasound2-dev pkg-config libx11-dev libdbus-1-dev

      - uses: Swatinem/rust-cache@v2
        with:
          shared-key: "msrv"

      - name: Check compilation
        run: cargo check --all-features
```

## Context

**Current state:** README claims "Rust 1.70+" but this isn't verified
**Impact:** Users on 1.70 may face build failures, unclear MSRV
**Priority:** High - verifies advertised compatibility

## Files to Modify

- `.github/workflows/ci.yml` - Add new job after test job

## Verification Steps

1. Test locally first: `rustup install 1.70.0 && rustup run 1.70.0 cargo check --all-features`
2. If it fails, determine actual MSRV or fix code
3. Add the job to CI workflow
4. Verify YAML syntax
5. Trigger CI run and confirm job passes
6. Update README badge if MSRV changes

## Estimated Complexity

**Medium** - May require MSRV adjustment or code fixes, 20 minutes

## Dependencies

- Should use `Swatinem/rust-cache@v2` if T-023 is completed
- Otherwise, can use manual caching or no caching

## Notes

- `cargo check` is sufficient for MSRV validation (faster than full build)
- If 1.70 fails, use `cargo-msrv` tool to find actual MSRV
- Common reasons for MSRV failures: newer syntax, dependency MSRV requirements
- Consider adding `rust-version = "1.70"` to Cargo.toml after verification
