use crate::transcription::GpuAvailability;
use crate::{AudioCapture, Config};
/// Utility functions shared across command handlers
///
/// This module contains helper functions used by multiple command implementations.
use anyhow::Result;

/// Display configuration status information
pub async fn show_config_status() -> Result<()> {
    println!("📋 Configuration:");

    match Config::load() {
        Ok(config) => {
            println!("  Model: {}", config.transcription.model_path().display());
            println!("  Use GPU: {}", config.transcription.use_gpu);
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

    let gpu = GpuAvailability::detect();
    if gpu.available {
        println!("  🚀 GPU: {} ({})", gpu.device_name, gpu.gpu_type);
    } else {
        println!("  💻 GPU: none available, using CPU ({})", gpu.device_name);
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

/// Print the commands `hush setup permissions` runs as root
pub fn print_uinput_quick_setup() {
    println!("🔧 Run as root (or run `hush setup permissions` on the host):");
    println!();
    print!("{}", crate::permissions::install_script());
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
        println!("  • RightAlt (default)");
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
