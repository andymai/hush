use anyhow::{Context, Result};
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
    println!("You can now use 'hush start' to begin voice-to-text.");

    Ok(())
}
