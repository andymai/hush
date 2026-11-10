/// Build script for Hush
///
/// Checks for required system dependencies and provides helpful error messages
/// when dependencies are missing.
use std::process::Command;

fn main() {
    eprintln!();
    eprintln!("Hush Build System");
    eprintln!();

    check_cpu_floor();
    check_system_blas();
    link_cuda();
    check_linux_dependencies();
    check_gpu_support();
    print_build_summary();

    println!("cargo:rerun-if-changed=build.rs");
}

/// The CMake arguments that transcribe-cpp-sys passes to the ggml build.
fn cmake_args() -> Vec<String> {
    std::env::var("TRANSCRIBE_CMAKE_ARGS")
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_ascii_uppercase)
        .collect()
}

/// `HUSH_CPU_FLOOR` turns on the runtime CPU check in a prebuilt binary. It
/// is valid only together with the ggml flags that make the floor true.
fn check_cpu_floor() {
    println!("cargo:rerun-if-env-changed=HUSH_CPU_FLOOR");
    println!("cargo:rerun-if-env-changed=TRANSCRIBE_CMAKE_ARGS");
    let Ok(floor) = std::env::var("HUSH_CPU_FLOOR") else {
        return;
    };
    let var = |name: &str| std::env::var(name).unwrap_or_default();
    let problem = if floor != "x86-64-v3" {
        format!("HUSH_CPU_FLOOR={floor} is not known; the only floor is x86-64-v3")
    } else if var("CARGO_CFG_TARGET_ARCH") != "x86_64" {
        "HUSH_CPU_FLOOR=x86-64-v3 needs an x86_64 target".to_string()
    } else if !cmake_args().iter().any(|arg| arg == "-DGGML_NATIVE=OFF") {
        "HUSH_CPU_FLOOR needs -DGGML_NATIVE=OFF in TRANSCRIBE_CMAKE_ARGS, or ggml targets the CPU \
         of this machine"
            .to_string()
    } else if var("CARGO_CFG_TARGET_FEATURE")
        .split(',')
        .any(|feature| feature == "avx")
    {
        "HUSH_CPU_FLOOR needs Rust code without AVX, or the runtime CPU check cannot run"
            .to_string()
    } else {
        return;
    };
    println!("cargo:warning={problem}");
    eprintln!("\nERROR: {problem}\n");
    std::process::exit(1);
}

/// transcribe.cpp links a system BLAS when CMake finds one, and the binary
/// then needs that library at runtime. A prebuilt binary must not, so a
/// floor build fails, and other builds warn.
fn check_system_blas() {
    println!("cargo:rerun-if-env-changed=DEP_TRANSCRIBE_CPP_LIB_DIR");
    let Some(lib_dir) = std::env::var_os("DEP_TRANSCRIBE_CPP_LIB_DIR") else {
        return;
    };
    let manifest = std::path::Path::new(&lib_dir).join("transcribe-link.json");
    let links_blas = std::fs::read_to_string(&manifest)
        .is_ok_and(|text| text.to_ascii_lowercase().contains("blas"));
    if !links_blas {
        return;
    }
    let problem = "transcribe.cpp links the system BLAS, so the binary needs it at runtime. \
                   Set TRANSCRIBE_CMAKE_ARGS=-DTRANSCRIBE_USE_SYSTEM_BLAS=OFF, as the Makefile does";
    println!("cargo:warning={problem}");
    if std::env::var_os("HUSH_CPU_FLOOR").is_some() {
        eprintln!("\nERROR: {problem}\n");
        std::process::exit(1);
    }
}

/// The link manifest of transcribe-cpp-sys 0.4.1 does not name the CUDA
/// libraries that the static ggml-cuda archive needs, so the binary links
/// them itself. Link arguments come at the end of the link line, after the
/// archive, so the linker keeps them.
fn link_cuda() {
    println!("cargo:rerun-if-env-changed=CUDA_HOME");
    println!("cargo:rerun-if-env-changed=CUDA_PATH");
    if std::env::var_os("CARGO_FEATURE_CUDA").is_none() {
        return;
    }
    let home = std::env::var("CUDA_HOME")
        .or_else(|_| std::env::var("CUDA_PATH"))
        .unwrap_or_else(|_| "/usr/local/cuda".to_string());
    for dir in ["lib64", "lib64/stubs"] {
        println!("cargo:rustc-link-arg=-L{home}/{dir}");
    }
    for lib in ["cudart", "cublas", "cublasLt", "cuda"] {
        println!("cargo:rustc-link-arg=-l{lib}");
    }
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
