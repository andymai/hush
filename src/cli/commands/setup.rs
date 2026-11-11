use anyhow::{Context, Result};
use std::io::{self, Write};

use crate::audio::wake::{self, Plan, Sleep, Undone};
use crate::cli::SetupCommands;
use crate::config::settings::TranscriptionConfig;
use crate::gui::{megabytes, size_key, MODEL_ORDER};
use crate::permissions;
use crate::transcription::models::{catalogue_entry, recommended_model, recommended_model_for};
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
/// * `Mic` - Keep the microphone awake between takes with a WirePlumber rule
///   - `--undo`: Remove the rule
///   - `--yes`: Restart WirePlumber without a question
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
        SetupCommands::Mic { undo, yes } => setup_mic(undo, yes).await,
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
    #[cfg(target_os = "linux")]
    if crate::portal::InputMode::current().is_portal() {
        println!("Hush uses the desktop portals, so it needs no device access.");
        println!("The desktop asks you once for keyboard access, at the first dictation.");
        println!("To use evdev and uinput instead, set backend = \"evdev\" under [input].");
        return Ok(());
    }
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

/// What a restart of WirePlumber does to the sound, before the question.
const RESTART_WARNING: &str =
    "WirePlumber restarts to read the change, so all sound stops for about a second.";

/// Write or remove the WirePlumber rule that keeps the microphone of Hush
/// awake between takes, restart WirePlumber, and make sure that the change
/// applies.
async fn setup_mic(undo: bool, yes: bool) -> Result<()> {
    if crate::portal::sandboxed() {
        println!("Nothing changed: {}.", wake::SANDBOX_NOTE);
        return Ok(());
    }
    let config_dir = wake::config_dir()?;
    let config = Config::load();
    if undo {
        let device = config.ok().and_then(|config| config.audio.device);
        let probe = wake::look(device.as_deref()).await;
        return undo_mic(&config_dir, probe, yes).await;
    }
    let config = config
        .context("The configuration does not load, so Hush cannot tell which microphone it uses")?;
    let probe = wake::look(config.audio.device.as_deref()).await;
    let change = match wake::plan(&wake::Host, &config_dir, probe).await? {
        Plan::Awake { source, by_hush } => {
            let by = if by_hush {
                "the rule of Hush. `hush setup mic --undo` lets it sleep again".to_string()
            } else {
                "a WirePlumber rule that Hush did not write, so Hush changes nothing".to_string()
            };
            println!(
                "✅ {} stays awake between takes, because of {}.",
                source.label, by
            );
            return Ok(());
        },
        Plan::Write(change) => change,
    };
    let source = &change.source;
    if let Sleep::After(seconds) = source.suspend() {
        println!(
            "🎤 WirePlumber suspends {} after {} s without a take, and the next take waits while it wakes.",
            source.label, seconds
        );
    }
    println!("   Node: {}", source.name);
    println!();
    println!(
        "Hush writes this rule for {} to {}:",
        change.format.name(),
        change.path.display()
    );
    println!();
    print!("{}", change.text);
    for stale in &change.stale {
        println!("Hush removes its old rule {}.", stale.display());
    }
    println!();
    println!("The rule keeps only this microphone awake. No app records while it is awake.");
    if !yes && !ask(RESTART_WARNING)? {
        println!("Nothing changed. Add --yes to skip the question.");
        return Ok(());
    }
    println!("WirePlumber restarts…");
    let after = wake::apply(&wake::Host, &change).await?;
    println!();
    println!("✅ {} stays awake between takes now.", after.label);
    println!("   `hush setup mic --undo` lets it sleep again.");
    Ok(())
}

async fn undo_mic(config_dir: &std::path::Path, probe: wake::Probe, yes: bool) -> Result<()> {
    let files = wake::rules_in(config_dir);
    if files.is_empty() {
        println!("Hush wrote no WirePlumber rule, so nothing changed.");
        return Ok(());
    }
    for file in &files {
        println!("Hush removes {}.", file.display());
    }
    if !yes && !ask(RESTART_WARNING)? {
        println!("Nothing changed. Add --yes to skip the question.");
        return Ok(());
    }
    println!("WirePlumber restarts…");
    match wake::undo(&wake::Host, config_dir, probe).await? {
        Undone::Nothing => println!("Hush wrote no WirePlumber rule, so nothing changed."),
        Undone::Removed { source: None, .. } => {
            println!("✅ The rule of Hush is gone, and WirePlumber restarted.")
        },
        Undone::Removed {
            source: Some(source),
            ..
        } => match source.suspend() {
            Sleep::After(seconds) => println!(
                "✅ {} sleeps again after {} s without a take.",
                source.label, seconds
            ),
            Sleep::Never => println!(
                "✅ The rule of Hush is gone. {} still stays awake, because another WirePlumber rule keeps it awake.",
                source.label
            ),
        },
    }
    Ok(())
}

