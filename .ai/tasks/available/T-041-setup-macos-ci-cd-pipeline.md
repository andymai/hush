# T-041: Set Up macOS CI/CD Pipeline with Release Builds

**Priority:** Low-Medium
**Effort:** Medium (4-6 hours)
**Type:** Infrastructure - CI/CD
**Status:** Available
**Created:** 2025-11-19

---

## Problem Statement

Need automated CI/CD pipeline for macOS builds to:
- Run tests on every commit
- Build release binaries for Apple Silicon and Intel
- Create code-signed, notarized releases
- Publish releases to GitHub

Currently, macOS builds are manual and untested in CI.

---

## Goals

1. ✅ Set up GitHub Actions for macOS builds
2. ✅ Test on both Apple Silicon and Intel runners
3. ✅ Build release binaries for both architectures
4. ✅ Create universal binaries (optional)
5. ✅ Code sign and notarize releases (optional for MVP)
6. ✅ Publish releases to GitHub Releases
7. ✅ Run tests on every PR

---

## Implementation Steps

### 1. Enhanced GitHub Actions Workflow

**Update: `.github/workflows/macos-ci.yml`**

```yaml
name: macOS CI/CD

on:
  push:
    branches: [ main, develop, 'claude/**' ]
    tags: [ 'v*' ]
  pull_request:
    branches: [ main, develop ]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  # Test on macOS
  test:
    name: Test (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [macos-14, macos-13]  # macos-14 = M1, macos-13 = Intel
        rust: [stable]

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: ${{ matrix.rust }}

      - name: Cache cargo registry
        uses: actions/cache@v3
        with:
          path: ~/.cargo/registry
          key: ${{ runner.os }}-cargo-registry-${{ hashFiles('**/Cargo.lock') }}

      - name: Cache cargo index
        uses: actions/cache@v3
        with:
          path: ~/.cargo/git
          key: ${{ runner.os }}-cargo-index-${{ hashFiles('**/Cargo.lock') }}

      - name: Cache cargo build
        uses: actions/cache@v3
        with:
          path: target
          key: ${{ runner.os }}-cargo-build-${{ hashFiles('**/Cargo.lock') }}

      - name: Check Rust version and target
        run: |
          rustc --version
          cargo --version
          rustup show
          uname -m

      - name: Run cargo check
        run: cargo check --workspace --all-targets

      - name: Run cargo test (lib)
        run: cargo test --lib --no-default-features

      - name: Run cargo test (integration)
        run: cargo test --test macos_integration --no-default-features
        continue-on-error: true  # May fail without GUI/permissions

      - name: Run cargo clippy
        run: cargo clippy --all-targets -- -D warnings

      - name: Check formatting
        run: cargo fmt -- --check

      - name: Test device detection
        run: |
          cargo build --release --no-default-features
          ./target/release/hush device-info || true

  # Build release binaries
  build:
    name: Build Release (${{ matrix.target }})
    runs-on: ${{ matrix.os }}
    needs: test
    if: startsWith(github.ref, 'refs/tags/v')
    strategy:
      matrix:
        include:
          - os: macos-14
            target: aarch64-apple-darwin
            arch: arm64
            features: metal
          - os: macos-13
            target: x86_64-apple-darwin
            arch: x64
            features: ""

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Cache cargo build
        uses: actions/cache@v3
        with:
          path: target
          key: ${{ runner.os }}-${{ matrix.target }}-cargo-build-release

      - name: Build release binary
        run: |
          if [ "${{ matrix.features }}" != "" ]; then
            cargo build --release --target ${{ matrix.target }} --features ${{ matrix.features }}
          else
            cargo build --release --target ${{ matrix.target }} --no-default-features
          fi

      - name: Strip binary
        run: |
          strip target/${{ matrix.target }}/release/hush

      - name: Create archive
        run: |
          cd target/${{ matrix.target }}/release
          tar czf hush-macos-${{ matrix.arch }}.tar.gz hush
          mv hush-macos-${{ matrix.arch }}.tar.gz ../../../

      - name: Upload artifact
        uses: actions/upload-artifact@v3
        with:
          name: hush-macos-${{ matrix.arch }}
          path: hush-macos-${{ matrix.arch }}.tar.gz

  # Create universal binary (optional)
  universal:
    name: Create Universal Binary
    runs-on: macos-14
    needs: build
    if: startsWith(github.ref, 'refs/tags/v')

    steps:
      - uses: actions/checkout@v4

      - name: Download arm64 binary
        uses: actions/download-artifact@v3
        with:
          name: hush-macos-arm64

      - name: Download x64 binary
        uses: actions/download-artifact@v3
        with:
          name: hush-macos-x64

      - name: Extract binaries
        run: |
          mkdir -p arm64 x64
          tar xzf hush-macos-arm64.tar.gz -C arm64
          tar xzf hush-macos-x64.tar.gz -C x64

      - name: Create universal binary
        run: |
          lipo -create -output hush arm64/hush x64/hush
          chmod +x hush

      - name: Verify universal binary
        run: |
          file hush
          lipo -info hush

      - name: Create universal archive
        run: |
          tar czf hush-macos-universal.tar.gz hush

      - name: Upload universal binary
        uses: actions/upload-artifact@v3
        with:
          name: hush-macos-universal
          path: hush-macos-universal.tar.gz

  # Create GitHub Release
  release:
    name: Create Release
    runs-on: macos-14
    needs: [build, universal]
    if: startsWith(github.ref, 'refs/tags/v')

    steps:
      - uses: actions/checkout@v4

      - name: Download all artifacts
        uses: actions/download-artifact@v3
        with:
          path: artifacts

      - name: Create checksums
        run: |
          cd artifacts
          for dir in */; do
            cd "$dir"
            for file in *.tar.gz; do
              shasum -a 256 "$file" > "$file.sha256"
            done
            cd ..
          done

      - name: Create Release
        uses: softprops/action-gh-release@v1
        with:
          files: |
            artifacts/hush-macos-arm64/*.tar.gz
            artifacts/hush-macos-arm64/*.sha256
            artifacts/hush-macos-x64/*.tar.gz
            artifacts/hush-macos-x64/*.sha256
            artifacts/hush-macos-universal/*.tar.gz
            artifacts/hush-macos-universal/*.sha256
          draft: false
          prerelease: false
          generate_release_notes: true
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

  # Code signing and notarization (optional, requires Apple Developer account)
  sign-and-notarize:
    name: Sign and Notarize
    runs-on: macos-14
    needs: build
    if: startsWith(github.ref, 'refs/tags/v') && false  # Disabled by default
    # Enable by removing: && false

    steps:
      - uses: actions/checkout@v4

      - name: Download binaries
        uses: actions/download-artifact@v3

      - name: Import signing certificate
        env:
          MACOS_CERTIFICATE: ${{ secrets.MACOS_CERTIFICATE }}
          MACOS_CERTIFICATE_PWD: ${{ secrets.MACOS_CERTIFICATE_PWD }}
        run: |
          # Import certificate to keychain
          echo $MACOS_CERTIFICATE | base64 --decode > certificate.p12
          security create-keychain -p actions temp.keychain
          security default-keychain -s temp.keychain
          security unlock-keychain -p actions temp.keychain
          security import certificate.p12 -k temp.keychain -P $MACOS_CERTIFICATE_PWD -T /usr/bin/codesign
          security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k actions temp.keychain

      - name: Sign binary
        env:
          MACOS_IDENTITY: ${{ secrets.MACOS_IDENTITY }}
        run: |
          # Sign each binary
          codesign --force --deep --sign "$MACOS_IDENTITY" \
            --options runtime \
            --entitlements entitlements.plist \
            hush-macos-arm64/hush

          # Verify signature
          codesign --verify --verbose hush-macos-arm64/hush

      - name: Notarize binary
        env:
          MACOS_NOTARIZATION_APPLE_ID: ${{ secrets.MACOS_NOTARIZATION_APPLE_ID }}
          MACOS_NOTARIZATION_TEAM_ID: ${{ secrets.MACOS_NOTARIZATION_TEAM_ID }}
          MACOS_NOTARIZATION_PWD: ${{ secrets.MACOS_NOTARIZATION_PWD }}
        run: |
          # Create zip for notarization
          ditto -c -k --keepParent hush-macos-arm64/hush hush.zip

          # Submit for notarization
          xcrun notarytool submit hush.zip \
            --apple-id "$MACOS_NOTARIZATION_APPLE_ID" \
            --team-id "$MACOS_NOTARIZATION_TEAM_ID" \
            --password "$MACOS_NOTARIZATION_PWD" \
            --wait

          # Staple ticket
          xcrun stapler staple hush-macos-arm64/hush
```

