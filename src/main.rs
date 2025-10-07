use clap::{Parser, Subcommand};
use anyhow::Result;
use hush::config::settings::Config;

#[derive(Parser)]
#[command(name = "hush")]
#[command(about = "🤫 Hush - Voice-to-Text for Linux Developers")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    
    /// Use CLI mode instead of TUI
    #[arg(long)]
    no_tui: bool,
    
    /// Verbose logging
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(Subcommand)]
enum Commands {
    /// Show UInput setup guide for optimal text insertion
    SetupUinput,
    /// Diagnose UInput setup issues and get specific solutions
    DiagnoseUinput,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Handle subcommands first
    if let Some(command) = cli.command {
        return handle_subcommand(command).await;
    }
    
    if cli.no_tui {
        // Run legacy CLI mode for testing/fallback
        run_cli_mode().await
    } else {
        // Run TUI mode (default)
        run_tui_mode().await
    }
}

async fn handle_subcommand(command: Commands) -> Result<()> {
    match command {
        Commands::SetupUinput => {
            hush::text::print_uinput_setup_guidance();
            Ok(())
        }
        Commands::DiagnoseUinput => {
            hush::text::diagnose_uinput_issues()
        }
    }
}

async fn run_tui_mode() -> Result<()> {
    hush::tui::run_tui().await
}

async fn run_cli_mode() -> Result<()> {
    println!("🤫 Hush - Voice-to-Text for Linux Developers (CLI Mode)");
    println!("This is a fallback CLI mode for testing.");
    println!("Use the TUI mode (default) for the full experience.");
    
    println!("Loading configuration...");
    let config = Config::load()?;
    
    println!("Testing audio capture system...");
    match hush::AudioCapture::new(config.audio.device.as_deref()) {
        Ok(audio_capture) => {
            println!("✅ Audio capture initialized successfully!");
            println!("   Device: {}", audio_capture.get_device_name());
            println!("   Available devices: {:?}", hush::AudioCapture::list_devices().unwrap_or_default());
        },
        Err(e) => {
            println!("⚠️  Audio capture failed: {}", e);
        }
    }
    
    println!("Testing Whisper transcription system...");
    match hush::WhisperTranscriber::new(&config.transcription.model_path, config.transcription.use_cuda).await {
        Ok(transcriber) => {
            println!("✅ Whisper transcriber initialized successfully!");
            println!("   Device: {}", transcriber.get_device_info());
            println!("   CUDA enabled: {}", transcriber.is_using_cuda());
            
            // Test transcription with dummy audio data
            let test_audio = vec![0.1f32; 48000]; // 3 seconds of test data at 16kHz
            match transcriber.transcribe(&test_audio) {
                Ok(text) => println!("   Test transcription: '{}'", text),
                Err(e) => println!("   Transcription test failed: {}", e),
            }
        },
        Err(e) => {
            println!("⚠️  Whisper transcriber failed: {}", e);
        }
    }
    
    println!("Testing global hotkey system...");
    match hush::hotkey::HotkeyManager::new(&config.hotkey.combination) {
        Ok((hotkey_manager, hotkey_receiver)) => {
            println!("✅ Hotkey manager initialized successfully!");
            println!("   Combination: '{}'", hotkey_manager.get_combination());
            
            match hotkey_manager.start_listening() {
                Ok(()) => {
                    println!("   Hotkey listening started. Press {} to test (waiting 2 seconds)...", config.hotkey.combination);
                    
                    // Wait for a short time to see if any hotkey events come in
                    use std::time::{Duration, Instant};
                    let start = Instant::now();
                    let timeout = Duration::from_secs(2);
                    
                    let mut events_detected = 0;
                    while start.elapsed() < timeout {
                        if let Ok(event) = hotkey_receiver.try_recv() {
                            events_detected += 1;
                            match event {
                                hush::hotkey::HotkeyEvent::Pressed => {
                                    println!("   🎯 Hotkey pressed detected!");
                                },
                                hush::hotkey::HotkeyEvent::Released => {
                                    println!("   🎯 Hotkey released detected!");
                                }
                            }
                        }
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    
                    if events_detected == 0 {
                        println!("   ℹ️  No hotkey events detected (this is normal for testing)");
                    }
                    
                    match hotkey_manager.stop_listening() {
                        Ok(()) => println!("   Hotkey listening stopped"),
                        Err(e) => println!("   ⚠️  Error stopping hotkey listener: {}", e),
                    }
                },
                Err(e) => println!("   ⚠️  Failed to start hotkey listening: {}", e),
            }
        },
        Err(e) => {
            println!("⚠️  Hotkey manager failed: {}", e);
        }
    }
    
    println!("Testing text insertion system...");
    // Check dependencies first
    match hush::text::check_dependencies() {
        Ok(()) => println!("✅ Text insertion dependencies OK"),
        Err(e) => println!("⚠️  Text insertion dependency check failed: {}", e),
    }
    
    match hush::TextInserter::new() {
        Ok(text_inserter) => {
            println!("✅ Text insertion system initialized successfully!");
            
            // Get focused window info
            match text_inserter.get_focused_window() {
                Ok(window) => {
                    println!("   Current window: '{}' (class: {})", window.title, window.class);
                },
                Err(e) => println!("   ⚠️  Could not get window info: {}", e),
            }
            
            println!("   ℹ️  Text insertion ready (test with: text_inserter.insert_text())");
        },
        Err(e) => {
            println!("⚠️  Text insertion system failed: {}", e);
        }
    }
    
    println!("Hush is ready! 🚀");
    println!("Note: Core functionality will be implemented in upcoming tasks:");
    println!("  - Task #2: Dependencies");
    println!("  - Task #3: Configuration System");
    println!("  - Task #4: Audio Capture");
    println!("  - Task #5: Whisper Integration");
    println!("  - Task #6: Global Hotkeys");
    println!("  - Task #7: Text Insertion");
    println!("  - Task #8: Audio Feedback");
    println!("  - Task #9: Main Event Loop");
    
    Ok(())
}
