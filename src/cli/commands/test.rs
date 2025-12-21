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
/// use hush::cli_main::TestCommands;
///
/// // Test audio capture for 3 seconds
/// let cmd = TestCommands::Audio {
///     duration: 3,
///     list_devices: false,
///     device: None,
///     save: None,
/// };
/// handle_test(cmd).await?;
///
/// // Run all tests
/// let cmd = TestCommands::All {
///     benchmarks: false,
///     output: None,
/// };
/// handle_test(cmd).await?;
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
        TestCommands::Pipeline {
            count,
            transcribe_only,
        } => test_full_pipeline(count, transcribe_only).await,
        TestCommands::All { benchmarks, output } => run_all_tests(benchmarks, output).await,
    }
}