### 2. Create Entitlements File (for code signing)

**New file: `entitlements.plist`**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <!-- Required for Accessibility API -->
    <key>com.apple.security.automation.apple-events</key>
    <true/>

    <!-- Required for microphone access -->
    <key>com.apple.security.device.audio-input</key>
    <true/>

    <!-- Hardened runtime -->
    <key>com.apple.security.cs.allow-jit</key>
    <true/>
    <key>com.apple.security.cs.allow-unsigned-executable-memory</key>
    <true/>
    <key>com.apple.security.cs.disable-library-validation</key>
    <true/>
</dict>
</plist>
```

### 3. Add Release Documentation

**New file: `docs/RELEASING.md`**

```markdown
# Release Process for macOS

## Prerequisites

### For Code Signing (Optional)
1. Apple Developer account ($99/year)
2. Developer ID Application certificate
3. App-specific password for notarization

### GitHub Secrets Required
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

# Update CHANGELOG.md
vi CHANGELOG.md

# Commit version bump
git add Cargo.toml CHANGELOG.md
git commit -m "Bump version to X.Y.Z"
git push origin main
```

### 2. Create Tag

```bash
# Create and push tag
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin vX.Y.Z
```

### 3. GitHub Actions Builds

- Push triggers CI/CD workflow
- Tests run on both Apple Silicon and Intel
- Release binaries built and published
- GitHub Release created automatically

### 4. Verify Release

1. Go to: https://github.com/your-username/hush/releases
2. Check release notes
3. Download and test binaries:
   ```bash
   # Test arm64
   curl -L https://github.com/.../hush-macos-arm64.tar.gz -o hush.tar.gz
   tar xzf hush.tar.gz
   ./hush --version

   # Test x64
   # Similar steps
   ```

### 5. Announce Release

- Update README.md badges
- Post to Discord/social media (if applicable)
- Update documentation links

## Manual Build (Without CI)

```bash
# Build for current architecture
cargo build --release --features metal  # Apple Silicon
cargo build --release                     # Intel

