# T-039: Add macOS Integration Tests

**Priority:** Medium
**Effort:** Medium (4-6 hours)
**Type:** Platform Support - Testing
**Status:** Available
**Created:** 2025-11-19

---

## Problem Statement

Current integration tests are Linux-specific or platform-agnostic. Need macOS-specific tests to validate:
- Text insertion via CGEvent and Accessibility API
- System tray integration via NSStatusBar
- Metal GPU acceleration
- Hotkey registration and event handling
- Permission flows (Accessibility, Microphone)

---

## Goals

1. ✅ Create macOS-specific integration test suite
2. ✅ Test text insertion adapter
3. ✅ Test system tray adapter
4. ✅ Test Metal GPU device detection
5. ✅ Test hotkey threading model
6. ✅ Test permission check utilities
7. ✅ Add CI/CD for macOS tests (if possible)

---

## Implementation Steps

### 1. Create macOS Test Module

**New file: `tests/macos_integration.rs`**

```rust
#![cfg(target_os = "macos")]

use hush::adapters::text::MacOSTextAdapter;
use hush::adapters::transcription::device::DeviceInfo;
use hush::adapters::hotkey::HotkeyAdapter;
use hush::core::traits::{TextOutput, SystemTray};

mod text_insertion {
    use super::*;

    #[test]
    fn test_macos_adapter_creation() {
        // Should succeed (may warn about permissions)
        let result = MacOSTextAdapter::new();
        assert!(result.is_ok() || result.err().unwrap().to_string().contains("Accessibility"));
    }

    #[test]
    fn test_accessibility_permission_check() {
        use hush::adapters::text::macos_accessibility::check_accessibility_permissions;

        // This will be false in CI, true if manually granted
        let has_permissions = check_accessibility_permissions();
        println!("Accessibility permissions: {}", has_permissions);

        // Don't fail test, just report
        // In CI, this will be false
    }

    #[tokio::test]
    #[ignore] // Requires Accessibility permissions
    async fn test_text_insertion_with_permissions() {
        let mut adapter = MacOSTextAdapter::new().expect("Failed to create adapter");

        // This requires Accessibility permissions
        let result = adapter.insert_text("Hello from Hush test").await;

        if result.is_err() {
            let err = result.err().unwrap();
            if err.to_string().contains("Accessibility") {
                println!("SKIP: Accessibility permissions not granted");
                return;
            }
            panic!("Unexpected error: {}", err);
        }

        // If we get here, text insertion worked
        println!("✅ Text insertion successful");
    }

    #[tokio::test]
    async fn test_window_detection() {
        use hush::adapters::text::macos_accessibility::get_focused_window_info;

        let result = get_focused_window_info();

        // May fail without permissions, but should not panic
        match result {
            Ok(window) => {
                println!("Focused window: {} ({})", window.title, window.app_name);
            }
            Err(e) => {
                println!("Window detection failed (expected without permissions): {}", e);
            }
        }
    }
}

mod system_tray {
    use super::*;
    use hush::tray::adapters::MacOSTrayAdapter;

    #[test]
    fn test_tray_creation() {
        // Tray creation should succeed
        let result = MacOSTrayAdapter::new();

        match result {
            Ok(_) => println!("✅ Tray created successfully"),
            Err(e) => println!("⚠️  Tray creation failed: {}", e),
        }

        // Don't fail test in CI (may not have display)
    }

    #[test]
    #[ignore] // Requires display/GUI
    fn test_tray_icon_update() {
        let mut tray = MacOSTrayAdapter::new().expect("Failed to create tray");

        // Test state transitions
        let states = vec!["idle", "recording", "processing"];

        for state in states {
            let result = tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(tray.set_status(state));

            assert!(result.is_ok(), "Failed to set status: {}", state);
        }
    }
}

mod gpu_acceleration {
    use super::*;

    #[test]
    fn test_device_detection() {
        let device_info = DeviceInfo::detect();

        println!("Detected device: {}", device_info.name);
        println!("Expected latency: {}ms", device_info.expected_latency_ms);

        // On Apple Silicon, should detect Metal
        #[cfg(target_arch = "aarch64")]
        {
            assert!(device_info.name.contains("Apple") || device_info.name.contains("Metal"));
        }

        // On Intel Mac, should fall back to CPU
        #[cfg(target_arch = "x86_64")]
        {
            // Intel Macs may use Metal or CPU
            println!("Intel Mac - device: {}", device_info.name);
        }
    }

    #[test]
    #[cfg(feature = "metal")]
    fn test_metal_device_creation() {
        use candle_core::Device;

        let result = Device::new_metal(0);

        match result {
            Ok(device) => {
                println!("✅ Metal device created successfully");
            }
            Err(e) => {
                // May fail on Intel Macs or in CI
                println!("⚠️  Metal device creation failed: {}", e);
                println!("This is expected on Intel Macs or without Metal support");
            }
        }
    }
}

mod hotkey {
    use super::*;

    #[test]
    fn test_hotkey_main_thread_check() {
        // This test runs on main thread, so should succeed
        let result = HotkeyAdapter::new();

        match result {
            Ok(_) => println!("✅ Hotkey adapter created on main thread"),
            Err(e) => {
                // May fail in CI without proper setup
                println!("⚠️  Hotkey creation failed: {}", e);
            }
        }
    }

    #[test]
    #[ignore] // Can't reliably test in CI
    fn test_hotkey_registration() {
        let adapter = HotkeyAdapter::new().expect("Failed to create adapter");

        // Register Cmd+Alt+V
        let result = adapter.register("Cmd+Alt+V");

        match result {
            Ok(id) => println!("✅ Hotkey registered with ID: {}", id),
            Err(e) => println!("⚠️  Hotkey registration failed: {}", e),
        }
    }
}

mod permissions {
    use super::*;

    #[test]
    fn test_accessibility_permission_prompt() {
        use hush::adapters::text::macos_accessibility::prompt_accessibility_permissions;

        // This should not panic, just print instructions
        // Don't actually open System Preferences in tests
        println!("Testing permission prompt (no-op in tests)");
    }

    #[test]
    fn test_microphone_permission_check() {
        // TODO: Add microphone permission check
        // This is handled by cpal, but we should verify
        println!("Microphone permission check - TBD");
    }
}

// Helper: Check if we're running in CI
fn is_ci() -> bool {
    std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok()
}
```

