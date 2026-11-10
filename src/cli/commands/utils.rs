use crate::config::server;
use crate::config::settings::{LlmConfig, LlmSetting, TranscriptionBackend};
use crate::transcription::GpuAvailability;
use crate::{AudioCapture, Config};
/// Utility functions shared across command handlers
///
/// This module contains helper functions used by multiple command implementations.
use anyhow::{Context, Result};
use std::io::Write;

/// Print one line on stdout for a script. A reader that closed the pipe
/// early is not an error.
pub fn print_line(line: &str) -> Result<()> {
    let mut out = std::io::stdout().lock();
    match writeln!(out, "{}", line).and_then(|()| out.flush()) {
        Err(e) if e.kind() != std::io::ErrorKind::BrokenPipe => {
            Err(e).context("Failed to write to stdout")
        },
        _ => Ok(()),
    }
}

/// Display configuration status information
pub async fn show_config_status() -> Result<()> {
    println!("📋 Configuration:");

    match Config::load() {
        Ok(config) => {
            let transcription = &config.transcription;
            let key = transcription
                .key_var()
                .map(|var| match crate::config::keys::api_key(var) {
                    Some(_) => format!("{} set", var),
                    None => format!("{} missing", var),
                });
            match transcription.backend {
                TranscriptionBackend::Local => {
                    println!("  Transcription: local");
                    println!("  Model: {}", transcription.model_path().display());
                    println!("  Use GPU: {}", transcription.use_gpu);
                },
                TranscriptionBackend::OpenAiCompatible => {
                    println!(
                        "  Transcription: {} ({})",
                        transcription.describe_server(),
                        key.as_deref().unwrap_or("no key")
                    );
                },
                backend => {
                    println!(
                        "  Transcription: {} {} (cloud, audio leaves this machine; {})",
                        backend.name(),
                        transcription.cloud_model().unwrap_or_default(),
                        key.unwrap_or_default()
                    );
                },
            }
            println!("  LLM: {}", llm_summary(&config.llm));
            println!("  Sample Rate: {}Hz", config.audio.sample_rate);
            println!("  Hotkey: {}", config.hotkey.combination);
        },
        Err(e) => {
            println!("  ❌ Failed to load config: {}", e);
        },
    }

    Ok(())
}

/// The LLM setting as written, without a probe of Ollama or the server.
fn llm_summary(llm: &LlmConfig) -> String {
    match llm.provider {
        LlmSetting::Auto => "auto (Ollama, else Anthropic)".to_string(),
        LlmSetting::Ollama => format!("Ollama {}", llm.ollama_model),
        LlmSetting::Anthropic => format!("Anthropic {}", llm.anthropic_model),
        LlmSetting::OpenAiCompatible => format!(
            "OpenAI-compatible {}",
            server::describe(&llm.server_model, &llm.server_url, server::Sends::Text)
        ),
        LlmSetting::None => "none".to_string(),
    }
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
    let engine = GpuAvailability::system_info();
    let mut lines = engine.lines();
    if let Some(name) = lines.next() {
        println!("  Speech engine: {}", name);
    }
    for device in lines {
        println!("    {}", device);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_llm_line_says_where_a_server_sends_the_text() {
        let mut llm = LlmConfig::defaults();
        assert_eq!(llm_summary(&llm), "auto (Ollama, else Anthropic)");
        llm.provider = LlmSetting::OpenAiCompatible;
        llm.server_url = "http://10.0.0.4:8000/v1".into();
        llm.server_model = "qwen3".into();
        assert_eq!(
            llm_summary(&llm),
            "OpenAI-compatible qwen3 on the local server 10.0.0.4:8000"
        );
        llm.server_url = "https://llm.example.com".into();
        assert_eq!(
            llm_summary(&llm),
            "OpenAI-compatible qwen3 on llm.example.com, a service that receives the text"
        );
    }
}
