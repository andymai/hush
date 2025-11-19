use crate::{AudioCapture, Config};
use crate::transcription::cuda::CudaAvailability;
/// Utility functions shared across command handlers
///
/// This module contains helper functions used by multiple command implementations.
use anyhow::Result;
use tracing::warn;

/// Display configuration status information
pub async fn show_config_status() -> Result<()> {
    println!("📋 Configuration:");

    match Config::load() {
        Ok(config) => {
            println!("  Model: {:?}", config.transcription.model_path);
            println!("  Use CUDA: {}", config.transcription.use_cuda);
            println!("  Sample Rate: {}Hz", config.audio.sample_rate);
            println!("  Hotkey: {}", config.hotkey.combination);
        },
        Err(e) => {
            println!("  ❌ Failed to load config: {}", e);
        },
    }

    Ok(())
}

/// Display system information including GPU status
pub async fn show_system_info() -> Result<()> {
    println!("📊 System Information:");
    println!("  OS: {}", std::env::consts::OS);
    println!("  Architecture: {}", std::env::consts::ARCH);

    let cuda = CudaAvailability::detect();
    if cuda.available {
        println!(
            "  🚀 GPU: {} (CUDA {})",
            cuda.device_name.as_deref().unwrap_or("Unknown"),
            cuda.cuda_version.as_deref().unwrap_or("Unknown")
        );
        println!("     Devices: {}", cuda.device_count);
    } else {
        #[cfg(feature = "cuda")]
        println!("  ⚠️  GPU: Not detected (using CPU)");

        #[cfg(not(feature = "cuda"))]
        println!("  💻 GPU: Not compiled (CPU-only build)");
    }

    Ok(())
}

/// Display audio device status information
pub async fn show_device_status() -> Result<()> {
    println!("🎤 Audio Devices:");

    match AudioCapture::list_devices() {
        Ok(devices) => {
            if devices.is_empty() {
                println!("  ⚠️ No audio devices found");
            } else {
                for (i, device) in devices.iter().enumerate() {
                    println!("  {}. {}", i + 1, device);
                }
            }
        },
        Err(e) => {
            println!("  ❌ Failed to list devices: {}", e);
        },
    }

    Ok(())
}

/// Print quick UInput setup instructions
pub fn print_uinput_quick_setup() {
    println!("🔧 Quick UInput Setup:");
    println!("sudo usermod -a -G input $USER");
    println!("sudo modprobe uinput");
    println!("echo 'uinput' | sudo tee /etc/modules-load.d/uinput.conf");
    println!("# Then log out and log back in");
}

/// Automatically fix UInput permissions (placeholder)
pub async fn auto_fix_uinput() -> Result<()> {
    warn!("Auto-fix functionality not yet implemented");
    println!("🚧 Auto-fix is not yet implemented. Please run 'hush setup uinput' for manual instructions.");
    Ok(())
}

/// Setup audio devices (list or test)
pub async fn setup_audio(list: bool, test: Option<String>) -> Result<()> {
    if list {
        println!("🎵 Available Audio Devices:");
        if let Ok(devices) = AudioCapture::list_devices() {
            for (i, device) in devices.iter().enumerate() {
                println!("  {}. {}", i + 1, device);
            }
        } else {
            println!("❌ Failed to list audio devices");
        }
    }

    if let Some(device_name) = test {
        println!("🎤 Testing audio device: {}", device_name);
        match AudioCapture::new(Some(&device_name)) {
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

/// Setup hotkeys (list or test)
pub async fn setup_hotkeys(test: Option<String>, list: bool) -> Result<()> {
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
