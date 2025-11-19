# CI/CD Patterns for Hush

**Last Updated:** 2025-11-19
**Status:** Reference for AI agents working on CI/CD

This document outlines CI/CD patterns, conventions, and best practices learned from building and maintaining the Hush GitHub Actions workflows.

---

## Overview

Hush uses GitHub Actions for continuous integration with a focus on:
- **Fast feedback** - Parallel jobs, effective caching
- **Comprehensive testing** - Multiple Rust versions, feature combinations
- **Security** - Automated vulnerability scanning
- **Quality** - Linting, formatting, compilation checks

---

## GitHub Actions Workflow Structure

### Standard Job Pattern

Every CI job follows this pattern:

```yaml
job-name:
  name: Human Readable Name
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4

    - name: Install Rust
      uses: dtolnay/rust-toolchain@stable  # or @1.70.0, @nightly

    - name: Install system dependencies
      run: |
        sudo apt-get update
        sudo apt-get install -y libasound2-dev pkg-config libx11-dev libdbus-1-dev

    - uses: Swatinem/rust-cache@v2
      with:
        shared-key: "job-specific-key"

    - name: Run checks
      run: cargo check  # or test, clippy, etc.
```

**Key components:**
1. **Checkout** - Always `actions/checkout@v4` (latest)
2. **Rust toolchain** - Use `dtolnay/rust-toolchain@{version}` for pinned versions
3. **System dependencies** - Install before Rust operations
4. **Caching** - `Swatinem/rust-cache@v2` for dependency caching
5. **Main action** - The actual check/test/build

---

## Caching Strategies

### Rust Dependency Caching

**Always use `Swatinem/rust-cache@v2`:**

```yaml
- uses: Swatinem/rust-cache@v2
  with:
    shared-key: "rust-stable"  # Shared across jobs with same toolchain
```

**Why?**
- Caches `target/` directory and Cargo registry
- Automatically handles cache invalidation
- Significantly faster than manual caching (~5-10x speedup)
- Handles lockfile changes automatically

### Cache Key Patterns

Different jobs need different cache keys:

```yaml
# Standard job (stable Rust)
shared-key: "rust-stable"

# MSRV job (different toolchain)
shared-key: "msrv"

# Feature matrix (different feature combinations)
shared-key: "features-${{ matrix.features }}"

# Security job (may have different dependencies)
shared-key: "security"
```

**Rule:** Use same cache key for jobs with identical:
- Rust toolchain version
- Feature flags
- Dependencies (Cargo.lock)

---

## Core CI Jobs

### 1. Fast Compilation Check

**Purpose:** Fastest possible feedback on compilation errors

```yaml
check:
  name: Check Compilation
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
    - name: Install system dependencies
      run: |
        sudo apt-get update
        sudo apt-get install -y libasound2-dev pkg-config libx11-dev libdbus-1-dev
    - uses: Swatinem/rust-cache@v2
    - run: cargo check --all-features
```

**Why first?** Catches 80% of issues in ~2 minutes

### 2. Test Suite

**Purpose:** Run all tests to verify functionality

```yaml
test:
  name: Run Tests
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
    - name: Install system dependencies
      run: |
        sudo apt-get update
        sudo apt-get install -y libasound2-dev pkg-config libx11-dev libdbus-1-dev
    - uses: Swatinem/rust-cache@v2
    - run: cargo test --verbose
```

### 3. Linting (Clippy)

**Purpose:** Enforce code quality standards

```yaml
clippy:
  name: Clippy Lints
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
      with:
        components: clippy
    - name: Install system dependencies
      run: |
        sudo apt-get update
        sudo apt-get install -y libasound2-dev pkg-config libx11-dev libdbus-1-dev
    - uses: Swatinem/rust-cache@v2
    - run: cargo clippy --all-features -- -D warnings
```

**Note:** `-D warnings` treats warnings as errors

### 4. Format Check

**Purpose:** Enforce consistent code formatting

```yaml
fmt:
  name: Check Formatting
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
      with:
        components: rustfmt
    - run: cargo fmt --all -- --check
```

**Note:** No system dependencies or caching needed (fast)

### 5. Security Audit

**Purpose:** Check for known vulnerabilities in dependencies

```yaml
security:
  name: Security Audit
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
    - uses: Swatinem/rust-cache@v2
      with:
        shared-key: "security"
    - run: cargo install cargo-audit
    - run: cargo audit
```

**Alternative with action:**