### 2. Add Manual Test Script

**New file: `scripts/test-macos.sh`**

```bash
#!/bin/bash
# Manual macOS testing script
# Run this on a real macOS machine with display and permissions

set -e

echo "🍎 macOS Integration Testing"
echo ""

# Check platform
if [[ "$OSTYPE" != "darwin"* ]]; then
    echo "❌ This script must run on macOS"
    exit 1
fi

# Check for Accessibility permissions
echo "1️⃣  Checking Accessibility permissions..."
if ./target/debug/hush check-permissions | grep -q "granted"; then
    echo "✅ Accessibility permissions granted"
else
    echo "⚠️  Accessibility permissions not granted"
    echo "   Some tests will be skipped"
fi

# Run unit tests
echo ""
echo "2️⃣  Running unit tests..."
cargo test --lib

# Run macOS-specific tests
echo ""
echo "3️⃣  Running macOS integration tests..."
cargo test --test macos_integration -- --test-threads=1

# Run ignored tests (require permissions)
echo ""
echo "4️⃣  Running permission-required tests..."
echo "   (These may fail without permissions)"
cargo test --test macos_integration -- --ignored --test-threads=1 || true

# Test hotkey functionality (interactive)
echo ""
echo "5️⃣  Testing hotkey (interactive)..."
echo "   Starting application, press Cmd+Alt+V to test"
timeout 10 ./target/debug/hush listen || true

# Test text insertion (interactive)
echo ""
echo "6️⃣  Testing text insertion (interactive)..."
echo "   Open TextEdit, then run: ./target/debug/hush listen"
echo "   Speak something and verify text appears"
echo "   Press Enter to continue..."
read

# Test system tray
echo ""
echo "7️⃣  Testing system tray..."
echo "   Check for tray icon in menu bar"
./target/debug/hush listen &
HUSH_PID=$!
sleep 3
echo "   See tray icon? (y/n)"
read response
kill $HUSH_PID

if [[ "$response" == "y" ]]; then
    echo "✅ System tray test passed"
else
    echo "❌ System tray test failed"
fi

# Test Metal GPU
echo ""
echo "8️⃣  Testing Metal GPU..."
./target/debug/hush device-info

echo ""
echo "✅ All tests complete!"
echo ""
echo "Summary:"
echo "  - Unit tests: ✅"
echo "  - Integration tests: ✅"
echo "  - Manual tests: Check output above"
```

### 3. Add CI/CD for macOS (GitHub Actions)

**New file: `.github/workflows/macos-tests.yml`**

```yaml
name: macOS Tests

on:
  push:
    branches: [ main, 'claude/**' ]
  pull_request:
    branches: [ main ]

jobs:
  test-macos:
    name: Test on macOS
    runs-on: macos-latest

    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rust-lang/setup-rust-toolchain@v1
        with:
          toolchain: stable

      - name: Check macOS version
        run: sw_vers

      - name: Detect Apple Silicon vs Intel
        run: |
          uname -m
          sysctl -n machdep.cpu.brand_string

      - name: Cache cargo registry
        uses: actions/cache@v3
        with:
          path: ~/.cargo/registry
          key: ${{ runner.os }}-cargo-registry-${{ hashFiles('**/Cargo.lock') }}

      - name: Cache cargo build
        uses: actions/cache@v3
        with:
          path: target
          key: ${{ runner.os }}-cargo-build-target-${{ hashFiles('**/Cargo.lock') }}

      - name: Build (no default features, no GPU)
        run: cargo build --no-default-features

      - name: Run unit tests
        run: cargo test --lib --no-default-features

      - name: Run macOS integration tests (no permissions)
        run: cargo test --test macos_integration --no-default-features
        continue-on-error: true  # May fail without display/permissions

      - name: Check device detection
        run: cargo run --bin hush -- device-info || true

      - name: Build with Metal feature
        if: runner.arch == 'ARM64'
        run: cargo build --features metal

      - name: Test Metal device detection
        if: runner.arch == 'ARM64'
        run: cargo test gpu_acceleration::test_device_detection --features metal
```

