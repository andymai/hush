/// Build script for Hush
///
/// Checks for required system dependencies and provides helpful error messages
/// when dependencies are missing.
use std::process::Command;

fn main() {
    println!("cargo:warning=");
    println!("cargo:warning=Hush Build System");
    println!("cargo:warning=");

    check_linux_dependencies();
    check_gpu_support();
    print_build_summary();

    println!("cargo:rerun-if-changed=build.rs");
}

fn check_linux_dependencies() {
    println!("cargo:warning=Checking Linux dependencies...");

    // Check for pkg-config
    if !check_command_exists("pkg-config", &["--version"]) {
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
        eprintln!("\nWARNING: X11 development libraries not found\n");
        eprintln!("X11 is recommended for full functionality.\n");
        eprintln!("Install it using:\n");
        eprintln!("  Ubuntu/Debian:  sudo apt-get install libx11-dev");
        eprintln!("  Fedora/RHEL:    sudo dnf install libX11-devel");
        eprintln!("  Arch Linux:     sudo pacman -S libx11");
        eprintln!("\nContinuing build, but some features may not work.\n");
    }

    println!("cargo:warning=Linux dependencies OK");
    println!("cargo:warning=");
}

fn check_gpu_support() {
    // Check for CUDA (optional - for GPU acceleration)
    if check_command_exists("nvcc", &["--version"]) {
        println!("cargo:warning=CUDA toolkit detected - GPU acceleration available");

        if let Ok(output) = Command::new("nvcc").arg("--version").output() {
            if let Ok(version_str) = String::from_utf8(output.stdout) {
                if let Some(version_line) = version_str.lines().find(|l| l.contains("release")) {
                    println!("cargo:warning=   CUDA version: {}", version_line.trim());
                }
            }
        }
    } else if check_command_exists("nvidia-smi", &[]) {
        println!("cargo:warning=NVIDIA GPU detected but CUDA toolkit not found");
        println!("cargo:warning=   Install CUDA 12.0+ for GPU acceleration");
        println!("cargo:warning=   See INSTALL.md for CUDA installation instructions");
    }
}

fn print_build_summary() {
    println!("cargo:warning=");
    println!("cargo:warning=Build Configuration:");
    println!("cargo:warning=   Platform: Linux");

    #[cfg(feature = "cuda")]
    println!("cargo:warning=   GPU: CUDA enabled");

    #[cfg(not(feature = "cuda"))]
    println!("cargo:warning=   GPU: CPU only (use --features cuda for GPU)");

    println!("cargo:warning=");
    println!("cargo:warning=Build dependency check complete");
    println!("cargo:warning=   For installation help, see: INSTALL.md");
    println!("cargo:warning=");
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
