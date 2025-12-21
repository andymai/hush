/// Build script for Hush
///
/// Checks for required system dependencies and provides helpful error messages
/// when dependencies are missing.
use std::process::Command;

fn main() {
    eprintln!();
    eprintln!("Hush Build System");
    eprintln!();

    check_linux_dependencies();
    check_gpu_support();
    print_build_summary();

    println!("cargo:rerun-if-changed=build.rs");
}

fn check_linux_dependencies() {
    eprintln!("Checking Linux dependencies...");

    // Check for pkg-config
    if !check_command_exists("pkg-config", &["--version"]) {
        println!("cargo:warning=pkg-config not found - required to build Hush");
        eprintln!("\nERROR: pkg-config not found\n");
        eprintln!("pkg-config is required to build Hush.\n");
        eprintln!("Install it using:\n");
        eprintln!("  Ubuntu/Debian:  sudo apt-get install pkg-config");
        eprintln!("  Fedora/RHEL:    sudo dnf install pkg-config");
        eprintln!("  Arch Linux:     sudo pacman -S pkg-config");
        eprintln!("\nFor more information, see INSTALL.md\n");
        std::process::exit(1);
    }

    // Check for ALSA (required for audio capture)
    if !check_pkg_config_package("alsa") {
        println!("cargo:warning=ALSA development libraries not found - required for audio capture");
        eprintln!("\nERROR: ALSA development libraries not found\n");
        eprintln!("ALSA is required for audio capture.\n");
        eprintln!("Install it using:\n");
        eprintln!("  Ubuntu/Debian:  sudo apt-get install libasound2-dev");
        eprintln!("  Fedora/RHEL:    sudo dnf install alsa-lib-devel");
        eprintln!("  Arch Linux:     sudo pacman -S alsa-lib");
        eprintln!("\nFor more information, see INSTALL.md\n");
        std::process::exit(1);
    }

    // Check for X11 (required for window management)
    if !check_pkg_config_package("x11") {
        println!("cargo:warning=X11 development libraries not found - some features may not work");
        eprintln!("\nWARNING: X11 development libraries not found\n");
        eprintln!("X11 is recommended for full functionality.\n");
        eprintln!("Install it using:\n");
        eprintln!("  Ubuntu/Debian:  sudo apt-get install libx11-dev");
        eprintln!("  Fedora/RHEL:    sudo dnf install libX11-devel");
        eprintln!("  Arch Linux:     sudo pacman -S libx11");
        eprintln!("\nContinuing build, but some features may not work.\n");
    }

    eprintln!("Linux dependencies OK");
}

fn check_gpu_support() {
    // Check for CUDA (optional - for GPU acceleration)
    if check_command_exists("nvcc", &["--version"]) {
        eprintln!("CUDA toolkit detected - GPU acceleration available");

        if let Ok(output) = Command::new("nvcc").arg("--version").output() {
            if let Ok(version_str) = String::from_utf8(output.stdout) {
                if let Some(version_line) = version_str.lines().find(|l| l.contains("release")) {
                    eprintln!("  CUDA version: {}", version_line.trim());
                }
            }
        }
    } else if check_command_exists("nvidia-smi", &[]) {
        println!("cargo:warning=NVIDIA GPU detected but CUDA toolkit not found - install CUDA 12.0+ for GPU acceleration");
        eprintln!("NVIDIA GPU detected but CUDA toolkit not found");
        eprintln!("  Install CUDA 12.0+ for GPU acceleration");
        eprintln!("  See INSTALL.md for CUDA installation instructions");
    }
}

fn print_build_summary() {
    eprintln!();
    eprintln!("Build Configuration:");
    eprintln!("  Platform: Linux");

    #[cfg(feature = "cuda")]
    eprintln!("  GPU: CUDA enabled");

    #[cfg(not(feature = "cuda"))]
    eprintln!("  GPU: CPU only (use --features cuda for GPU)");

    eprintln!();
}

fn check_command_exists(command: &str, args: &[&str]) -> bool {
    Command::new(command).args(args).output().is_ok()
}

fn check_pkg_config_package(package: &str) -> bool {
    Command::new("pkg-config")
        .args(["--exists", package])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}
