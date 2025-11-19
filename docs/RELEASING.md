# Release Process

This document outlines the release process for Hush, including building and publishing binaries for Linux and macOS.

## Overview

Releases are automated via GitHub Actions and triggered by git tags. The CI/CD pipeline builds binaries for:
- Linux x86_64 (with CUDA support)
- macOS Apple Silicon (arm64, with Metal support)
- macOS Intel (x64, CPU-only)
- macOS Universal binary (arm64 + x64)

## Prerequisites

### For Code Signing (Optional)
1. Apple Developer account ($99/year)
2. Developer ID Application certificate
3. App-specific password for notarization

### GitHub Secrets (for automated releases)
If you want to enable code signing and notarization, add these secrets:
- `MACOS_CERTIFICATE`: Base64-encoded .p12 certificate
- `MACOS_CERTIFICATE_PWD`: Certificate password
- `MACOS_IDENTITY`: Developer ID (e.g., "Developer ID Application: Your Name (TEAM_ID)")
- `MACOS_NOTARIZATION_APPLE_ID`: Apple ID email
- `MACOS_NOTARIZATION_TEAM_ID`: Team ID from Apple Developer
- `MACOS_NOTARIZATION_PWD`: App-specific password

## Release Steps

### 1. Prepare Release

```bash
# Update version in Cargo.toml
vi Cargo.toml
# Change: version = "0.2.0"

# Update CHANGELOG.md
vi CHANGELOG.md
# Add release notes for the new version

# Commit version bump
git add Cargo.toml CHANGELOG.md
git commit -m "chore: Bump version to 0.2.0"
git push origin main
```

### 2. Create Tag

```bash
# Create annotated tag
git tag -a v0.2.0 -m "Release v0.2.0"

# Push tag to trigger CI/CD
git push origin v0.2.0
```

This will trigger the GitHub Actions workflows which will:
1. Run tests on both macOS (Apple Silicon and Intel) and Linux
2. Build release binaries for all platforms
3. Create checksums for verification
4. Create a GitHub Release with all binaries attached
5. Auto-generate release notes from commits

### 3. Monitor CI/CD

1. Go to: https://github.com/andymai/hush/actions
2. Watch the "macOS CI/CD" workflow
3. Ensure all jobs complete successfully

### 4. Verify Release

1. Go to: https://github.com/andymai/hush/releases
2. Check the new release appears
3. Verify all binaries are attached:
   - `hush-macos-arm64.tar.gz` (+ .sha256)
   - `hush-macos-x64.tar.gz` (+ .sha256)
   - `hush-macos-universal.tar.gz` (+ .sha256)
   - `hush-linux-x86_64.tar.gz` (+ .sha256, if Linux CI is set up)

4. Download and test binaries:
   ```bash
   # Test arm64 (on Apple Silicon Mac)
   curl -L https://github.com/andymai/hush/releases/download/v0.2.0/hush-macos-arm64.tar.gz -o hush.tar.gz
   tar xzf hush.tar.gz
   ./hush --version
   ./hush check-permissions

   # Test x64 (on Intel Mac)
   curl -L https://github.com/andymai/hush/releases/download/v0.2.0/hush-macos-x64.tar.gz -o hush.tar.gz
   tar xzf hush.tar.gz
   ./hush --version
   ```

### 5. Post-Release Tasks

- Update README.md badges if needed
- Announce release (Discord, social media, etc.)
- Update documentation links if URLs changed
- Close milestone in GitHub (if using milestones)

## Manual Build (Without CI)

If you need to build manually for local testing:

### macOS

```bash
# Build for current architecture
cargo build --release --features metal  # Apple Silicon
cargo build --release                     # Intel

# Create archive
cd target/release
tar czf hush-macos.tar.gz hush
shasum -a 256 hush-macos.tar.gz > hush-macos.tar.gz.sha256
```

### Linux

```bash
# Build with CUDA
cargo build --release --features cuda

# Create archive
cd target/release
tar czf hush-linux-x86_64.tar.gz hush
sha256sum hush-linux-x86_64.tar.gz > hush-linux-x86_64.tar.gz.sha256
```

## Troubleshooting

### Build fails on Intel runner

- Check `macos-13` runner is available in GitHub Actions
- Verify no Apple Silicon-specific code in critical paths
- Check feature flags are correctly applied

### Notarization fails

- Verify Apple Developer account is active
- Check app-specific password is current
- Ensure entitlements are correct
- Note: Code signing is optional for MVP

### Universal binary creation fails

- Ensure both architectures built successfully
- Check `lipo` command syntax
- Verify binary formats are compatible

### Release not created

- Check GitHub Actions permissions (needs `contents: write`)
- Verify tag format matches pattern (`v*`)
- Check GITHUB_TOKEN has sufficient permissions

## Version Numbering

We follow [Semantic Versioning](https://semver.org/):
- MAJOR version for incompatible API changes
- MINOR version for new functionality (backwards-compatible)
- PATCH version for bug fixes (backwards-compatible)

Examples:
- `v0.1.0` - Initial release
- `v0.2.0` - Added macOS support (new functionality)
- `v0.2.1` - Bug fixes
- `v1.0.0` - First stable release

## Hotfix Releases

For critical bugs:

```bash
# Create hotfix branch from tag
git checkout -b hotfix/v0.2.1 v0.2.0

# Fix the bug
# ...commit changes...

# Update version
vi Cargo.toml  # Change to 0.2.1

# Tag and push
git tag -a v0.2.1 -m "Hotfix: Critical bug fix"
git push origin v0.2.1

# Merge back to main
git checkout main
git merge hotfix/v0.2.1
git push origin main
```

## Beta/Pre-releases

For testing releases:

```bash
# Tag as pre-release
git tag -a v0.3.0-beta.1 -m "Beta release 0.3.0-beta.1"
git push origin v0.3.0-beta.1
```

GitHub will automatically mark tags with pre-release identifiers as pre-releases.

## Cost Considerations

### GitHub Actions Minutes

- Free tier: Unlimited for public repos
- macOS runners: 10x multiplier (1 minute = 10 minutes charged on private repos)
- Estimated per release: ~30 minutes = 300 minutes charged (private repos only)

**Recommendation:** Use public repo to avoid costs, or budget ~$20/month for private repo CI.

### Apple Developer Account

- Required for: Code signing, notarization
- Cost: $99/year
- Optional for: MVP (unsigned binaries work, just show warning)

**Recommendation:** Skip for MVP, add later for production releases.

## Further Reading

- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [macOS Runners](https://github.com/actions/runner-images/blob/main/images/macos/macos-14-Readme.md)
- [Code Signing](https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution)
- [Universal Binaries](https://developer.apple.com/documentation/apple-silicon/building-a-universal-macos-binary)
- [Semantic Versioning](https://semver.org/)