```yaml
security:
  name: Security Audit
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: rustsec/audit-check@v1.4.1
      with:
        token: ${{ secrets.GITHUB_TOKEN }}
```

---

## Advanced CI Patterns

### MSRV (Minimum Supported Rust Version) Testing

**Purpose:** Ensure project builds on claimed minimum Rust version

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

**Best practices:**
1. Use `cargo check` (faster than full build)
2. Match version in README badge
3. Add `rust-version = "1.70"` to Cargo.toml after verification
4. Run on every PR to catch MSRV regressions

### Feature Flag Matrix Testing

**Purpose:** Test all feature combinations build correctly

```yaml
test-features:
  name: Test Feature Combinations
  runs-on: ubuntu-latest
  strategy:
    fail-fast: false
    matrix:
      features:
        - ""                          # Minimal (no features)
        - "notifications"             # Individual feature
        - "system-tray"               # Individual feature
        - "notifications,system-tray" # All features (default)
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

**Key patterns:**
- `fail-fast: false` - Test all combinations even if one fails
- Empty string `""` - Represents no features (--no-default-features only)
- Cache key per feature combination
- Conditional shell logic for empty features

### Multi-Platform Testing

**Purpose:** Ensure cross-platform compatibility

```yaml
test-platforms:
  name: Test on ${{ matrix.os }}
  runs-on: ${{ matrix.os }}
  strategy:
    matrix:
      os: [ubuntu-latest, ubuntu-20.04, macos-latest, windows-latest]
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable

    # Platform-specific dependency installation
    - name: Install Linux dependencies
      if: runner.os == 'Linux'
      run: |
        sudo apt-get update
        sudo apt-get install -y libasound2-dev pkg-config libx11-dev libdbus-1-dev

    - uses: Swatinem/rust-cache@v2
    - run: cargo test --verbose
```

**Note:** Hush is currently Linux-only, but pattern shown for reference

---

## Workflow Optimization Techniques

### Job Dependencies and Parallelization

**Run jobs in parallel when independent:**

```yaml
jobs:
  check:
    # Runs immediately

  test:
    # Runs in parallel with check

  clippy:
    # Runs in parallel with check and test

  fmt:
    # Runs in parallel (fastest, no dependencies)

  deploy:
    needs: [check, test, clippy, fmt]  # Wait for all to pass
    # Only runs if all above succeed
```

**Rule:** Use `needs:` only when jobs have actual dependencies

### Conditional Job Execution

**Run expensive jobs only on main branch:**

```yaml
expensive-job:
  if: github.ref == 'refs/heads/main'
  runs-on: ubuntu-latest
  steps:
    # ...
```

**Run security checks on schedule:**

```yaml
on:
  schedule:
    - cron: '0 0 * * 0'  # Weekly on Sunday
  push:
    branches: [main]
```

### Artifact Sharing Between Jobs

**Build once, test multiple times:**

```yaml
build:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - run: cargo build --release
    - uses: actions/upload-artifact@v3
      with:
        name: hush-binary
        path: target/release/hush

test-binary:
  needs: build
  runs-on: ubuntu-latest
  steps:
    - uses: actions/download-artifact@v3
      with:
        name: hush-binary
    - run: ./hush --version
```

---

## System Dependencies Installation

### Ubuntu/Debian Pattern

```yaml
- name: Install system dependencies
  run: |
    sudo apt-get update
    sudo apt-get install -y \
      libasound2-dev \
      pkg-config \
      libx11-dev \
      libdbus-1-dev
```

### Caching APT Packages (Optional)

```yaml
- name: Cache APT packages
  uses: awalsh128/cache-apt-pkgs-action@latest
  with:
    packages: libasound2-dev pkg-config libx11-dev libdbus-1-dev
    version: 1.0

- name: Install system dependencies
  run: |
    sudo apt-get update
    sudo apt-get install -y \
      libasound2-dev \
      pkg-config \
      libx11-dev \
      libdbus-1-dev
```

**Note:** Usually not needed with fast runners

---

## Error Handling and Debugging

### Continue on Error

```yaml
- name: Optional check
  continue-on-error: true
  run: cargo bench
```

### Debugging Failed Jobs

```yaml
# Add to workflow for debugging
- name: Setup tmate session
  if: ${{ failure() }}
  uses: mxschmitt/action-tmate@v3
  timeout-minutes: 15
