/// Build script for Hush
///
/// Checks for required system dependencies and provides helpful error messages
/// when dependencies are missing.
///
/// Platform-specific checks:
/// - Linux: pkg-config, ALSA, X11, D-Bus, CUDA
/// - macOS: CoreAudio, CoreGraphics, AppKit, Metal
use std::process::Command;

fn main() {
    println!("cargo:warning=");
    println!("cargo:warning=🔧 Hush Build System");
    println!("cargo:warning=");

    // Platform-specific dependency checks
    #[cfg(target_os = "linux")]
    check_linux_dependencies();

    #[cfg(target_os = "macos")]
    check_macos_dependencies();

    // GPU support detection (platform-specific)
    check_gpu_support();

    // Build configuration summary
    print_build_summary();

    // Emit metadata for cargo
    println!("cargo:rerun-if-changed=build.rs");
}

//==============================================================================
// Linux Dependency Checks
//==============================================================================

#[cfg(target_os = "linux")]
fn check_linux_dependencies() {
    println!("cargo:warning=🐧 Checking Linux dependencies...");

    // Check for pkg-config
    if !check_command_exists("pkg-config", &["--version"]) {
        eprintln!("\n❌ ERROR: pkg-config not found\n");
        eprintln!("pkg-config is required to build Hush.\n");
        eprintln!("Install it using:\n");
        eprintln!("  Ubuntu/Debian:  sudo apt-get install pkg-config");
        eprintln!("  Fedora/RHEL:    sudo dnf install pkg-config");
        eprintln!("  Arch Linux:     sudo pacman -S pkg-config");
        eprintln!("  openSUSE:       sudo zypper install pkg-config");
        eprintln!("\nFor more information, see INSTALL.md\n");
        std::process::exit(1);
    }

    // Check for ALSA (required for audio capture)
    if !check_pkg_config_package("alsa") {
        eprintln!("\n❌ ERROR: ALSA development libraries not found\n");
        eprintln!("ALSA is required for audio capture.\n");
        eprintln!("Install it using:\n");
        eprintln!("  Ubuntu/Debian:  sudo apt-get install libasound2-dev");
        eprintln!("  Fedora/RHEL:    sudo dnf install alsa-lib-devel");
        eprintln!("  Arch Linux:     sudo pacman -S alsa-lib");
        eprintln!("  openSUSE:       sudo zypper install alsa-devel");
        eprintln!("\nFor more information, see INSTALL.md\n");
        std::process::exit(1);
    }

    // Check for X11 (required for window management)
    if !check_pkg_config_package("x11") {
        eprintln!("\n⚠️  WARNING: X11 development libraries not found\n");
        eprintln!("X11 is recommended for full functionality.\n");
        eprintln!("Install it using:\n");
        eprintln!("  Ubuntu/Debian:  sudo apt-get install libx11-dev");
        eprintln!("  Fedora/RHEL:    sudo dnf install libX11-devel");
        eprintln!("  Arch Linux:     sudo pacman -S libx11");
        eprintln!("  openSUSE:       sudo zypper install libX11-devel");
        eprintln!("\nContinuing build, but some features may not work.\n");
        // Don't exit - X11 issues will be caught by cargo if critical
    }

    println!("cargo:warning=✅ Linux dependencies OK");
    println!("cargo:warning=");
}

//==============================================================================
// macOS Dependency Checks
//==============================================================================

#[cfg(target_os = "macos")]
fn check_macos_dependencies() {
    println!("cargo:warning=🍎 Checking macOS system frameworks...");

    // Check for CoreAudio (required for audio capture)
    if !check_macos_framework("CoreAudio") {
        eprintln!("\n❌ ERROR: CoreAudio framework not found\n");
        eprintln!("CoreAudio is required for audio capture.");
        eprintln!("This should be available on all macOS systems.");
        eprintln!("\nIf you're on an older macOS version, try:");
        eprintln!("  xcode-select --install");
        eprintln!();
        std::process::exit(1);
    }

    // Check for CoreGraphics (required for text insertion)
    if !check_macos_framework("CoreGraphics") {
        eprintln!("\n❌ ERROR: CoreGraphics framework not found\n");
        eprintln!("CoreGraphics is required for keyboard simulation.");
        eprintln!("Install Xcode Command Line Tools:");
        eprintln!("  xcode-select --install");
        eprintln!();
        std::process::exit(1);
    }

    // Check for AppKit (required for system tray)
    if !check_macos_framework("AppKit") {
        println!("cargo:warning=⚠️  AppKit framework not detected");
        println!("cargo:warning=   System tray may not function properly");
    }

    // Check for CoreFoundation (commonly available, but verify)
    if !check_macos_framework("CoreFoundation") {
        println!("cargo:warning=⚠️  CoreFoundation framework not detected");
        println!("cargo:warning=   Some system APIs may not work correctly");
    }

    // Detect Accessibility permissions requirement
    println!("cargo:warning=");
    println!("cargo:warning=⚠️  macOS Accessibility Permissions Required");
    println!("cargo:warning=   Hush requires Accessibility permissions for text insertion");
    println!("cargo:warning=   You will be prompted to grant access on first run");
    println!("cargo:warning=   Settings → Privacy & Security → Accessibility");
    println!("cargo:warning=");

    println!("cargo:warning=✅ macOS frameworks detected");
    println!("cargo:warning=");
}

