use anyhow::Result;

// Import TestCommands enum from cli_main
use crate::cli_main::TestCommands;

// Import test helper functions from dispatcher
use crate::cli::dispatcher::{
    run_all_tests, test_audio_system, test_full_pipeline, test_hotkey_system,
    test_text_insertion_system, test_transcription_system,
};

/// Handle the test command
///
/// Executes various system tests to verify Hush functionality.
/// This command provides comprehensive testing for all major subsystems:
/// audio capture, transcription, text insertion, hotkeys, and full pipeline.
///
/// # Arguments
///
/// * `test_command` - The specific test to run (audio, transcription, text-insertion, hotkeys, pipeline, or all)
///
/// # Examples
///
/// ```no_run
/// # use hush::cli::commands::handle_test;
/// # use hush::cli::TestCommands;
/// # async fn example() -> anyhow::Result<()> {
/// // Test audio capture for 3 seconds
/// handle_test(TestCommands::Audio {
///     duration: 3,
///     list_devices: false,
///     device: None,
///     save: None,
/// })
/// .await?;
/// # Ok(())
/// # }
/// ```
///
/// # Test Types
///
/// - **Audio**: Tests audio capture system, optionally lists devices or tests specific device
/// - **Transcription**: Tests Whisper transcription, optionally with different models
/// - **TextInsertion**: Tests text insertion methods (X11, uinput, etc.)
/// - **Hotkeys**: Tests hotkey detection and handling
/// - **Pipeline**: Tests the complete voice-to-text pipeline end-to-end
/// - **All**: Runs comprehensive test suite across all subsystems
///
/// # Returns
///
/// Returns `Ok(())` if tests complete successfully, or an error if any test fails.
pub async fn handle_test(test_command: TestCommands) -> Result<()> {
    match test_command {
        TestCommands::Audio {
            duration,
            list_devices,
            device,
            save,
        } => test_audio_system(duration, list_devices, device, save).await,
        TestCommands::Transcription {
            file,
            all_models,
            timing,
        } => test_transcription_system(file, all_models, timing).await,
        TestCommands::TextInsertion {
            text,
            all_methods,
            uinput,
        } => test_text_insertion_system(text, all_methods, uinput).await,
        TestCommands::Hotkeys {
            combination,
            duration,
        } => test_hotkey_system(combination, duration).await,
        TestCommands::Window { duration } => test_window_detection(duration).await,
        TestCommands::Pipeline {
            count,
            transcribe_only,
        } => test_full_pipeline(count, transcribe_only).await,
        TestCommands::All { benchmarks, output } => run_all_tests(benchmarks, output).await,
    }
}

/// Report the focused window once a second so the user can switch around.
async fn test_window_detection(duration: u64) -> Result<()> {
    use crate::text::WindowProvider;
    use crate::text_processing::{AppKind, ProfilesConfig};

    let provider = WindowProvider::detect();
    println!("Window detection: {}", provider.name());
    if provider.name() == "none" {
        println!("No compositor backend matched (X11, Hyprland, Sway, or KDE Plasma on Wayland).");
        return Ok(());
    }
    let profiles = crate::config::Config::load()
        .map(|c| c.profiles)
        .unwrap_or_else(|_| ProfilesConfig::defaults());
    let mut last: Option<String> = None;
    for _ in 0..duration.max(1) {
        let line = match provider.focused() {
            Some(info) => format!(
                "{:?} profile, class \"{}\", title \"{}\"",
                AppKind::classify(&info, &profiles),
                info.class,
                info.title
            ),
            None => "no focused window reported".to_string(),
        };
        if last.as_deref() != Some(&line) {
            println!("{}", line);
            last = Some(line);
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    Ok(())
}