```

### Verbose Output

```yaml
- run: cargo test --verbose -- --nocapture
```

---

## Common Patterns and Anti-Patterns

### ✅ DO

**Use latest action versions:**
```yaml
- uses: actions/checkout@v4  # Not @v3
- uses: Swatinem/rust-cache@v2  # Not @v1
```

**Pin Rust versions explicitly for MSRV:**
```yaml
- uses: dtolnay/rust-toolchain@1.70.0  # Explicit version
```

**Use fail-fast: false for exploratory matrices:**
```yaml
strategy:
  fail-fast: false  # See all failures
  matrix: ...
```

**Cache appropriately:**
```yaml
- uses: Swatinem/rust-cache@v2
  with:
    shared-key: "unique-per-job"
```

### ❌ DON'T

**Don't use outdated actions:**
```yaml
- uses: actions/checkout@v2  # Bad - outdated
```

**Don't install unnecessary dependencies:**
```yaml
# Bad - fmt doesn't need system libs
fmt:
  steps:
    - run: sudo apt-get install libasound2-dev  # Unnecessary
    - run: cargo fmt --check
```

**Don't skip caching without reason:**
```yaml
# Bad - wastes time on every run
- run: cargo build  # No caching = slow
```

**Don't use wildcards in toolchain versions:**
```yaml
- uses: dtolnay/rust-toolchain@1  # Bad - unpredictable
```

---

## Troubleshooting CI Failures

### Common Issues and Solutions

| Issue | Cause | Solution |
|-------|-------|----------|
| `cargo: command not found` | Rust not installed | Add `dtolnay/rust-toolchain` step |
| `libdbus-sys build failed` | Missing system dependency | Install `libdbus-1-dev` |
| `error: linker 'cc' not found` | Missing build-essential | Install `build-essential` or `gcc` |
| Cache miss on every run | Different cache keys | Use consistent `shared-key` |
| Job times out | Build too slow | Add caching, use `cargo check` |
| MSRV job fails | Code uses newer Rust features | Fix code or update MSRV claim |

### Verification Commands

Before adding CI jobs, test locally:

```bash
# Simulate fresh CI environment
docker run --rm -it rust:1.70 bash

# Inside container:
apt-get update
apt-get install -y libasound2-dev pkg-config libx11-dev libdbus-1-dev
git clone <your-repo>
cd <repo>
cargo check --all-features
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

---

## Security Best Practices

### Secret Management

```yaml
# Never log secrets
- run: echo "Token: ${{ secrets.API_TOKEN }}"  # BAD

# Use environment variables
- env:
    API_TOKEN: ${{ secrets.API_TOKEN }}
  run: ./deploy.sh  # Good - script uses env var
```

### Dependency Pinning

```yaml
# Pin action versions for security
- uses: actions/checkout@v4  # Good - specific version
- uses: actions/checkout@main  # Bad - unpredictable
```

### Minimal Permissions

```yaml
permissions:
  contents: read  # Minimal permissions

jobs:
  test:
    runs-on: ubuntu-latest
    # Inherits minimal permissions
```

---

## Complete Example Workflow

See `.github/workflows/ci.yml` for the complete Hush CI workflow incorporating all these patterns.

**Key jobs:**
1. `check` - Fast compilation check
2. `test` - Full test suite
3. `clippy` - Linting
4. `fmt` - Format check
5. `security` - Vulnerability scanning
6. `msrv` - Minimum Rust version check
7. `test-features` - Feature flag matrix

**Total runtime:** ~5-8 minutes with caching

---

## Quick Reference

### Adding a New CI Job

```bash
# 1. Test locally first
cargo <command>

# 2. Add to .github/workflows/ci.yml
# 3. Use standard job structure (see above)
# 4. Add appropriate caching
# 5. Install only necessary system dependencies
# 6. Verify YAML syntax
# 7. Push and monitor CI run
```

### Finding CI Configuration

```bash
# Find workflow files
fd ".yml" .github/workflows/

# Check for Rust toolchain usage
rg "rust-toolchain" .github/workflows/

# Check for caching
rg "rust-cache" .github/workflows/

# Check for system dependencies
rg "apt-get install" .github/workflows/
```

---

## Related Documentation

- `.github/workflows/ci.yml` - Main CI workflow
- `CLAUDE.md` - Task execution protocol (includes git/PR creation)
- `.ai/knowledge/build-system.md` - Build dependencies and setup
- `.ai/knowledge/conventions.md` - General coding conventions

---

**Remember:** CI should be fast, reliable, and informative. Every job should have a clear purpose and provide actionable feedback. When in doubt, test locally first, then add to CI.