#[cfg(target_os = "macos")]
fn check_macos_framework(framework: &str) -> bool {
    // Check if framework exists by trying to compile a minimal program that links against it
    use std::io::Write;
    use std::process::Stdio;

    let test_code = "int main(void) { return 0; }";

    let mut child = match Command::new("clang")
        .args(&["-framework", framework, "-xc", "-", "-o", "/dev/null"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return false,
    };

    // Write test code to stdin
    if let Some(ref mut stdin) = child.stdin {
        let _ = stdin.write_all(test_code.as_bytes());
    }

    // Check if compilation succeeded
    child.wait().map(|status| status.success()).unwrap_or(false)
}

//==============================================================================
// GPU Support Detection
//==============================================================================

fn check_gpu_support() {
    #[cfg(target_os = "linux")]
    {
        // Check for CUDA (optional - for GPU acceleration)
        if check_command_exists("nvcc", &["--version"]) {
            println!("cargo:warning=✅ CUDA toolkit detected - GPU acceleration available");

            // Try to get CUDA version
            if let Ok(output) = Command::new("nvcc").arg("--version").output() {
                if let Ok(version_str) = String::from_utf8(output.stdout) {
                    if let Some(version_line) = version_str.lines().find(|l| l.contains("release"))
                    {
                        println!("cargo:warning=   CUDA version: {}", version_line.trim());
                    }
                }
            }
        } else if check_command_exists("nvidia-smi", &[]) {
            println!("cargo:warning=⚠️  NVIDIA GPU detected but CUDA toolkit not found");
            println!(
                "cargo:warning=   Install CUDA 12.0+ for GPU acceleration (10x faster transcription)"
            );
            println!("cargo:warning=   See INSTALL.md for CUDA installation instructions");
        }
    }

    #[cfg(target_os = "macos")]
    {
        // Detect Apple Silicon vs Intel for GPU recommendations
        if let Ok(output) = Command::new("sysctl")
            .args(&["-n", "machdep.cpu.brand_string"])
            .output()
        {
            if let Ok(cpu_info) = String::from_utf8(output.stdout) {
                if cpu_info.contains("Apple") {
                    println!("cargo:warning=✅ Apple Silicon detected");
                    println!("cargo:warning=   Metal GPU acceleration available");
                    println!(
                        "cargo:warning=   Expected performance: ~120ms first token on M1/M2/M3"
                    );

                    #[cfg(feature = "metal")]
                    println!("cargo:warning=   ✅ Metal feature enabled");

                    #[cfg(not(feature = "metal"))]
                    {
                        println!("cargo:warning=   ⚠️  Metal feature not enabled");
                        println!(
                            "cargo:warning=   Build with --features metal for GPU acceleration"
                        );
                    }
                } else {
                    println!("cargo:warning=ℹ️  Intel Mac detected");
                    println!("cargo:warning=   Metal GPU available but slower than Apple Silicon");
                    println!("cargo:warning=   CPU-only mode recommended for best performance");
                }
            }
        }
    }
}

//==============================================================================
// Build Configuration Summary
//==============================================================================

fn print_build_summary() {
    println!("cargo:warning=");
    println!("cargo:warning=📋 Build Configuration:");

    #[cfg(target_os = "linux")]
    println!("cargo:warning=   Platform: Linux");

    #[cfg(target_os = "macos")]
    println!("cargo:warning=   Platform: macOS");

    #[cfg(feature = "cuda")]
    println!("cargo:warning=   GPU: CUDA enabled");

    #[cfg(all(not(feature = "cuda"), target_os = "linux"))]
    println!("cargo:warning=   GPU: CPU only (use --features cuda for GPU)");

    #[cfg(all(target_os = "macos", feature = "metal"))]
    println!("cargo:warning=   GPU: Metal enabled");

    #[cfg(all(target_os = "macos", not(feature = "metal")))]
    println!("cargo:warning=   GPU: CPU only (use --features metal for GPU)");

    println!("cargo:warning=");
    println!("cargo:warning=📋 Build dependency check complete");
    println!("cargo:warning=   For installation help, see: INSTALL.md");
    println!("cargo:warning=");
}

//==============================================================================
// Helper Functions
//==============================================================================

/// Check if a command exists and can be executed
fn check_command_exists(command: &str, args: &[&str]) -> bool {
    Command::new(command).args(args).output().is_ok()
}

/// Check if a pkg-config package exists (Linux only)
#[cfg(target_os = "linux")]
fn check_pkg_config_package(package: &str) -> bool {
    Command::new("pkg-config")
        .args(["--exists", package])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}
