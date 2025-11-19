# Task: Add Benchmark Tracking to CI (Optional Enhancement)

## Description

Add criterion benchmark execution to CI to track performance over time and catch performance regressions. This is optional but valuable for a performance-focused application like Hush.

## Requirements

- [ ] Add benchmark job to CI workflow
- [ ] Run `cargo bench` on criterion benchmarks
- [ ] Consider adding historical tracking (optional)
- [ ] Ensure benchmarks complete in reasonable time
- [ ] Make job optional/non-blocking initially

## Success Criteria

- Benchmarks run successfully in CI
- Results are captured/logged
- Job doesn't significantly slow down CI
- Can catch obvious performance regressions
- Optional: Historical comparison available

## Implementation

Add to `.github/workflows/ci.yml`:

```yaml
  benchmarks:
    name: Run Benchmarks
    runs-on: ubuntu-latest
    # Make optional - don't block merges
    continue-on-error: true
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
          shared-key: "bench"

      - name: Run benchmarks
        run: cargo bench --no-fail-fast -- --output-format bencher | tee bench-output.txt

      - name: Store benchmark result
        uses: benchmark-action/github-action-benchmark@v1
        if: github.event_name != 'pull_request'
        with:
          tool: 'cargo'
          output-file-path: bench-output.txt
          github-token: ${{ secrets.GITHUB_TOKEN }}
          auto-push: true
```

## Context

**Current state:** Criterion benchmarks defined but not run in CI
**Impact:** Performance regressions undetected
**Priority:** Low - nice to have, not critical

## Files to Modify

- `.github/workflows/ci.yml` - Add benchmark job

## Verification Steps

1. Test locally: `cargo bench`
2. Ensure benchmarks complete in <5 minutes
3. Add job to CI workflow
4. Verify YAML syntax
5. Run in CI and review results
6. Consider adding historical tracking later

## Estimated Complexity

**Medium** - Benchmark setup plus optional historical tracking, 30-45 minutes

## Dependencies

- Benefits from T-017 (rust-cache) for faster runs
- `benchmark-action/github-action-benchmark` is optional enhancement

## Notes

- `continue-on-error: true` prevents benchmark failures from blocking PRs
- Benchmarks in CI may be noisy due to shared runners
- Consider running only on main branch to reduce CI load
- For accurate benchmarks, dedicated hardware is better
- Can skip this if benchmarks are only run manually
- Current benchmarks: `benches/architecture_benchmarks.rs`
