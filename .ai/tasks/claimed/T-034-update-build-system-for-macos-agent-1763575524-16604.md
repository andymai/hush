# T-034: Update Build System for macOS Framework Detection

**Priority:** High
**Effort:** Low (2-3 hours)
**Type:** Platform Support - Foundation
**Status:** Available
**Created:** 2025-11-19

---

## Problem Statement

The current `build.rs` hardcodes Linux-only dependency checks (ALSA, X11, D-Bus), causing build failures on macOS. The build script needs platform-conditional checks for system libraries.

### Current Issues

```rust
// build.rs:22-63 - All checks are Linux-specific
check_pkg_config_package("alsa");    // Linux only
check_pkg_config_package("x11");     // Linux only
check_pkg_config_package("dbus-1");  // Linux only
```

On macOS, these checks fail even though equivalent frameworks exist (CoreAudio, CoreGraphics, etc.).

---

## Goals

1. ✅ Make dependency checks platform-conditional
2. ✅ Add macOS framework detection
3. ✅ Provide helpful error messages for missing macOS frameworks
4. ✅ Detect Apple Silicon vs Intel for GPU recommendations
5. ✅ Maintain existing Linux behavior

---

## Implementation Steps

### 1. Wrap Linux Checks in Platform Conditionals

```rust
// build.rs
fn main() {
    #[cfg(target_os = "linux")]
    check_linux_dependencies();

    #[cfg(target_os = "macos")]
    check_macos_dependencies();

    // Common checks (CUDA optional, applies to both platforms)
    check_gpu_support();
}

fn check_linux_dependencies() {
    // Existing code for pkg-config, ALSA, X11, D-Bus
    check_pkg_config_package("alsa");
    check_pkg_config_package("x11");

    #[cfg(any(feature = "notifications", feature = "system-tray"))]
    check_pkg_config_package("dbus-1");
}
```

### 2. Add macOS Framework Detection

```rust
fn check_macos_dependencies() {
    println!("cargo:warning=🍎 Detecting macOS system frameworks...");

    // Check for CoreAudio (required for audio capture)
    if !check_macos_framework("CoreAudio") {
        eprintln!("\n❌ ERROR: CoreAudio framework not found\n");
        eprintln!("CoreAudio is required for audio capture.");
        eprintln!("This should be available on all macOS systems.");
        eprintln!("\nIf you're on an older macOS version, try:");
        eprintln!("  xcode-select --install");
        std::process::exit(1);
    }

    // Check for CoreGraphics (required for text insertion)
    if !check_macos_framework("CoreGraphics") {
        eprintln!("\n❌ ERROR: CoreGraphics framework not found\n");
        eprintln!("CoreGraphics is required for keyboard simulation.");
        eprintln!("Install Xcode Command Line Tools:");
        eprintln!("  xcode-select --install");
        std::process::exit(1);
    }

    // Check for AppKit (required for system tray)
    if !check_macos_framework("AppKit") {
        println!("cargo:warning=⚠️  AppKit framework not detected");
        println!("cargo:warning=   System tray may not function properly");
    }

    // Detect Accessibility permissions requirement
    println!("cargo:warning=");
    println!("cargo:warning=⚠️  macOS Accessibility Permissions Required");
    println!("cargo:warning=   Hush requires Accessibility permissions for text insertion");
    println!("cargo:warning=   You will be prompted to grant access on first run");
    println!("cargo:warning=   Settings → Privacy & Security → Accessibility");

    println!("cargo:warning=✅ macOS frameworks detected");
}

fn check_macos_framework(framework: &str) -> bool {
    // Check if framework exists by trying to link against it
    use std::process::Command;

    Command::new("clang")
        .args(&[
            "-framework",
            framework,
            "-xc",
            "-",
            "-o",
            "/dev/null",
        ])
        .stdin(std::process::Stdio::piped())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}
```

### 3. Enhance GPU Detection for Apple Silicon

