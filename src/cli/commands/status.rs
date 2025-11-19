use super::utils::{show_config_status, show_device_status, show_system_info};
use crate::{hotkey, AudioCapture, Config, TextInserter, WhisperTranscriber};
/// Status command implementation
///
/// Displays system status including configuration, devices, and component health.
use anyhow::Result;

/// Handle the status command
///
/// # Arguments
///
/// * `_config` - Show configuration details (currently unused, always shown)
/// * `_devices` - Show device details (currently unused, always shown)
/// * `full` - Show full system component status
///
/// # Examples
///
/// ```no_run
/// // Basic status
/// handle_status(false, false, false).await?;
///
/// // Full status with component health
/// handle_status(false, false, true).await?;
/// ```
pub async fn handle_status(_config: bool, _devices: bool, full: bool) -> Result<()> {
    println!("🤫 Hush System Status");
    println!();

    // Always show basic status
    show_system_info().await?;
    println!();
    show_config_status().await?;
    println!();
    show_device_status().await?;

    if full {
        println!();
        println!("⚙️ System Components:");

        // Test each component
        print!("Audio Capture: ");
        match AudioCapture::new(None) {
            Ok(_) => println!("✅ Available"),
            Err(e) => println!("❌ Failed ({})", e),
        }

        print!("Text Insertion: ");
        match TextInserter::new() {
            Ok(_) => println!("✅ Available"),
            Err(e) => println!("❌ Failed ({})", e),
        }

        print!("Hotkey System: ");
        let config = Config::load()?;
        match hotkey::HotkeyManager::new(&config.hotkey.combination) {
            Ok(_) => println!("✅ Available"),
            Err(e) => println!("❌ Failed ({})", e),
        }

        print!("Whisper Transcriber: ");
        match WhisperTranscriber::new(
            &config.transcription.model_path,
            config.transcription.use_cuda,
        )
        .await
        {
            Ok(_) => println!("✅ Available"),
            Err(e) => println!("❌ Failed ({})", e),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_status_command_runs() {
        // This is a basic smoke test - the command should not panic
        // Actual functionality depends on system configuration
        let result = handle_status(false, false, false).await;

        // We don't assert success since it depends on system setup
        // but we verify it doesn't panic
        let _ = result;
    }
}
