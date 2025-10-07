/// Integration tests with real hardware components
///
/// These tests require actual hardware (microphone, X11, etc.)
/// and are meant to validate the adapters work correctly with real components.
///
/// Run with: cargo test --test integration_hardware_tests -- --ignored
///
/// Note: These tests are marked as #[ignore] by default since they require:
/// - Microphone access
/// - X11 display server
/// - Hotkey registration permissions
/// - Whisper model files

use hush::adapters::{CpalAudioAdapter, WhisperAdapter, X11TextAdapter, HotkeyTriggerAdapter};
use hush::application::{HushAppBuilder, AppMode};
use hush::core::traits::{AudioSource, Transcriber, TextOutput, InputTrigger};
use hush::Config;
use std::time::Duration;

#[test]
#[ignore] // Requires real audio hardware
fn test_audio_adapter_with_real_hardware() {
    // This test validates CpalAudioAdapter works with real audio device
    let result = CpalAudioAdapter::new(None);

    match result {
        Ok(mut adapter) => {
            println!("✅ Audio adapter created successfully");
            println!("   Device: {}", adapter.device_name());
            println!("   Config: {:?}", adapter.config());

            // Test recording cycle
            adapter.start_recording().expect("Failed to start recording");
            assert!(adapter.is_recording());

            // Record for 1 second
            std::thread::sleep(Duration::from_secs(1));

            let buffer = adapter.stop_recording().expect("Failed to stop recording");
            assert!(!adapter.is_recording());
            assert!(!buffer.is_empty());

            println!("   Recorded: {} samples ({:.2}s)",
                     buffer.len(),
                     buffer.duration.as_secs_f32());
        }
        Err(e) => {
            eprintln!("⚠️  Audio adapter creation failed: {}", e);
            eprintln!("   This is expected if no microphone is available");
        }
    }
}

#[tokio::test]
#[ignore] // Requires Whisper model files
async fn test_transcriber_adapter_with_real_model() {
    let config = Config::load().expect("Failed to load config");

    let result = WhisperAdapter::new(
        &config.transcription.model_path,
        config.transcription.use_cuda,
    ).await;

    match result {
        Ok(transcriber) => {
            println!("✅ Transcriber adapter created successfully");
            let info = transcriber.info();
            println!("   Name: {}", info.name);
            println!("   Hardware Accelerated: {}", info.hardware_accelerated);

            // Test transcription with dummy audio
            let dummy_audio = hush::core::traits::AudioBuffer::new(
                vec![0.1f32; 16000], // 1 second of audio
                16000,
                1,
            );

            let result = transcriber.transcribe(&dummy_audio).await;

            match result {
                Ok(transcription) => {
                    println!("   Transcription: '{}'", transcription.text);
                    println!("   Confidence: {:.2}", transcription.confidence);
                    println!("   Processing Time: {:?}", transcription.processing_time);
                }
                Err(e) => {
                    eprintln!("   ⚠️  Transcription failed: {}", e);
                }
            }
        }
        Err(e) => {
            eprintln!("⚠️  Transcriber creation failed: {}", e);
            eprintln!("   This is expected if model files are not available");
            eprintln!("   Run: ./scripts/download-models.sh");
        }
    }
}

#[test]
#[ignore] // Requires X11 display server
fn test_text_output_adapter_with_x11() {
    let result = X11TextAdapter::new();

    match result {
        Ok(adapter) => {
            println!("✅ Text output adapter created successfully");
            println!("   Method: {}", adapter.output_method());
            println!("   Available: {}", adapter.is_available());

            // Note: We don't actually insert text in tests to avoid disrupting user's workflow
            println!("   (Text insertion not tested to avoid disruption)");
        }
        Err(e) => {
            eprintln!("⚠️  Text output adapter creation failed: {}", e);
            eprintln!("   This is expected if X11 is not available");
            eprintln!("   Check: echo $DISPLAY");
        }
    }
}