# Create archive
cd target/release
tar czf hush-macos.tar.gz hush
shasum -a 256 hush-macos.tar.gz > hush-macos.tar.gz.sha256
```

## Troubleshooting

### Build fails on Intel runner

- Check `macos-13` runner is available in GitHub Actions
- Verify no Apple Silicon-specific code in critical paths

### Notarization fails

- Verify Apple Developer account is active
- Check app-specific password is current
- Ensure entitlements are correct

### Universal binary creation fails

- Ensure both architectures built successfully
- Check `lipo` command syntax
- Verify binary formats are compatible
```

---

## Success Criteria

- [ ] GitHub Actions workflow runs on both Apple Silicon and Intel
- [ ] Tests pass on macOS CI
- [ ] Release binaries built automatically on tags
- [ ] arm64, x64, and universal binaries created
- [ ] Binaries uploaded to GitHub Releases
- [ ] Checksums generated for all binaries
- [ ] Release notes auto-generated
- [ ] Code signing works (if enabled)
- [ ] Documentation for release process complete

---

## Verification Steps

```bash
# 1. Test workflow locally (optional)
# Install act: brew install act
act -j test

# 2. Push test tag
git tag -a v0.1.0-test -m "Test release"
git push origin v0.1.0-test

# 3. Check GitHub Actions
# Navigate to: https://github.com/your-repo/actions

# 4. Verify release created
# Navigate to: https://github.com/your-repo/releases

# 5. Download and test binaries
curl -L <release-url> -o hush.tar.gz
tar xzf hush.tar.gz
./hush --version
./hush check-permissions
```

---

## Files to Create

- `.github/workflows/macos-ci.yml` (~300 lines)
- `entitlements.plist` (~30 lines)
- `docs/RELEASING.md` (~200 lines)

## Files to Modify

- `.github/workflows/ci.yml` - Update to include macOS or split
- `README.md` - Add CI badge for macOS

---

## Dependencies

**Blocks:** None (infrastructure task)

**Blocked by:**
- T-033 through T-039 (need working macOS support to test)

---

## References

- **GitHub Actions**: https://docs.github.com/en/actions
- **macOS Runners**: https://github.com/actions/runner-images/blob/main/images/macos/macos-14-Readme.md
- **Code Signing**: https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution
- **Universal Binaries**: https://developer.apple.com/documentation/apple-silicon/building-a-universal-macos-binary

---

## Optional Enhancements

- Homebrew formula auto-update
- Upload to S3/CDN for faster downloads
- Build DMG installer (prettier than .tar.gz)
- Auto-update checker in app
- Beta/nightly builds from develop branch

Priority: Low (post-MVP)

---

## Cost Considerations

### GitHub Actions Minutes

- Free tier: 2,000 minutes/month for private repos, unlimited for public
- macOS runners: 10x multiplier (1 minute = 10 minutes charged)
- Estimated per release: ~30 minutes = 300 minutes charged

**Recommendation:** Use public repo to avoid costs, or budget ~$20/month for private repo CI.

### Apple Developer Account

- Required for: Code signing, notarization
- Cost: $99/year
- Optional for: MVP (unsigned binaries work, just show warning)

**Recommendation:** Skip for MVP, add later for production releases.
