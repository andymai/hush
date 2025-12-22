use anyhow::{Context, Result};
use std::io::{self, Write};
use std::path::Path;
use tracing::warn;

use crate::cli::SetupCommands;

/// Handle the setup command
///
/// Manages setup and configuration of Hush system components.
///
/// # Arguments
///
/// * `setup_command` - The specific setup subcommand to execute
///
/// # Subcommands
///
/// * `Uinput` - Configure UInput device permissions for text insertion
///   - `--quick`: Print quick setup commands
///   - `--auto-fix`: Attempt automatic setup (not yet implemented)
/// * `DiagnoseUinput` - Diagnose UInput permission issues
/// * `Audio` - Configure and test audio devices
///   - `--list`: List available audio input devices
///   - `--test <device>`: Test a specific audio device
/// * `Hotkeys` - Configure and test hotkey combinations
///   - `--list`: Show common hotkey combinations
///   - `--test <combination>`: Test a specific hotkey combination
/// * `Wizard` - Run interactive setup wizard
///   - `--auto`: Run wizard in automatic mode without prompts
///
/// # Examples
///
/// ```no_run
/// // Show quick UInput setup commands
/// handle_setup(SetupCommands::Uinput { quick: true, auto_fix: false }).await?;
///
/// // List available audio devices
/// handle_setup(SetupCommands::Audio { list: true, test: None }).await?;
///
/// // Run setup wizard
/// handle_setup(SetupCommands::Wizard { auto: false }).await?;
/// ```
///
/// # Workflow
///
/// - **UInput Setup**: Configures permissions for virtual keyboard device
/// - **Audio Setup**: Lists and tests audio input devices
/// - **Hotkey Setup**: Tests hotkey combinations for activation
/// - **Wizard**: Guides user through complete system configuration
pub async fn handle_setup(setup_command: SetupCommands) -> Result<()> {
    match setup_command {
        SetupCommands::Init { defaults, force } => init_config(defaults, force).await,
        SetupCommands::Uinput { quick, auto_fix } => {
            #[cfg(target_os = "linux")]
            {
                if quick {
                    print_uinput_quick_setup();
                } else if auto_fix {
                    auto_fix_uinput().await?;
                } else {
                    crate::text::print_uinput_setup_guidance();
                }
                Ok(())
            }
            #[cfg(not(target_os = "linux"))]
            {
                eprintln!("UInput setup is only available on Linux");
                Ok(())
            }
        },
        SetupCommands::DiagnoseUinput => {
            #[cfg(target_os = "linux")]
            {
                crate::text::diagnose_uinput_issues()
            }
            #[cfg(not(target_os = "linux"))]
            {
                eprintln!("UInput diagnostics are only available on Linux");
                Ok(())
            }
        },
        SetupCommands::Audio { list, test } => setup_audio(list, test).await,
        SetupCommands::Hotkeys { test, list } => setup_hotkeys(test, list).await,
        SetupCommands::Wizard { auto } => run_setup_wizard(auto).await,
    }
}

/// Print quick UInput setup commands
///
/// Displays the essential commands needed to configure UInput permissions.
/// These commands add the user to the input group and load the uinput kernel module.
fn print_uinput_quick_setup() {
    println!("🔧 Quick UInput Setup:");
    println!("sudo usermod -a -G input $USER");
    println!("sudo modprobe uinput");
    println!("echo 'uinput' | sudo tee /etc/modules-load.d/uinput.conf");
    println!("# Then log out and log back in");
}

/// Attempt automatic UInput setup
///
/// **Note**: This functionality is not yet implemented.
/// Users should run manual setup commands instead.
async fn auto_fix_uinput() -> Result<()> {
    warn!("Auto-fix functionality not yet implemented");
    println!("🚧 Auto-fix is not yet implemented. Please run 'hush setup uinput' for manual instructions.");
    Ok(())
}