```rust
fn check_gpu_support() {
    #[cfg(target_os = "linux")]
    {
        // Existing CUDA detection
        if check_command_exists("nvcc", &["--version"]) {
            println!("cargo:warning=✅ CUDA toolkit detected - GPU acceleration available");
        } else if check_command_exists("nvidia-smi", &[]) {
            println!("cargo:warning=⚠️  NVIDIA GPU detected but CUDA toolkit not found");
        }
    }

    #[cfg(target_os = "macos")]
    {
        // Detect Apple Silicon vs Intel
        if let Ok(output) = Command::new("sysctl")
            .args(&["-n", "machdep.cpu.brand_string"])
            .output()
        {
            if let Ok(cpu_info) = String::from_utf8(output.stdout) {
                if cpu_info.contains("Apple") {
                    println!("cargo:warning=✅ Apple Silicon detected");
                    println!("cargo:warning=   Metal GPU acceleration available (use --features metal)");
                    println!("cargo:warning=   Expected performance: ~120ms first token on M2");
                } else {
                    println!("cargo:warning=⚠️  Intel Mac detected");
                    println!("cargo:warning=   Metal GPU available but slower than Apple Silicon");
                    println!("cargo:warning=   Consider CPU-only mode for best performance");
                }
            }
        }
    }
}
```

### 4. Add Build Configuration Helpers

```rust
// Print helpful build configuration summary
println!("cargo:warning=");
println!("cargo:warning=📋 Build Configuration:");

#[cfg(target_os = "linux")]
println!("cargo:warning=   Platform: Linux");

#[cfg(target_os = "macos")]
println!("cargo:warning=   Platform: macOS");

#[cfg(feature = "cuda")]
println!("cargo:warning=   GPU: CUDA");

#[cfg(feature = "metal")]
println!("cargo:warning=   GPU: Metal");

#[cfg(not(any(feature = "cuda", feature = "metal")))]
println!("cargo:warning=   GPU: CPU only");

println!("cargo:warning=");
```

---

## Success Criteria

- [ ] `cargo build` succeeds on Linux with existing behavior
- [ ] `cargo build` succeeds on macOS with appropriate framework checks
- [ ] Build script detects missing macOS frameworks and provides helpful errors
- [ ] Apple Silicon vs Intel detection works correctly
- [ ] Metal GPU recommendations appear on Apple Silicon Macs
- [ ] Accessibility permission warnings shown on macOS
- [ ] No false positives or unnecessary warnings

---

## Verification Steps

```bash
# On Linux: Verify existing behavior
cargo clean && cargo build 2>&1 | grep -A5 "Build dependency"

# On macOS: Verify framework detection
cargo clean && cargo build 2>&1 | grep -A10 "macOS system frameworks"

# Verify Apple Silicon detection (on M1/M2 Mac)
cargo clean && cargo build 2>&1 | grep "Apple Silicon"

# Verify Intel detection (on Intel Mac)
cargo clean && cargo build 2>&1 | grep "Intel Mac"

# Test missing framework handling (simulate)
# (This requires temporarily renaming/hiding a framework - advanced testing)
```

---

## Files to Modify

- `build.rs` (Lines 1-107)
  - Add platform-conditional compilation blocks
  - Add `check_macos_dependencies()` function
  - Add `check_macos_framework()` helper
  - Update `check_gpu_support()` for Apple Silicon detection
  - Add macOS-specific error messages

---

## Dependencies

**Blocks:**
- T-035 (text adapter needs framework checks to validate environment)
- T-036 (Metal feature needs GPU detection)

**Blocked by:**
- T-033 (needs platform-conditional dependencies in Cargo.toml first)

---

## References

- **macOS Frameworks**: https://developer.apple.com/documentation/
  - CoreAudio: Audio capture and playback
  - CoreGraphics: CGEvent API for keyboard/mouse simulation
  - AppKit: System tray and menu bar integration
  - Accessibility: Required for text insertion
- **Apple Silicon Detection**: `sysctl -n machdep.cpu.brand_string`
- **Framework Linking**: https://doc.rust-lang.org/rustc/command-line-arguments.html#-l-link-the-generated-crate

---

## Testing Notes

**On macOS:**
1. First build should show all framework checks
2. Subsequent builds should be fast (framework checks cached)
3. Apple Silicon Macs should see Metal recommendation
4. Intel Macs should see CPU recommendation

**On Linux:**
1. Build behavior should be identical to before
2. No macOS-related messages should appear
3. CUDA detection unchanged

---

## Edge Cases

- **Virtual machines**: Framework detection may behave differently in VMs
- **Hackintosh**: May have incomplete framework support
- **Older macOS versions**: Some frameworks may be missing
- **Cross-compilation**: Build script runs on build host, not target (document this limitation)