/// Print `warning`, ask whether to go on, and read the answer. Only `y` or
/// `yes` goes on; an empty answer or the end of the input does not.
fn ask(warning: &str) -> Result<bool> {
    println!("{}", warning);
    print!("Go on? [y/N] ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("Failed to read the answer")?;
    Ok(is_yes(&input))
}

fn is_yes(answer: &str) -> bool {
    matches!(answer.trim().to_lowercase().as_str(), "y" | "yes")
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
/// 2. **Model Download**: Keep a valid configured model, and download a
///    configured model again when it is missing or fails its check. Only
///    without a config file, download the model that [`recommended_model`]
///    suggests and make it the active model. A cloud backend needs no model
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
    let step = wizard_model(
        &transcription,
        Config::path().exists(),
        downloaded,
        suggested,
    );
    println!("{}", wizard_model_message(&step, suggested));
    match step {
        WizardModel::Repair(size) => {
            super::models::download_model(size_key(size), false).await?;
        },
        WizardModel::Install(model) | WizardModel::Replace { model, .. } => {
            super::models::set_model(size_key(model)).await?;
        },
        WizardModel::Cloud(_) | WizardModel::KeepPath(_) | WizardModel::Keep(_) => {},
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
        collect_user_settings(gpu)?
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
    /// A service transcribes the speech, so no local model is needed.
    Cloud(&'static str),
    /// The file that `model_path` names stays.
    KeepPath(String),
    /// The configured catalogue model is valid and stays.
    Keep(ModelSize),
    /// The configured catalogue model is missing or fails its size or
    /// SHA-256 check, so the wizard downloads it again.
    Repair(ModelSize),
    /// No config file names a model, so the wizard downloads this one and
    /// makes it the active model.
    Install(ModelSize),
    /// The configured name is not a catalogue model, so the wizard installs
    /// the suggestion in its place.
    Replace { unknown: String, model: ModelSize },
}

/// What the wizard does for `transcription`. `config_exists` says whether
/// the user has a config file, and `downloaded` whether a catalogue model's
/// file is there and valid.
fn wizard_model(
    transcription: &TranscriptionConfig,
    config_exists: bool,
    downloaded: impl Fn(ModelSize) -> bool,
    suggested: ModelSize,
) -> WizardModel {
    if transcription.backend.is_cloud() {
        return WizardModel::Cloud(transcription.backend.name());
    }
    if let Some(path) = &transcription.model_path {
        return WizardModel::KeepPath(path.display().to_string());
    }
    if !config_exists {
        return WizardModel::Install(suggested);
    }
    match transcription.model_size.parse::<ModelSize>() {
        Ok(size) if downloaded(size) => WizardModel::Keep(size),
        Ok(size) => WizardModel::Repair(size),
        Err(_) => WizardModel::Replace {
            unknown: transcription.model_size.clone(),
            model: suggested,
        },
    }
}

fn wizard_model_message(step: &WizardModel, suggested: ModelSize) -> String {
    let hint = |kept: Option<ModelSize>| {
        if kept == Some(suggested) {
            " It is also the suggestion for this machine.".to_string()
        } else {
            format!(
                " Hush suggests {0} for this machine. To use it, run: hush models set {0}",
                size_key(suggested)
            )
        }
    };
    match step {
        WizardModel::Cloud(name) => format!(
            "{} transcribes the speech (transcription.backend), so the wizard downloads no \
             local model.",
            name
        ),
        WizardModel::KeepPath(path) => format!(
            "Hush keeps the model file that model_path names: {}.{}",
            path,
            hint(None)
        ),
        WizardModel::Keep(size) => format!(
            "Hush keeps the configured model, {}.{}",
            size_key(*size),
            hint(Some(*size))
        ),
        WizardModel::Repair(size) => format!(
            "The configured model, {}, is missing or fails its check, so Hush downloads it \
             again.{}",
            size_key(*size),
            hint(Some(*size))
        ),
        WizardModel::Install(size) => format!(
            "No configuration file names a model. Hush downloads {}, the suggestion for this \
             machine, and makes it the active model.",
            size_key(*size)
        ),
        WizardModel::Replace { unknown, model } => format!(
            "'{}' is not a model that Hush knows. Hush downloads {}, the suggestion for this \
             machine, and makes it the active model.",
            unknown,
            size_key(*model)
        ),
    }
}

/// Collect user settings interactively. The GPU answer comes first, so the
/// default model suits the device that runs it.
fn collect_user_settings(gpu: &GpuAvailability) -> Result<(ModelSize, bool, String)> {
    let mut input = String::new();
    let use_gpu = if gpu.available {
        println!(
            "🎮 GPU detected: {}. Enable GPU acceleration?",
            gpu.device_name
        );
        print!("Use GPU? [Y/n]: ");
        io::stdout().flush()?;

        io::stdin().read_line(&mut input)?;
        !input.trim().eq_ignore_ascii_case("n")
    } else {
        println!("💻 No GPU backend available. Using CPU mode.");
        false
    };

    let recommended = recommended_model_for(use_gpu);
    println!();
    print!("{}", model_menu(recommended));
    io::stdout().flush()?;

    input.clear();
    io::stdin().read_line(&mut input)?;
    let model = pick_model(&input, recommended).unwrap_or_else(|| {
        println!(
            "'{}' is not on the list. Hush uses {}.",
            input.trim(),
            size_key(recommended)
        );
        recommended
    });

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
    fn the_wizard_keeps_a_valid_chosen_model_and_only_hints_the_suggestion() {
        let only_small = |size: ModelSize| size == ModelSize::Small;

        let step = wizard_model(
            &transcription("small", None),
            true,
            only_small,
            ModelSize::Medium,
        );
        assert_eq!(step, WizardModel::Keep(ModelSize::Small));
        let message = wizard_model_message(&step, ModelSize::Medium);
        assert!(
            message.contains("keeps the configured model, small"),
            "{message}"
        );
        assert!(message.contains("hush models set medium"), "{message}");
        assert!(!message.contains("downloads"), "{message}");

        let by_hand = transcription("medium", Some("/models/custom.bin"));
        let step = wizard_model(&by_hand, true, |_| false, ModelSize::Medium);
        assert_eq!(
            step,
            WizardModel::KeepPath("/models/custom.bin".to_string())
        );
        let message = wizard_model_message(&step, ModelSize::Medium);
        assert!(message.contains("/models/custom.bin"), "{message}");
        assert!(message.contains("hush models set medium"), "{message}");

        let step = wizard_model(
            &transcription("medium", None),
            true,
            |_| true,
            ModelSize::Medium,
        );
        let message = wizard_model_message(&step, ModelSize::Medium);
        assert!(message.contains("also the suggestion"), "{message}");
        assert!(!message.contains("hush models set"), "{message}");
    }

    #[test]
    fn the_wizard_downloads_a_chosen_model_again_when_it_is_missing() {
        for model_size in ["large-v3", "parakeet-v3", "tiny"] {
            let step = wizard_model(
                &transcription(model_size, None),
                true,
                |_| false,
                ModelSize::Medium,
            );
            let size = model_size.parse().unwrap();
            assert_eq!(step, WizardModel::Repair(size), "{model_size}");
            let message = wizard_model_message(&step, ModelSize::Medium);
            assert!(
                message.contains(&format!("{model_size}, is missing")),
                "{message}"
            );
            assert!(message.contains("hush models set medium"), "{message}");
        }
    }

    #[test]
    fn the_wizard_installs_the_suggestion_only_without_a_config_file() {
        for model_size in ["base", "large-v3"] {
            for downloaded in [true, false] {
                let step = wizard_model(
                    &transcription(model_size, None),
                    false,
                    |_| downloaded,
                    ModelSize::Small,
                );
                assert_eq!(step, WizardModel::Install(ModelSize::Small), "{model_size}");
            }
        }
        let step = wizard_model(
            &transcription("base", None),
            false,
            |_| false,
            recommended_model(),
        );
        let message = wizard_model_message(&step, recommended_model());
        assert!(
            message.contains(&format!("downloads {},", size_key(recommended_model()))),
            "{message}"
        );

        let step = wizard_model(
            &transcription("huge", None),
            true,
            |_| false,
            ModelSize::Small,
        );
        assert_eq!(
            step,
            WizardModel::Replace {
                unknown: "huge".to_string(),
                model: ModelSize::Small
            }
        );
    }

    #[test]
    fn a_cloud_backend_needs_no_local_model() {
        let mut cloud = transcription("large-v3", None);
        cloud.backend = crate::config::settings::TranscriptionBackend::Groq;
        for config_exists in [true, false] {
            let step = wizard_model(&cloud, config_exists, |_| false, ModelSize::Small);
            assert_eq!(step, WizardModel::Cloud("Groq"));
            let message = wizard_model_message(&step, ModelSize::Small);
            assert!(message.contains("downloads no local model"), "{message}");
        }
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

    #[test]
    fn only_yes_goes_on() {
        for answer in ["y\n", "Y", " yes \n", "YES"] {
            assert!(is_yes(answer), "{answer:?}");
        }
        for answer in ["", "\n", "n", "no", "yep", "sure"] {
            assert!(!is_yes(answer), "{answer:?}");
        }
    }
}