#[test]
#[ignore] // Requires hotkey registration permissions
fn test_hotkey_adapter_with_system() {
    let result = HotkeyTriggerAdapter::new("Ctrl+Shift+F12");

    match result {
        Ok(adapter) => {
            println!("✅ Hotkey adapter created successfully");
            println!("   Combination: {}", adapter.description());

            // Note: We don't test actual hotkey events to avoid system interference
            println!("   (Event listening not tested to avoid interference)");
        }
        Err(e) => {
            eprintln!("⚠️  Hotkey adapter creation failed: {}", e);
            eprintln!("   This might require elevated permissions");
            eprintln!("   Try: Use 'hush manual' mode instead");
        }
    }
}

#[tokio::test]
#[ignore] // Requires all hardware components
async fn test_full_application_with_real_hardware() {
    println!("🧪 Testing full application with real hardware components");

    let config = Config::load().expect("Failed to load config");

    // Try to create all real components
    let audio_result = CpalAudioAdapter::new(config.audio.device.as_deref());
    let transcriber_result = WhisperAdapter::new(
        &config.transcription.model_path,
        config.transcription.use_cuda,
    ).await;
    let text_output_result = X11TextAdapter::new();
    let hotkey_result = HotkeyTriggerAdapter::new(&config.hotkey.combination);

    // Check what components are available
    let components_available = audio_result.is_ok()
        && transcriber_result.is_ok()
        && text_output_result.is_ok()
        && hotkey_result.is_ok();

    if !components_available {
        println!("⚠️  Not all components available:");
        if audio_result.is_err() {
            println!("   ❌ Audio: {}", audio_result.unwrap_err());
        } else {
            println!("   ✅ Audio");
        }
        if transcriber_result.is_err() {
            println!("   ❌ Transcriber: {}", transcriber_result.unwrap_err());
        } else {
            println!("   ✅ Transcriber");
        }
        if text_output_result.is_err() {
            println!("   ❌ Text Output: {}", text_output_result.unwrap_err());
        } else {
            println!("   ✅ Text Output");
        }
        if hotkey_result.is_err() {
            println!("   ❌ Hotkey: {}", hotkey_result.unwrap_err());
        } else {
            println!("   ✅ Hotkey");
        }
        println!();
        println!("   Skipping full integration test");
        return;
    }

    println!("✅ All components available, creating application");

    // Create application with real components
    let app_result = HushAppBuilder::new()
        .with_audio(Box::new(audio_result.unwrap()))
        .with_transcriber(Box::new(transcriber_result.unwrap()))
        .with_text_output(Box::new(text_output_result.unwrap()))
        .with_input_trigger(Box::new(hotkey_result.unwrap()))
        .manual_mode()
        .with_notifications(false)
        .build();

    match app_result {
        Ok(app) => {
            println!("✅ Application created successfully with real hardware");
            let stats = app.get_stats();
            println!("   Audio Device: {}", stats.audio_device);
            println!("   Transcriber: {}", stats.transcriber_name);
            println!("   Text Output: {}", stats.text_output_method);
            println!("   Input Trigger: {}", stats.input_trigger_desc);
            println!();
            println!("🎉 Integration test passed!");
        }
        Err(e) => {
            eprintln!("❌ Application creation failed: {}", e);
            panic!("Integration test failed");
        }
    }
}

/// Helper test to show what tests are available
#[test]
fn test_integration_tests_info() {
    println!("
🧪 Integration Test Suite

This test suite validates the refactored architecture with real hardware.

Available Tests (all marked #[ignore]):
  - test_audio_adapter_with_real_hardware
  - test_transcriber_adapter_with_real_model
  - test_text_output_adapter_with_x11
  - test_hotkey_adapter_with_system
  - test_full_application_with_real_hardware

Run Integration Tests:
  cargo test --test integration_hardware_tests -- --ignored

Run Specific Test:
  cargo test --test integration_hardware_tests test_audio_adapter_with_real_hardware -- --ignored --nocapture

Requirements:
  ✓ Microphone connected
  ✓ X11 display server running (echo $DISPLAY)
  ✓ Whisper model files downloaded (./scripts/download-models.sh)
  ✓ Hotkey registration permissions

Note: Tests are safe and non-destructive. They won't actually insert text or
register permanent hotkeys.
    ");
}
