/// Build script for Hush
///
/// Checks for required system dependencies and provides helpful error messages
/// when dependencies are missing.
use std::process::Command;

fn main() {
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

    // Check for D-Bus (optional - only needed for notifications or system-tray features)
    #[cfg(any(feature = "notifications", feature = "system-tray"))]
    {
        if !check_pkg_config_package("dbus-1") {
            eprintln!("\n⚠️  WARNING: D-Bus development libraries not found\n");
            eprintln!("D-Bus is required for desktop notifications and system tray.\n");
            eprintln!("Install it using:\n");
            eprintln!("  Ubuntu/Debian:  sudo apt-get install libdbus-1-dev");
            eprintln!("  Fedora/RHEL:    sudo dnf install dbus-devel");
            eprintln!("  Arch Linux:     sudo pacman -S dbus");
            eprintln!("  openSUSE:       sudo zypper install dbus-1-devel");
            eprintln!("\nAlternatively, build without D-Bus features:\n");
            eprintln!("  cargo build --no-default-features");
            eprintln!("\nFor more information, see INSTALL.md\n");
            std::process::exit(1);
        }
    }

    // Check for CUDA (optional - for GPU acceleration)
    if check_command_exists("nvcc", &["--version"]) {
        println!("cargo:warning=✅ CUDA toolkit detected - GPU acceleration will be available");

        // Try to get CUDA version
        if let Ok(output) = Command::new("nvcc").arg("--version").output() {
            if let Ok(version_str) = String::from_utf8(output.stdout) {
                if let Some(version_line) = version_str.lines().find(|l| l.contains("release")) {
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

    // Provide helpful summary
    println!("cargo:warning=");
    println!("cargo:warning=📋 Build dependency check complete");
    println!("cargo:warning=   For installation help, see: INSTALL.md");
    println!("cargo:warning=");

    // Emit metadata for cargo
    println!("cargo:rerun-if-changed=build.rs");
}

/// Check if a command exists and can be executed
fn check_command_exists(command: &str, args: &[&str]) -> bool {
    Command::new(command).args(args).output().is_ok()
}

/// Check if a pkg-config package exists
fn check_pkg_config_package(package: &str) -> bool {
    Command::new("pkg-config")
        .args(&["--exists", package])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}
