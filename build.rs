/// Build script for Hush
///
/// Checks for required system dependencies and provides helpful error messages
/// when dependencies are missing.
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where an installed binary finds libtranscribe and the ggml modules: a
/// private folder in the same prefix, such as /usr/lib/hush for /usr/bin/hush.
/// The same path serves the .deb, the .rpm, the tarball, and the AppImage.
const PACKAGE_RPATH: &str = "$ORIGIN/../lib/hush";

/// Where the binary in target/<profile> finds the bundle that `bundle_runtime`
/// makes. Tests and examples one folder below find it through
/// [`PACKAGE_RPATH`].
const BUILD_RPATH: &str = "$ORIGIN/lib/hush";

fn main() {
    eprintln!();
    eprintln!("Hush Build System");
    eprintln!();

    check_system_blas();
    bundle_runtime();
    check_linux_dependencies();
    check_gpu_support();
    print_build_summary();

    println!("cargo:rerun-if-changed=build.rs");
}

/// transcribe.cpp links a system BLAS when CMake finds one, and the binary
/// then needs that library at runtime. A package must not, so a
/// `HUSH_PACKAGE` build fails, and other builds warn.
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
    if std::env::var_os("HUSH_PACKAGE").is_some() {
        eprintln!("\nERROR: {problem}\n");
        std::process::exit(1);
    }
}

/// Copy libtranscribe, libggml, and the ggml backend modules to
/// target/<profile>/lib/hush, where packaging takes them, and give the binary
/// its library search path. `HUSH_PACKAGE` leaves out the path into target/,
/// so that a package binary looks only in its own prefix.
fn bundle_runtime() {
    println!("cargo:rerun-if-env-changed=HUSH_PACKAGE");
    println!("cargo:rerun-if-env-changed=DEP_TRANSCRIBE_CPP_RUNTIME_DIR");
    let Some(runtime) = std::env::var_os("DEP_TRANSCRIBE_CPP_RUNTIME_DIR").map(PathBuf::from)
    else {
        fail("transcribe-cpp built no shared library. Hush needs its dynamic-backends feature");
    };
    // A rebuild of transcribe.cpp in the same folder keeps the variable.
    println!("cargo:rerun-if-changed={}", runtime.display());
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default());
    // OUT_DIR is target/<profile>/build/hush-<hash>/out.
    let Some(profile) = out_dir.ancestors().nth(3) else {
        fail(&format!(
            "OUT_DIR {} is not in a target folder",
            out_dir.display()
        ));
    };
    let bundle = profile.join("lib").join("hush");
    if let Err(e) = copy_runtime(&runtime, &bundle) {
        fail(&format!(
            "could not copy the libraries from {} to {}: {e}",
            runtime.display(),
            bundle.display()
        ));
    }
    println!("cargo:rustc-link-arg=-Wl,-rpath,{PACKAGE_RPATH}");
    if std::env::var_os("HUSH_PACKAGE").is_none() {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{BUILD_RPATH}");
    }
}

/// The files that run: each library under the name that the loader asks for
/// (its soname, such as libggml.so.0), and each backend module. The link-time
/// names (libggml.so) and the fully versioned files stay out.
fn copy_runtime(from: &Path, to: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(to) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
        _ => {},
    }
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let symlink = entry.file_type()?.is_symlink();
        let module = !symlink && name.starts_with("lib") && name.ends_with(".so");
        let soname = symlink && name.starts_with("lib") && name.contains(".so.");
        if module || soname {
            std::fs::copy(entry.path(), to.join(name))?;
        }
    }
    Ok(())
}

fn fail(problem: &str) -> ! {
    println!("cargo:warning={problem}");
    eprintln!("\nERROR: {problem}\n");
    std::process::exit(1);
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
