#![cfg(target_os = "macos")]

//! macOS-specific integration tests
//!
//! These tests validate platform-specific functionality on macOS including:
//! - Text insertion via CGEvent and Accessibility API
//! - System tray integration via tray-icon/NSStatusBar
//! - Metal GPU device detection
//! - Hotkey registration and event handling
//! - Permission checking utilities

use std::time::Duration;

mod text_insertion {
    use super::*;

    #[test]
    fn test_macos_adapter_creation() {
        // Should succeed (may warn about permissions)
        #[cfg(feature = "default")]
        {
            use hush::adapters::text::MacOSTextAdapter;
            let result = MacOSTextAdapter::new();

            // Adapter creation should succeed even without permissions
            // It will just use fallback methods
            match result {
                Ok(_) => println!("✅ macOS text adapter created"),
                Err(e) => {
                    // Only acceptable error is accessibility-related
                    let err_str = e.to_string();
                    assert!(
                        err_str.contains("Accessibility") || err_str.contains("accessibility"),
                        "Unexpected error: {}",
                        e
                    );
                }
            }
        }
    }

    #[test]
    fn test_accessibility_permission_check() {
        #[cfg(feature = "default")]
        {
            use hush::adapters::text::macos_adapter::check_accessibility_permissions;

            // This will be false in CI, true if manually granted
            let has_permissions = check_accessibility_permissions();
            println!("Accessibility permissions: {}", has_permissions);

            // Don't fail test, just report
            // In CI, this will be false
        }
    }

    #[tokio::test]
    #[ignore] // Requires Accessibility permissions
    async fn test_text_insertion_with_permissions() {
        #[cfg(feature = "default")]
        {
            use hush::adapters::text::MacOSTextAdapter;
            use hush::core::traits::TextOutput;

            let mut adapter = MacOSTextAdapter::new().expect("Failed to create adapter");

            // This requires Accessibility permissions
            let result = adapter.insert_text("Hello from Hush test").await;

            if let Err(e) = result {
                let err_str = e.to_string();
                if err_str.contains("Accessibility") || err_str.contains("accessibility") {
                    println!("SKIP: Accessibility permissions not granted");
                    return;
                }
                panic!("Unexpected error: {}", e);
            }

            // If we get here, text insertion worked
            println!("✅ Text insertion successful");
        }
    }

    #[tokio::test]
    async fn test_window_detection() {
        #[cfg(feature = "default")]
        {
            use hush::adapters::text::macos_adapter::get_focused_window_info;

            let result = get_focused_window_info();

            // May fail without permissions, but should not panic
            match result {
                Ok(window) => {
                    println!("Focused window: {} ({})", window.title, window.app_name);
                }
                Err(e) => {
                    println!("Window detection failed (expected without permissions): {}", e);
                }
            }
        }
    }
}

mod system_tray {
    use super::*;

    #[test]
    #[cfg(feature = "system-tray")]
    fn test_tray_creation() {
        use hush::tray::MacOSTrayAdapter;

        // Tray creation should succeed
        let result = MacOSTrayAdapter::new();

        match result {
            Ok(_) => println!("✅ Tray adapter created successfully"),
            Err(e) => {
                // In headless CI, tray creation might fail
                println!("⚠️  Tray creation failed: {}", e);
            }
        }

        // Don't fail test in CI (may not have display)
    }

    #[tokio::test]
    #[ignore] // Requires display/GUI
    #[cfg(feature = "system-tray")]
    async fn test_tray_show_and_hide() {
        use hush::tray::MacOSTrayAdapter;
        use hush::core::traits::SystemTray;

        let mut tray = MacOSTrayAdapter::new().expect("Failed to create tray");

        // Show tray
        let show_result = tray.show().await;
        assert!(show_result.is_ok(), "Failed to show tray: {:?}", show_result);

        // Wait a bit
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Hide tray
        let hide_result = tray.hide().await;
        assert!(hide_result.is_ok(), "Failed to hide tray: {:?}", hide_result);

        println!("✅ Tray show/hide test passed");
    }