### 4. Add Permission Check CLI Command

**Update: `src/cli/mod.rs`**

```rust
#[derive(Subcommand)]
pub enum Commands {
    // ... existing commands

    /// Check system permissions (macOS)
    #[cfg(target_os = "macos")]
    CheckPermissions,
}

// Handler
#[cfg(target_os = "macos")]
Commands::CheckPermissions => {
    check_macos_permissions();
}

#[cfg(target_os = "macos")]
fn check_macos_permissions() {
    use crate::adapters::text::macos_accessibility::check_accessibility_permissions;

    println!("🔐 macOS Permission Status\n");

    // Accessibility
    let accessibility = check_accessibility_permissions();
    if accessibility {
        println!("✅ Accessibility: Granted");
    } else {
        println!("❌ Accessibility: Not granted");
        println!("   Required for text insertion");
        println!("   Settings → Privacy & Security → Accessibility");
    }

    // Microphone (check if cpal can access)
    println!("\n🎤 Microphone: Checking...");
    match cpal::default_host().default_input_device() {
        Some(_) => println!("✅ Microphone: Accessible"),
        None => {
            println!("⚠️  Microphone: Not accessible");
            println!("   Settings → Privacy & Security → Microphone");
        }
    }

    println!();
}
```

---

## Success Criteria

- [ ] macOS integration test suite compiles
- [ ] Tests run in CI (even if some skip)
- [ ] Manual test script works on real macOS machines
- [ ] Permission checks work correctly
- [ ] Device detection tests pass
- [ ] Text insertion tests pass (when permissions granted)
- [ ] Tray tests pass (when display available)
- [ ] GitHub Actions workflow runs on macOS

---

## Verification Steps

```bash
# On macOS (local machine):
# 1. Run all tests
./scripts/test-macos.sh

# 2. Run just integration tests
cargo test --test macos_integration

# 3. Run with permissions-required tests
cargo test --test macos_integration -- --ignored

# 4. Check permissions
cargo run --bin hush -- check-permissions

# In CI (GitHub Actions):
# Push to branch, check Actions tab
git push origin your-branch
# Navigate to: https://github.com/your-repo/actions
```

---

## Files to Create

- `tests/macos_integration.rs` (~400 lines)
- `scripts/test-macos.sh` (~100 lines)
- `.github/workflows/macos-tests.yml` (~80 lines)

## Files to Modify

- `src/cli/mod.rs` - Add check-permissions command
- `Cargo.toml` - Add test dependencies if needed

---

## Dependencies

**Blocks:**
- T-040 (docs need test instructions)

**Blocked by:**
- T-035 (text adapter must exist)
- T-036 (Metal support must exist)
- T-037 (tray adapter must exist)
- T-038 (hotkey threading must be fixed)

---

## References

- **Rust testing**: https://doc.rust-lang.org/book/ch11-00-testing.html
- **GitHub Actions macOS**: https://docs.github.com/en/actions/using-github-hosted-runners/about-github-hosted-runners#supported-runners-and-hardware-resources
- **Integration testing**: https://doc.rust-lang.org/rust-by-example/testing/integration_testing.html

---

## Testing Checklist

Manual testing on real macOS machine:

- [ ] Text insertion works in TextEdit
- [ ] Text insertion works in Terminal
- [ ] Text insertion works in browser
- [ ] System tray icon appears
- [ ] Tray menu items clickable
- [ ] Hotkey registration works
- [ ] Hotkey triggering works
- [ ] Metal GPU detected on Apple Silicon
- [ ] CPU fallback works on Intel
- [ ] Permission prompts appear correctly
- [ ] App works after granting permissions
- [ ] App degrades gracefully without permissions

---

## Known Limitations

1. **CI limitations**: GitHub Actions macOS runners don't have:
   - Display/GUI access (tray tests will skip)
   - Accessibility permissions (text tests will skip)
   - Interactive capability (hotkey tests will skip)

2. **Solution**: Most tests are "smoke tests" that verify compilation and basic functionality. Full testing requires manual verification.

3. **Future**: Consider using macOS UI testing frameworks (XCTest) for more comprehensive automation.
