use anyhow::{Context, Result};
use std::io::{self, Write};

use crate::cli::SetupCommands;
use crate::config::settings::TranscriptionConfig;
use crate::gui::{megabytes, size_key, MODEL_ORDER};
use crate::permissions;
use crate::transcription::models::{catalogue_entry, recommended_model};
use crate::transcription::{GpuAvailability, ModelManager, ModelSize};
use crate::Config;

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
/// # use hush::cli::commands::handle_setup;
/// # use hush::cli::SetupCommands;
/// # async fn example() -> anyhow::Result<()> {
/// // Grant keyboard and uinput access through the udev rule
/// handle_setup(SetupCommands::Permissions { check: false, print: false }).await?;
///
/// // List available audio devices
/// handle_setup(SetupCommands::Audio { list: true, test: None }).await?;
///
/// // Run setup wizard
/// handle_setup(SetupCommands::Wizard { auto: false }).await?;
/// # Ok(())
/// # }
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
        SetupCommands::Permissions { check, print } => setup_permissions(check, print),
        SetupCommands::Uinput { quick, auto_fix } => {
            #[cfg(target_os = "linux")]
            {
                if quick {
                    print_uinput_quick_setup();
                } else if auto_fix {
                    setup_permissions(false, false)?;
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

/// Print the commands `hush setup permissions` runs as root
fn print_uinput_quick_setup() {
    println!("🔧 Run as root (or run `hush setup permissions` on the host):");
    println!();
    print!("{}", permissions::install_script());
}

/// Install, print, or check the udev rule that grants device access.
fn setup_permissions(check: bool, print: bool) -> Result<()> {
    let status = permissions::status();

    if print {
        print_uinput_quick_setup();
        return Ok(());
    }

    permissions::print_status(&status);
    if check {
        return Ok(());
    }

    if status.ready() && status.rule_installed {
        println!("\n✅ Permissions are already set up.");
        return Ok(());
    }

    if status.in_container {
        println!("\n⚠️  This shell is inside a container, so the rule must go on the host.");
        println!("   Run `hush setup permissions` from a host terminal, or as root on the host:");
        println!();
        print!("{}", permissions::install_script());
        return Ok(());
    }

    let elevation = permissions::preferred_elevation()
        .ok_or_else(|| anyhow::anyhow!("Neither pkexec nor sudo is available; run `hush setup permissions --print` and apply the commands as root"))?;
    println!(
        "\n🔐 Installing {} (one {} prompt)...",
        permissions::UDEV_RULE_PATH,
        match elevation {
            permissions::Elevation::Pkexec => "polkit",
            permissions::Elevation::Sudo => "sudo",
        }
    );
    permissions::install(elevation)?;

    let after = permissions::status();
    permissions::print_status(&after);
    if after.ready() {
        println!("\n✅ Hotkeys and text insertion are ready. No logout needed.");
    } else {
        println!("\n⚠️  The rule is installed but this session did not receive access yet.");
        println!("   Log out and back in, then run `hush setup permissions --check`.");
        println!("   If it still fails, your session may lack a seat (some SSH or nested setups);");
        println!("   `sudo usermod -a -G input $USER` and a re-login grants access group-wide.");
    }
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
        println!("  • RightAlt (default)");
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
/// 2. **Model Download**: Keep the configured model when it is there.
///    Otherwise download the model that [`recommended_model`] suggests for
///    this machine, and make it the active model
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

    println!("\n📋 Step 2: Model Download");
    let suggested = recommended_model();
    let transcription = Config::load()
        .context("Failed to load the configuration")?
        .transcription;
    let manager = ModelManager::new(crate::config::paths::models_dir())?;
    let downloaded = |size: ModelSize| {
        manager
            .get_model_info(&size)
            .and_then(|info| manager.valid_path(info))
            .is_some()
    };
    let step = wizard_model(&transcription, downloaded, suggested);
    println!("{}", wizard_model_message(&step, suggested));
    if let WizardModel::Install(size) = step {
        super::models::set_model(size_key(size)).await?;
    }

    // Step 3: Test systems
    println!("\n📋 Step 3: System Test");
    crate::cli::dispatcher::run_all_tests(false, None).await?;

    println!("\n✅ Setup wizard completed!");
    println!("You can now use 'hush listen' to begin voice-to-text.");

    Ok(())
}

/// Initialize Hush configuration file
///
/// Writes the user configuration file, optionally prompting for key settings.
///
/// # Arguments
///
/// * `defaults` - If true, write the built-in defaults without prompting.
///   The model is base, not the model that [`recommended_model`] suggests
/// * `force` - If true, overwrite existing config file
async fn init_config(defaults: bool, force: bool) -> Result<()> {
    let config_path = Config::path();

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

    let gpu = GpuAvailability::detect();

    let (model, use_gpu, hotkey) = if defaults {
        (ModelSize::Base, gpu.available, "RightAlt".to_string())
    } else {
        collect_user_settings(gpu, recommended_model())?
    };
    let model_size = size_key(model);

    let mut config = Config::defaults()?;
    config.transcription.select_model(model);
    config.transcription.use_gpu = use_gpu;
    config.hotkey.combination = hotkey.clone();
    config.save().context("Failed to write config file")?;

    let model_path = config.transcription.model_path();

    println!("\n✅ Configuration created: {}", config_path.display());
    println!("\nSettings:");
    println!("   Model: {} ({})", model_size, model_path.display());
    println!(
        "   GPU:   {}",
        if use_gpu {
            gpu.device_name.as_str()
        } else {
            "disabled"
        }
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
    println!("   2. Setup permissions: ./hush setup permissions");
    println!("   3. Start listening:   ./hush listen");

    Ok(())
}

/// What the wizard does about the speech model.
#[derive(Debug, PartialEq, Eq)]
enum WizardModel {
    /// The configured model is there and stays: its name or its path.
    Keep(String),
    /// The configured model is missing, so the wizard downloads this model
    /// and makes it the active model.
    Install(ModelSize),
}

/// A `model_path` set by hand, or a catalogue model whose file `downloaded`
/// finds valid, stays. Otherwise the wizard installs `suggested`.
fn wizard_model(
    transcription: &TranscriptionConfig,
    downloaded: impl Fn(ModelSize) -> bool,
    suggested: ModelSize,
) -> WizardModel {
    if let Some(path) = &transcription.model_path {
        return WizardModel::Keep(path.display().to_string());
    }
    match transcription.model_size.parse::<ModelSize>() {
        Ok(size) if downloaded(size) => WizardModel::Keep(size_key(size).to_string()),
        _ => WizardModel::Install(suggested),
    }
}

fn wizard_model_message(step: &WizardModel, suggested: ModelSize) -> String {
    let suggested = size_key(suggested);
    match step {
        WizardModel::Keep(model) if model == suggested => format!(
            "Hush keeps the configured model, {}, which is also the suggestion for this machine.",
            model
        ),
        WizardModel::Keep(model) => format!(
            "Hush keeps the configured model, {}. Hush suggests {} for this machine. \
             To use it, run: hush models set {}",
            model, suggested, suggested
        ),
        WizardModel::Install(size) => format!(
            "The configured model is not downloaded. Hush downloads {}, the suggestion for \
             this machine, and makes it the active model.",
            size_key(*size)
        ),
    }
}

/// Collect user settings interactively. `recommended` is the default model.
fn collect_user_settings(
    gpu: &GpuAvailability,
    recommended: ModelSize,
) -> Result<(ModelSize, bool, String)> {
    print!("{}", model_menu(recommended));
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let model = pick_model(&input, recommended).unwrap_or_else(|| {
        println!(
            "'{}' is not on the list. Hush uses {}.",
            input.trim(),
            size_key(recommended)
        );
        recommended
    });

    let use_gpu = if gpu.available {
        println!(
            "\n🎮 GPU detected: {}. Enable GPU acceleration?",
            gpu.device_name
        );
        print!("Use GPU? [Y/n]: ");
        io::stdout().flush()?;

        input.clear();
        io::stdin().read_line(&mut input)?;
        !input.trim().eq_ignore_ascii_case("n")
    } else {
        println!("\n💻 No GPU backend available. Using CPU mode.");
        false
    };

    // Hotkey
    println!("\n⌨️  Configure hotkey (press and hold to record):");
    println!("   Common options: RightAlt, RightCtrl, F13, Ctrl+Shift+Space");
    print!("Hotkey [RightAlt]: ");
    io::stdout().flush()?;

    input.clear();
    io::stdin().read_line(&mut input)?;
    let hotkey = if input.trim().is_empty() {
        "RightAlt".to_string()
    } else {
        input.trim().to_string()
    };

    Ok((model, use_gpu, hotkey))
}

/// The model list of `hush setup init`: the models of the Speech page, in
/// its order, with download sizes from the catalogue. `recommended` is the
/// default.
fn model_menu(recommended: ModelSize) -> String {
    let mut menu = String::from("📦 Select a speech model:\n");
    for (number, size) in MODEL_ORDER.into_iter().enumerate() {
        let bytes = catalogue_entry(size)
            .map(|info| megabytes(info.expected_size))
            .unwrap_or_default();
        let suggested = if size == recommended {
            " (suggested for this machine)"
        } else {
            ""
        };
        menu.push_str(&format!(
            "   {}. {:<11} {:>7}  {}{}\n",
            number + 1,
            size_key(size),
            bytes,
            model_note(size),
            suggested
        ));
    }
    menu.push_str(&format!("\nChoice [{}]: ", size_key(recommended)));
    menu
}

fn model_note(size: ModelSize) -> &'static str {
    match size {
        ModelSize::Tiny => "fastest, least accurate",
        ModelSize::Base => "fast",
        ModelSize::Small => "more accurate",
        ModelSize::Medium => "high accuracy",
        ModelSize::Large | ModelSize::LargeV2 | ModelSize::LargeV3 => "most accurate, slowest",
        ModelSize::ParakeetV3 => {
            "opt-in: 25 European languages only, picks the language itself, reads no prompt"
        },
    }
}

/// The model for an answer to [`model_menu`]: a number from the list, a
/// model name, or nothing for `recommended`. `None` when the answer is
/// neither.
fn pick_model(input: &str, recommended: ModelSize) -> Option<ModelSize> {
    let input = input.trim();
    if input.is_empty() {
        return Some(recommended);
    }
    match input.parse::<usize>() {
        Ok(number) => number
            .checked_sub(1)
            .and_then(|index| MODEL_ORDER.get(index))
            .copied(),
        Err(_) => input.parse().ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_model_is_the_one_for_this_machine() {
        for recommended in [
            ModelSize::Tiny,
            ModelSize::Base,
            ModelSize::Medium,
            recommended_model(),
        ] {
            assert_eq!(pick_model("", recommended), Some(recommended));
            assert_eq!(pick_model("  \n", recommended), Some(recommended));

            let menu = model_menu(recommended);
            assert!(
                menu.ends_with(&format!("Choice [{}]: ", size_key(recommended))),
                "{menu}"
            );
            let suggested: Vec<&str> = menu
                .lines()
                .filter(|line| line.contains("suggested for this machine"))
                .collect();
            assert_eq!(suggested.len(), 1, "{menu}");
            assert!(
                suggested[0].contains(&format!(" {} ", size_key(recommended))),
                "{menu}"
            );
        }
    }

    fn transcription(model_size: &str, model_path: Option<&str>) -> TranscriptionConfig {
        let mut transcription = Config::defaults().unwrap().transcription;
        transcription.model_size = model_size.to_string();
        transcription.model_path = model_path.map(Into::into);
        transcription
    }

    #[test]
    fn the_wizard_keeps_a_chosen_model_and_only_hints_the_suggestion() {
        let only_small = |size: ModelSize| size == ModelSize::Small;

        let step = wizard_model(&transcription("small", None), only_small, ModelSize::Medium);
        assert_eq!(step, WizardModel::Keep("small".to_string()));
        let message = wizard_model_message(&step, ModelSize::Medium);
        assert!(
            message.contains("keeps the configured model, small"),
            "{message}"
        );
        assert!(message.contains("hush models set medium"), "{message}");
        assert!(!message.contains("downloads"), "{message}");

        let by_hand = transcription("medium", Some("/models/custom.bin"));
        let step = wizard_model(&by_hand, |_| false, ModelSize::Medium);
        assert_eq!(step, WizardModel::Keep("/models/custom.bin".to_string()));
        let message = wizard_model_message(&step, ModelSize::Medium);
        assert!(message.contains("/models/custom.bin"), "{message}");
        assert!(message.contains("hush models set medium"), "{message}");

        let step = wizard_model(&transcription("medium", None), |_| true, ModelSize::Medium);
        let message = wizard_model_message(&step, ModelSize::Medium);
        assert!(message.contains("also the suggestion"), "{message}");
        assert!(!message.contains("hush models set"), "{message}");
    }

    #[test]
    fn the_wizard_installs_the_suggestion_only_for_a_missing_model() {
        for model_size in ["base", "large-v3", "not-a-model"] {
            let step = wizard_model(
                &transcription(model_size, None),
                |_| false,
                ModelSize::Small,
            );
            assert_eq!(step, WizardModel::Install(ModelSize::Small), "{model_size}");
        }
        let step = wizard_model(&transcription("base", None), |_| false, recommended_model());
        let message = wizard_model_message(&step, recommended_model());
        assert!(
            message.contains(&format!("downloads {},", size_key(recommended_model()))),
            "{message}"
        );
    }

    #[test]
    fn a_number_or_a_name_picks_a_model() {
        assert_eq!(pick_model("1", ModelSize::Medium), Some(ModelSize::Tiny));
        assert_eq!(pick_model("2\n", ModelSize::Medium), Some(ModelSize::Base));
        assert_eq!(
            pick_model("8", ModelSize::Medium),
            Some(ModelSize::ParakeetV3)
        );
        assert_eq!(
            pick_model("large-v3", ModelSize::Tiny),
            Some(ModelSize::LargeV3)
        );
        assert_eq!(
            pick_model("parakeet-v3", ModelSize::Tiny),
            Some(ModelSize::ParakeetV3)
        );
        for bad in ["0", "9", "huge", "-1"] {
            assert_eq!(pick_model(bad, ModelSize::Base), None, "{bad}");
        }
    }

    #[test]
    fn the_menu_sizes_come_from_the_catalogue() {
        let menu = model_menu(ModelSize::Base);
        for (number, size) in MODEL_ORDER.into_iter().enumerate() {
            let info = catalogue_entry(size).expect("every listed model is in the catalogue");
            let line = menu
                .lines()
                .find(|line| line.trim_start().starts_with(&format!("{}. ", number + 1)))
                .expect("one line for each model");
            assert!(line.contains(size_key(size)), "{line}");
            assert!(line.contains(&megabytes(info.expected_size)), "{line}");
        }
        assert!(menu.contains(" 78 MB"), "tiny is 77.7 MB:\n{menu}");
        assert!(menu.contains("740 MB"), "parakeet-v3 is 739.5 MB:\n{menu}");
        for stale in ["75 MB", "145 MB", "466 MB", "2.9 GB"] {
            assert!(!menu.contains(stale), "{stale} in:\n{menu}");
        }
    }

    #[test]
    fn parakeet_is_listed_as_opt_in_with_its_limits() {
        let menu = model_menu(ModelSize::Base);
        let line = menu
            .lines()
            .find(|line| line.contains("parakeet-v3"))
            .expect("a parakeet-v3 line");
        assert!(line.contains("opt-in"), "{line}");
        assert!(line.contains("25 European languages"), "{line}");
    }
}