/// Setup and test audio devices
///
/// # Arguments
///
/// * `list` - If true, list all available audio input devices
/// * `test` - Optional device name to test
///
/// # Workflow
///
/// 1. If `list` is true, enumerate and display all audio input devices
/// 2. If `test` is provided, attempt to open and verify the device
async fn setup_audio(list: bool, test: Option<String>) -> Result<()> {
    if list {
        println!("🎵 Available Audio Devices:");
        // Use existing functionality
        if let Ok(devices) = crate::AudioCapture::list_devices() {
            for (i, device) in devices.iter().enumerate() {
                println!("  {}. {}", i + 1, device);
            }
        } else {
            println!("❌ Failed to list audio devices");
        }
    }

    if let Some(device_name) = test {
        println!("🎤 Testing audio device: {}", device_name);
        match crate::AudioCapture::new(Some(&device_name)) {
            Ok(capture) => {
                println!("✅ Device '{}' is available", capture.get_device_name());
            },
            Err(e) => {
                println!("❌ Device test failed: {}", e);
            },
        }
    }

    Ok(())
}

/// Setup and test hotkey combinations
///
/// # Arguments
///
/// * `test` - Optional hotkey combination to test (e.g., "Ctrl+Shift+Space")
/// * `list` - If true, display common hotkey combinations
///
/// # Workflow
///
/// 1. If `list` is true, show examples of common hotkey combinations
/// 2. If `test` is provided, verify the hotkey combination is valid
async fn setup_hotkeys(test: Option<String>, list: bool) -> Result<()> {
    if list {
        println!("⌨️ Common Hotkey Combinations:");
        println!("  • Ctrl+Shift+Space (default)");
        println!("  • Ctrl+Alt+Space");
        println!("  • F12");
        println!("  • Ctrl+F12");
        println!("  • Alt+Space");
    }

    if let Some(combination) = test {
        println!("🎯 Testing hotkey combination: {}", combination);
        // Test the hotkey combination
        match crate::hotkey::HotkeyManager::new(&combination) {
            Ok((manager, _receiver)) => {
                println!("✅ Hotkey combination '{}' is valid", combination);
                drop(manager);
            },
            Err(e) => {
                println!("❌ Hotkey test failed: {}", e);
            },
        }
    }

    Ok(())
}

/// Run interactive setup wizard
///
/// Guides the user through complete Hush system configuration.
///
/// # Arguments
///
/// * `auto` - If true, run in automatic mode without user prompts
///
/// # Workflow
///
/// 1. **UInput Setup**: Configure text insertion permissions
/// 2. **Model Download**: Download recommended Whisper model
/// 3. **System Test**: Verify all components are working
///
/// The wizard will pause for user confirmation at each step unless
/// running in automatic mode.
async fn run_setup_wizard(auto: bool) -> Result<()> {
    println!("🧙 Hush Setup Wizard");
    println!("This wizard will help you configure Hush for optimal performance.\n");

    if !auto {
        println!("Press Enter to continue, or Ctrl+C to cancel...");
        std::io::stdin()
            .read_line(&mut String::new())
            .context("Failed to read user input")?;
    }

    // Step 1: UInput setup
    println!("📋 Step 1: UInput Setup");
    crate::text::print_uinput_setup_guidance();

    if !auto {
        println!("\nHave you completed the UInput setup? (y/N)");
        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .context("Failed to read user input")?;
        if !input.trim().to_lowercase().starts_with('y') {
            println!("⚠️ Please complete UInput setup before continuing.");
            return Ok(());
        }
    }

    // Step 2: Model download
    println!("\n📋 Step 2: Model Download");
    println!("Downloading recommended model (base)...");
    super::models::download_model("base", false).await?;

    // Step 3: Test systems
    println!("\n📋 Step 3: System Test");
    crate::cli::dispatcher::run_all_tests(false, None).await?;

    println!("\n✅ Setup wizard completed!");
    println!("You can now use 'hush listen' to begin voice-to-text.");

    Ok(())
}