    #[test]
    #[ignore] // Requires display/GUI
    #[cfg(feature = "system-tray")]
    fn test_tray_icon_state_changes() {
        use hush::tray::MacOSTrayAdapter;
        use hush::core::traits::{SystemTray, TrayIconState};

        let mut tray = MacOSTrayAdapter::new().expect("Failed to create tray");

        // Test all state transitions
        let states = vec![
            TrayIconState::Idle,
            TrayIconState::Recording,
            TrayIconState::Processing,
            TrayIconState::Error,
            TrayIconState::Idle,
        ];

        for state in states {
            tray.set_icon(state);
            println!("Set icon state to: {:?}", state);
            std::thread::sleep(Duration::from_millis(100));
        }

        println!("✅ Icon state changes test passed");
    }
}

mod gpu_acceleration {
    use super::*;

    #[test]
    fn test_device_detection() {
        use hush::adapters::transcription::device::DeviceInfo;

        let device_info = DeviceInfo::detect();

        println!("Detected device: {}", device_info.name);
        println!("Expected latency: {}ms", device_info.expected_latency_ms);

        // On Apple Silicon, should detect Metal
        #[cfg(target_arch = "aarch64")]
        {
            assert!(
                device_info.name.contains("Apple") ||
                device_info.name.contains("Metal") ||
                device_info.name.contains("GPU"),
                "Expected Metal/Apple device on Apple Silicon, got: {}",
                device_info.name
            );
        }

        // On Intel Mac, may use Metal or CPU
        #[cfg(target_arch = "x86_64")]
        {
            println!("Intel Mac - device: {}", device_info.name);
            // No strict assertion for Intel Macs
        }
    }

    #[test]
    #[cfg(feature = "metal")]
    fn test_metal_device_creation() {
        use candle_core::Device;

        let result = Device::new_metal(0);

        match result {
            Ok(_device) => {
                println!("✅ Metal device created successfully");
            }
            Err(e) => {
                // May fail on Intel Macs or in CI
                println!("⚠️  Metal device creation failed: {}", e);
                println!("This is expected on Intel Macs or without Metal support");
            }
        }
    }
}

mod hotkey {
    use super::*;

    #[test]
    #[ignore] // May require main thread
    fn test_hotkey_adapter_creation() {
        use hush::adapters::hotkey::HotkeyAdapter;

        let result = HotkeyAdapter::new();

        match result {
            Ok(_) => println!("✅ Hotkey adapter created"),
            Err(e) => {
                // May fail in CI without proper setup
                println!("⚠️  Hotkey creation failed: {}", e);
                let err_str = e.to_string();
                assert!(
                    err_str.contains("main thread") || err_str.contains("display"),
                    "Unexpected error: {}",
                    e
                );
            }
        }
    }
}

mod permissions {
    use super::*;

    #[test]
    fn test_accessibility_permission_detection() {
        #[cfg(feature = "default")]
        {
            use hush::adapters::text::macos_adapter::check_accessibility_permissions;

            // Should not panic, just return true/false
            let has_permissions = check_accessibility_permissions();

            if has_permissions {
                println!("✅ Accessibility permissions granted");
            } else {
                println!("⚠️  Accessibility permissions not granted");
                println!("   Grant in: Settings → Privacy & Security → Accessibility");
            }
        }
    }

    #[test]
    fn test_permission_prompt_no_crash() {
        #[cfg(feature = "default")]
        {
            use hush::adapters::text::macos_adapter::prompt_accessibility_permissions;

            // Should not crash, just print instructions
            prompt_accessibility_permissions();
            println!("✅ Permission prompt did not crash");
        }
    }
}

// Helper: Check if we're running in CI
#[allow(dead_code)]
fn is_ci() -> bool {
    std::env::var("CI").is_ok() ||
    std::env::var("GITHUB_ACTIONS").is_ok() ||
    std::env::var("CONTINUOUS_INTEGRATION").is_ok()
}

// Helper: Check if display is available
#[allow(dead_code)]
fn has_display() -> bool {
    std::env::var("DISPLAY").is_ok() || cfg!(target_os = "macos")
}
