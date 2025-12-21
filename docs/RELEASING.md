# Release Process

## Overview

Releases are automated via GitHub Actions and triggered by git tags.

## Release Steps

### 1. Prepare Release

```bash
# Update version in Cargo.toml
vi Cargo.toml

# Commit version bump
git add Cargo.toml
git commit -m "chore: Bump version to 0.2.0"
git push origin main
```

### 2. Create Tag

```bash
git tag -a v0.2.0 -m "Release v0.2.0"
git push origin v0.2.0
```

### 3. Manual Build

```bash
# Build with CUDA
cargo build --release --features cuda

# Create archive
cd target/release
tar czf hush-linux-x86_64.tar.gz hush
sha256sum hush-linux-x86_64.tar.gz > hush-linux-x86_64.tar.gz.sha256
```

## Version Numbering

We follow [Semantic Versioning](https://semver.org/):
- MAJOR version for incompatible API changes
- MINOR version for new functionality
- PATCH version for bug fixes