/// Initialize Hush configuration file
///
/// Creates config/default.toml from the example template, optionally
/// prompting the user for key settings.
///
/// # Arguments
///
/// * `defaults` - If true, use defaults without prompting
/// * `force` - If true, overwrite existing config file
async fn init_config(defaults: bool, force: bool) -> Result<()> {
    let config_path = Path::new("config/default.toml");
    let example_path = Path::new("config/default.toml.example");

    println!("🔧 Hush Configuration Setup\n");

    // Check if config already exists
    if config_path.exists() && !force {
        println!(
            "⚠️  Configuration file already exists: {}",
            config_path.display()
        );
        print!("Overwrite? [y/N] ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled. Use --force to overwrite.");
            return Ok(());
        }
    }

    // Verify example exists (for reference)
    if !example_path.exists() {
        warn!("config/default.toml.example not found, using built-in defaults");
    }

    // Detect system capabilities
    let has_cuda = detect_cuda();
    let models_dir = get_models_dir();

    // Collect settings
    let (model_size, use_cuda, hotkey) = if defaults {
        ("base".to_string(), has_cuda, "Ctrl+Shift+Space".to_string())
    } else {
        collect_user_settings(has_cuda)?
    };

    // Generate the config content
    let model_path = models_dir.join(format!("ggml-{}.bin", model_size));
    let config_content = format!(
        r#"# Hush Configuration
# Generated by: hush setup init

[audio]
sample_rate = 16000
channels = 1
buffer_size = 1024

[transcription]
model_path = "{}"
model_size = "{}"
language = "en"
use_cuda = {}
beam_size = 5
no_speech_threshold = 0.6

[hotkey]
enabled = true
combination = "{}"

[feedback]
audio_enabled = false
"#,
        model_path.display(),
        model_size,
        use_cuda,
        hotkey
    );

    // Ensure config directory exists
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create config directory")?;
    }

    // Write the config file
    std::fs::write(config_path, &config_content).context("Failed to write config file")?;

    println!("\n✅ Configuration created: {}", config_path.display());
    println!("\nSettings:");
    println!("   Model: {} ({})", model_size, model_path.display());
    println!(
        "   CUDA:  {}",
        if use_cuda { "enabled" } else { "disabled" }
    );
    println!("   Hotkey: {}", hotkey);

    // Check if model exists
    if !model_path.exists() {
        println!("\n📥 Model not found. Download it with:");
        println!("   ./hush models download {}", model_size);
    }

    println!("\n🚀 Next steps:");
    println!(
        "   1. Download a model:  ./hush models download {}",
        model_size
    );
    println!("   2. Setup permissions: ./hush setup uinput --quick");
    println!("   3. Start listening:   ./hush listen");

    Ok(())
}

/// Collect user settings interactively
fn collect_user_settings(has_cuda: bool) -> Result<(String, bool, String)> {
    // Model size
    println!("📦 Select Whisper model size:");
    println!("   1. tiny   (75 MB)  - Fastest, lower accuracy");
    println!("   2. base   (145 MB) - Recommended balance");
    println!("   3. small  (466 MB) - Better accuracy");
    println!("   4. medium (1.5 GB) - High accuracy");
    println!("   5. large  (2.9 GB) - Best accuracy");
    print!("\nChoice [2]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let model_size = match input.trim() {
        "1" | "tiny" => "tiny",
        "" | "2" | "base" => "base",
        "3" | "small" => "small",
        "4" | "medium" => "medium",
        "5" | "large" => "large",
        _ => "base",
    }
    .to_string();

    // CUDA
    let use_cuda = if has_cuda {
        println!("\n🎮 CUDA GPU detected. Enable GPU acceleration?");
        print!("Use CUDA? [Y/n]: ");
        io::stdout().flush()?;

        input.clear();
        io::stdin().read_line(&mut input)?;
        !input.trim().eq_ignore_ascii_case("n")
    } else {
        println!("\n💻 No CUDA GPU detected. Using CPU mode.");
        false
    };

    // Hotkey
    println!("\n⌨️  Configure hotkey (press and hold to record):");
    println!("   Common options: Ctrl+Shift+Space, Ctrl+Alt+Space, F12");
    print!("Hotkey [Ctrl+Shift+Space]: ");
    io::stdout().flush()?;

    input.clear();
    io::stdin().read_line(&mut input)?;
    let hotkey = if input.trim().is_empty() {
        "Ctrl+Shift+Space".to_string()
    } else {
        input.trim().to_string()
    };

    Ok((model_size, use_cuda, hotkey))
}

/// Detect if CUDA is available
fn detect_cuda() -> bool {
    // Check for nvidia-smi
    std::process::Command::new("nvidia-smi")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Get the models directory path
fn get_models_dir() -> std::path::PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(".cache"))
        .join("hush")
        .join("models")
}
