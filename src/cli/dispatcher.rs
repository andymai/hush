use crate::cli::commands::{
    handle_listen, handle_manual, handle_models, handle_setup, handle_test,
};
use crate::cli::{Commands, ModelCommands, SetupCommands, TestCommands};
use crate::logging::RequestContext;
use anyhow::{Context as AnyhowContext, Result};
use hound;
use std::path::PathBuf;
use std::{env, fs};
use tracing::{debug, error, info, warn};

// Import Hush components
use crate::{hotkey, AudioCapture, Config, WhisperTranscriber};

#[cfg(target_os = "linux")]
use crate::TextInserter;

pub struct CommandDispatcher {
    _config_path: Option<PathBuf>,
    _notifications_enabled: bool,
}

impl CommandDispatcher {
    pub fn new(config_path: Option<PathBuf>, notifications_enabled: bool) -> Self {
        Self {
            _config_path: config_path,
            _notifications_enabled: notifications_enabled,
        }
    }

    pub async fn dispatch(&self, command: Commands) -> Result<()> {
        let command_name = match &command {
            Commands::Start { .. } => "start",
            Commands::Record { .. } => "record",
            Commands::Manual { .. } => "manual",
            Commands::Listen { .. } => "listen",
            Commands::Setup { .. } => "setup",
            Commands::Test { .. } => "test",
            Commands::Models { .. } => "models",
            Commands::Status { .. } => "status",
            Commands::Install { .. } => "install",
            Commands::Uninstall { .. } => "uninstall",
            #[cfg(target_os = "macos")]
            Commands::CheckPermissions => "check_permissions",
        };

        let ctx = RequestContext::new(&format!("command_{}", command_name))
            .with_metadata("command", command_name);

        info!(
            request_id = %ctx.request_id,
            command = %command_name,
            "🚀 Executing command"
        );

        let result = match command {
            Commands::Start {
                daemon,
                elevated,
                cli,
            } => self.handle_start(daemon, elevated, cli).await,
            Commands::Record {
                duration,
                print_only,
                save_audio,
            } => {
                debug!(
                    request_id = %ctx.request_id,
                    duration = %duration,
                    print_only = %print_only,
                    "Record command parameters"
                );
                self.handle_record(duration, print_only, save_audio).await
            },
            Commands::Manual { count } => {
                debug!(
                    request_id = %ctx.request_id,
                    count = %count,
                    "Manual command parameters"
                );
                handle_manual(count).await
            },
            Commands::Listen {
                editing_mode,
                no_processing,
                no_button,
            } => {
                debug!(
                    request_id = %ctx.request_id,
                    editing_mode = %editing_mode,
                    no_processing = %no_processing,
                    no_button = %no_button,
                    "Listen command parameters"
                );
                handle_listen(editing_mode.clone(), no_processing, no_button).await
            },
            Commands::Setup { setup_command } => handle_setup(setup_command).await,
            Commands::Test { test_command } => handle_test(test_command).await,
            Commands::Models { model_command } => self.handle_models(model_command).await,
            Commands::Status {
                config,
                devices,
                full,
            } => {
                debug!(
                    request_id = %ctx.request_id,
                    config = %config,
                    devices = %devices,
                    full = %full,
                    "Status command parameters"
                );
                self.handle_status(config, devices, full).await
            },
            Commands::Install {
                autostart,
                desktop,
                system,
            } => self.handle_install(autostart, desktop, system).await,
            Commands::Uninstall {
                autostart,
                desktop,
                system,
            } => self.handle_uninstall(autostart, desktop, system).await,
            #[cfg(target_os = "macos")]
            Commands::CheckPermissions => self.handle_check_permissions().await,
        };

        match &result {
            Ok(()) => {
                info!(
                    request_id = %ctx.request_id,
                    command = %command_name,
                    elapsed_ms = %ctx.elapsed().as_millis(),
                    "✅ Command completed successfully"
                );
            },
            Err(e) => {
                error!(
                    request_id = %ctx.request_id,
                    command = %command_name,
                    elapsed_ms = %ctx.elapsed().as_millis(),
                    error = %e,
                    "❌ Command failed"
                );
            },
        }

        result
    }

    async fn handle_start(&self, _daemon: bool, _elevated: bool, cli: bool) -> Result<()> {
        info!("🚀 Starting Hush voice-to-text");

        if cli {
            // Use CLI/hotkey mode (legacy mode)
            info!("Starting CLI/hotkey mode...");
            println!("🚧 Direct hotkey mode not yet implemented in new CLI system.");
            println!("Please use one of these alternatives:");
            println!("  • cargo run --bin hush-mvp daemon  - Use original daemon mode");
            println!("  • ./hush record          - Single recording mode");
            Ok(())
        } else {
            // TUI has been removed
            println!("❌ TUI interface has been removed from this project.");
            println!("Please use one of these alternatives:");
            println!("  • ./hush start --cli      - Start with CLI/hotkey mode");
            println!("  • ./hush record           - Single recording mode");
            println!("  • ./hush manual           - Manual recording mode");
            Ok(())
        }
    }

    async fn handle_record(
        &self,
        duration: u64,
        print_only: bool,
        save_audio: Option<PathBuf>,
    ) -> Result<()> {
        info!("🎙️ Starting single recording (max {}s)", duration);

        // Simple recording implementation
        let config = Config::load()?;
        let mut audio_capture = AudioCapture::new(config.audio.device.as_deref())?;

        println!("Press Enter to start recording...");
        std::io::stdin()
            .read_line(&mut String::new())
            .context("Failed to read user input")?;

        println!(
            "🎤 Recording... (will auto-stop in {}s or press Enter to stop earlier)",
            duration
        );
        audio_capture.start_recording()?;

        // Wait for either timeout or user input
        let start_time = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(duration);

        while start_time.elapsed() < timeout {
            // Check for user input to stop early
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            // TODO: Add non-blocking stdin check
        }

        let audio_data = audio_capture.stop_recording()?;
        println!("✅ Recording stopped ({} samples)", audio_data.len());

        // Transcribe if not print-only mode
        if !print_only {
            println!("🗣️ Transcribing...");
            let transcriber = WhisperTranscriber::new(
                &config.transcription.model_path,
                config.transcription.use_cuda,
            )
            .await?;

            match transcriber
                .transcribe_async(&audio_data, config.audio.sample_rate)
                .await
            {
                Ok(result) => {
                    println!("✅ Transcription: '{}'", result.text);

                    // Insert text
                    #[cfg(target_os = "linux")]
                    {
                        let mut text_inserter = TextInserter::new()?;
                        println!("⌨️ Inserting text (3 second delay)...");
                        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

                        match text_inserter.insert_text(&result.text) {
                            Ok(()) => println!("✅ Text inserted successfully"),
                            Err(e) => println!("❌ Text insertion failed: {}", e),
                        }
                    }

                    #[cfg(not(target_os = "linux"))]
                    {
                        println!("ℹ️  Text insertion not available on this platform");
                    }
                },
                Err(e) => println!("❌ Transcription failed: {}", e),
            }
        } else {
            println!("Print-only mode: audio recorded but not transcribed");
        }

        // Save audio to file if requested
        if let Some(path) = save_audio {
            Self::save_audio_to_file(&audio_data, &path, config.audio.sample_rate)?;
        }

        Ok(())
    }

    async fn handle_models(&self, model_command: ModelCommands) -> Result<()> {
        handle_models(model_command).await
    }

    /// Save audio data to a WAV file
    fn save_audio_to_file(audio_data: &[f32], path: &PathBuf, sample_rate: u32) -> Result<()> {
        info!("💾 Saving audio to: {}", path.display());

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut writer = hound::WavWriter::create(path, spec)
            .with_context(|| format!("Failed to create WAV file: {}", path.display()))?;

        // Convert f32 samples to i16
        for &sample in audio_data {
            let sample_i16 = (sample * i16::MAX as f32) as i16;
            writer
                .write_sample(sample_i16)
                .with_context(|| "Failed to write audio sample")?;
        }

        writer
            .finalize()
            .with_context(|| "Failed to finalize WAV file")?;

        println!("✅ Audio saved to: {}", path.display());
        info!(
            "Audio file saved: {} samples @ {}Hz",
            audio_data.len(),
            sample_rate
        );
        Ok(())
    }

    async fn handle_status(&self, _config: bool, _devices: bool, full: bool) -> Result<()> {
        println!("🤫 Hush System Status");
        println!();

        // Always show basic status
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
            #[cfg(target_os = "linux")]
            {
                match TextInserter::new() {
                    Ok(_) => println!("✅ Available"),
                    Err(e) => println!("❌ Failed ({})", e),
                }
            }
            #[cfg(not(target_os = "linux"))]
            {
                println!("ℹ️  Platform-specific (use trait-based adapters)");
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

    async fn handle_install(&self, autostart: bool, desktop: bool, system: bool) -> Result<()> {
        println!("🔧 Installing Hush desktop integration...");

        if !autostart && !desktop && !system {
            // Install everything by default
            install_autostart().await?;
            install_desktop_entry().await?;
        } else {
            if autostart {
                install_autostart().await?;
            }
            if desktop {
                install_desktop_entry().await?;
            }
            if system {
                install_system_wide().await?;
            }
        }

        println!("✅ Installation complete!");
        Ok(())
    }

    async fn handle_uninstall(&self, autostart: bool, desktop: bool, system: bool) -> Result<()> {
        println!("🗑️ Removing Hush desktop integration...");

        if !autostart && !desktop && !system {
            // Remove everything by default
            remove_autostart().await?;
            remove_desktop_entry().await?;
        } else {
            if autostart {
                remove_autostart().await?;
            }
            if desktop {
                remove_desktop_entry().await?;
            }
            if system {
                remove_system_wide().await?;
            }
        }

        println!("✅ Uninstallation complete!");
        Ok(())
    }

    /// Check macOS system permissions (Accessibility, Microphone)
    #[cfg(target_os = "macos")]
    async fn handle_check_permissions(&self) -> Result<()> {
        println!("🔐 macOS Permission Status\n");

        // Check Accessibility permissions
        #[cfg(feature = "default")]
        {
            use crate::adapters::text::macos_adapter::check_accessibility_permissions;

            let accessibility = check_accessibility_permissions();
            if accessibility {
                println!("✅ Accessibility: Granted");
            } else {
                println!("❌ Accessibility: Not granted");
                println!("   Required for text insertion");
                println!("   Settings → Privacy & Security → Accessibility");
            }
        }

        // Check Microphone permissions
        println!("\n🎤 Microphone: Checking...");

        // Try to create audio capture to check microphone access
        match crate::AudioCapture::new(None) {
            Ok(_) => {
                println!("✅ Microphone: Accessible");
            }
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("permission") || err_str.contains("access") {
                    println!("❌ Microphone: Access denied");
                    println!("   Settings → Privacy & Security → Microphone");
                } else {
                    println!("⚠️  Microphone: Check failed");
                    println!("   Error: {}", e);
                }
            }
        }

        println!();
        Ok(())
    }
}

// Test functions (these would call existing test binaries)
pub async fn test_audio_system(
    duration: u64,
    list_devices: bool,
    device: Option<String>,
    save: Option<PathBuf>,
) -> Result<()> {
    println!("🎤 Testing audio capture system...");

    if list_devices {
        if let Ok(devices) = crate::AudioCapture::list_devices() {
            println!("Available devices:");
            for device in devices {
                println!("  • {}", device);
            }
        }
    }

    // Run the test-audio equivalent functionality
    println!("Recording for {} seconds...", duration);

    let audio_device = device.as_deref();
    match crate::AudioCapture::new(audio_device) {
        Ok(mut capture) => match capture.start_recording() {
            Ok(()) => {
                println!("✅ Audio recording started successfully");

                tokio::time::sleep(tokio::time::Duration::from_secs(duration)).await;

                match capture.stop_recording() {
                    Ok(audio_data) => {
                        println!("✅ Audio recording completed");
                        println!("   Duration: {}s", duration);
                        println!("   Samples: {}", audio_data.len());

                        if let Some(save_path) = save {
                            CommandDispatcher::save_audio_to_file(&audio_data, &save_path, 16000)?;
                        }
                    },
                    Err(e) => println!("❌ Failed to stop recording: {}", e),
                }
            },
            Err(e) => println!("❌ Failed to start recording: {}", e),
        },
        Err(e) => println!("❌ Audio system test failed: {}", e),
    }

    Ok(())
}

pub async fn test_transcription_system(
    file: Option<PathBuf>,
    all_models: bool,
    timing: bool,
) -> Result<()> {
    println!("🗣️ Testing transcription system...");

    let config = crate::Config::load()?;

    if let Some(audio_file) = file {
        println!("Transcribing file: {}", audio_file.display());

        // Read the WAV file
        let mut reader = hound::WavReader::open(&audio_file)
            .with_context(|| format!("Failed to open audio file: {}", audio_file.display()))?;

        let spec = reader.spec();
        println!("   Sample rate: {}Hz", spec.sample_rate);
        println!("   Channels: {}", spec.channels);
        println!("   Bits per sample: {}", spec.bits_per_sample);

        // Read samples and convert to f32
        let audio_data: Vec<f32> = if spec.sample_format == hound::SampleFormat::Int {
            reader
                .samples::<i16>()
                .map(|s| {
                    s.context("Failed to read audio sample")
                        .map(|v| v as f32 / i16::MAX as f32)
                })
                .collect::<Result<Vec<f32>>>()?
        } else {
            reader
                .samples::<f32>()
                .map(|s| s.context("Failed to read audio sample"))
                .collect::<Result<Vec<f32>>>()?
        };

        println!("   Samples: {}", audio_data.len());
        println!(
            "   Duration: {:.2}s",
            audio_data.len() as f32 / spec.sample_rate as f32
        );

        // Transcribe
        let transcriber = crate::WhisperTranscriber::new(
            &config.transcription.model_path,
            config.transcription.use_cuda,
        )
        .await?;

        let start = std::time::Instant::now();
        match transcriber
            .transcribe_async(&audio_data, spec.sample_rate)
            .await
        {
            Ok(result) => {
                let elapsed = start.elapsed();
                println!("\n✅ Transcription completed in {:?}", elapsed);
                println!("   Text: '{}'", result.text);
                println!("   Confidence: {:.2}", result.confidence);
            },
            Err(e) => {
                println!("❌ Transcription failed: {}", e);
                return Err(e);
            },
        }
    } else {
        // Test with generated audio
        match crate::WhisperTranscriber::new(
            &config.transcription.model_path,
            config.transcription.use_cuda,
        )
        .await
        {
            Ok(transcriber) => {
                println!("✅ Transcriber initialized");
                println!("   Device: {}", transcriber.get_device_info());
                println!("   CUDA: {}", transcriber.is_using_cuda());

                if timing {
                    let start = std::time::Instant::now();
                    let test_audio = vec![0.1f32; 48000]; // 3s of test data
                    match transcriber.transcribe_async(&test_audio, 16000).await {
                        Ok(result) => {
                            let elapsed = start.elapsed();
                            println!("✅ Test transcription completed in {:?}", elapsed);
                            println!("   Result: '{}'", result.text);
                            println!("   Confidence: {:.2}", result.confidence);
                        },
                        Err(e) => println!("❌ Transcription failed: {}", e),
                    }
                } else {
                    println!("✅ Transcription system ready");
                }
            },
            Err(e) => println!("❌ Transcription system test failed: {}", e),
        }
    }

    if all_models {
        println!("🚧 Multi-model testing not yet implemented");
    }

    Ok(())
}

pub async fn test_text_insertion_system(
    text: String,
    all_methods: bool,
    uinput: bool,
) -> Result<()> {
    println!("⌨️ Testing text insertion system...");
    println!("Text to insert: '{}'", text);

    #[cfg(target_os = "linux")]
    match crate::TextInserter::new() {
        Ok(mut inserter) => {
            if uinput || all_methods {
                println!("Testing UInput method...");
                // Test UInput specifically
            }

            println!("⚠️ About to insert text at cursor position!");
            println!("Press Enter to continue (you have 3 seconds to position cursor)...");
            std::io::stdin()
                .read_line(&mut String::new())
                .context("Failed to read user input")?;

            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

            match inserter.insert_text(&text) {
                Ok(()) => println!("✅ Text insertion successful"),
                Err(e) => println!("❌ Text insertion failed: {}", e),
            }
        },
        Err(e) => println!("❌ Text insertion system test failed: {}", e),
    }

    #[cfg(not(target_os = "linux"))]
    {
        println!("ℹ️  Text insertion system test is Linux-only");
        println!("💡 Use trait-based adapters for platform-specific text insertion");
    }

    Ok(())
}

pub async fn test_hotkey_system(combination: Option<String>, duration: u64) -> Result<()> {
    let config = crate::Config::load()?;
    let hotkey_combo = combination.unwrap_or(config.hotkey.combination);

    println!("⌨️ Testing hotkey system...");
    println!("Combination: {}", hotkey_combo);
    println!("Test duration: {}s", duration);

    match crate::hotkey::HotkeyManager::new(&hotkey_combo) {
        Ok((manager, receiver)) => match manager.start_listening() {
            Ok(()) => {
                println!("✅ Hotkey listener started");
                println!(
                    "Press {} to test (listening for {}s)...",
                    hotkey_combo, duration
                );

                let start = std::time::Instant::now();
                let timeout = std::time::Duration::from_secs(duration);
                let mut events = 0;

                while start.elapsed() < timeout {
                    if let Ok(event) = receiver.try_recv() {
                        events += 1;
                        match event {
                            crate::hotkey::HotkeyEvent::Pressed => {
                                println!("🎯 Hotkey pressed detected!");
                            },
                            crate::hotkey::HotkeyEvent::Released => {
                                println!("🎯 Hotkey released detected!");
                            },
                        }
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                }

                if events == 0 {
                    println!("ℹ️ No hotkey events detected");
                } else {
                    println!("✅ Detected {} hotkey events", events);
                }

                manager.stop_listening()?;
            },
            Err(e) => println!("❌ Failed to start hotkey listening: {}", e),
        },
        Err(e) => println!("❌ Hotkey system test failed: {}", e),
    }

    Ok(())
}

pub async fn test_full_pipeline(count: u32, transcribe_only: bool) -> Result<()> {
    println!("🔄 Testing complete voice-to-text pipeline...");

    for i in 1..=count {
        if count > 1 {
            println!("\n📹 Test {}/{}", i, count);
        }

        // This would run the equivalent of test-voice-to-text
        let config = crate::Config::load()?;

        // Initialize components
        let mut audio_capture = crate::AudioCapture::new(config.audio.device.as_deref())?;
        let transcriber = crate::WhisperTranscriber::new(
            &config.transcription.model_path,
            config.transcription.use_cuda,
        )
        .await?;
        let mut text_inserter;
        #[cfg(target_os = "linux")]
        {
            text_inserter = if transcribe_only {
                None
            } else {
                Some(crate::TextInserter::new()?)
            };
        }

        #[cfg(not(target_os = "linux"))]
        {
            text_inserter = None::<()>;
            if !transcribe_only {
                println!("ℹ️  Text insertion not available on this platform");
            }
        }

        println!("Press Enter to start recording...");
        std::io::stdin()
            .read_line(&mut String::new())
            .context("Failed to read user input")?;

        println!("🎤 Recording... (press Enter to stop)");
        audio_capture.start_recording()?;

        std::io::stdin()
            .read_line(&mut String::new())
            .context("Failed to read user input")?;

        let audio_data = audio_capture.stop_recording()?;
        println!("✅ Recording stopped ({} samples)", audio_data.len());

        println!("🗣️ Transcribing...");
        match transcriber
            .transcribe_async(&audio_data, config.audio.sample_rate)
            .await
        {
            Ok(result) => {
                println!("✅ Transcription: '{}'", result.text);

                #[cfg(target_os = "linux")]
                if let Some(ref mut inserter) = text_inserter {
                    println!("⌨️ Inserting text (3 second delay)...");
                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

                    match inserter.insert_text(&result.text) {
                        Ok(()) => println!("✅ Text inserted successfully"),
                        Err(e) => println!("❌ Text insertion failed: {}", e),
                    }
                }
            },
            Err(e) => println!("❌ Transcription failed: {}", e),
        }
    }

    Ok(())
}

pub async fn run_all_tests(benchmarks: bool, output: Option<PathBuf>) -> Result<()> {
    println!("🧪 Running all system tests...\n");

    // Test audio
    println!("=== Audio System Test ===");
    test_audio_system(2, false, None, None).await?;

    println!("\n=== Transcription System Test ===");
    test_transcription_system(None, false, true).await?;

    println!("\n=== Text Insertion System Test ===");
    // Skip interactive text insertion in full test
    #[cfg(target_os = "linux")]
    {
        match crate::TextInserter::new() {
            Ok(_) => println!("✅ Text insertion system available"),
            Err(e) => println!("❌ Text insertion system failed: {}", e),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        println!("ℹ️  Text insertion test skipped (platform-specific)");
    }

    println!("\n=== Hotkey System Test ===");
    test_hotkey_system(None, 2).await?;

    if benchmarks {
        println!("\n=== Performance Benchmarks ===");
        println!("🚧 Benchmarks not yet implemented");
    }

    if let Some(output_file) = output {
        println!("\n💾 Saving results to: {}", output_file.display());
        println!("🚧 Result saving not yet implemented");
    }

    println!("\n✅ All tests completed!");
    Ok(())
}

// Status functions
async fn show_config_status() -> Result<()> {
    println!("⚙️ Configuration Status:");
    match crate::Config::load() {
        Ok(config) => {
            println!("✅ Configuration loaded successfully");
            println!("   Audio device: {:?}", config.audio.device);
            println!("   Sample rate: {}", config.audio.sample_rate);
            println!("   Hotkey: {}", config.hotkey.combination);
            println!(
                "   Model path: {}",
                config.transcription.model_path.display()
            );
            println!("   CUDA enabled: {}", config.transcription.use_cuda);
        },
        Err(e) => println!("❌ Configuration error: {}", e),
    }
    Ok(())
}

async fn show_device_status() -> Result<()> {
    println!("🎵 Device Status:");
    match crate::AudioCapture::list_devices() {
        Ok(devices) => {
            println!("Available audio devices:");
            for (i, device) in devices.iter().enumerate() {
                println!("  {}. {}", i + 1, device);
            }
        },
        Err(e) => println!("❌ Failed to list devices: {}", e),
    }
    Ok(())
}

// Installation functions
async fn install_autostart() -> Result<()> {
    println!("📥 Installing autostart entry...");

    // Get XDG autostart directory
    let config_home = env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
        env::var("HOME")
            .map(|home| format!("{}/.config", home))
            .unwrap_or_else(|_| ".config".to_string())
    });
    let autostart_dir = PathBuf::from(config_home).join("autostart");

    // Create autostart directory if it doesn't exist
    fs::create_dir_all(&autostart_dir).with_context(|| {
        format!(
            "Failed to create autostart directory: {}",
            autostart_dir.display()
        )
    })?;

    let autostart_file = autostart_dir.join("hush.desktop");

    // Get current executable path
    let exe_path = env::current_exe().with_context(|| "Failed to determine executable path")?;

    // Create autostart desktop entry
    let desktop_content = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Hush Voice-to-Text\n\
         Comment=Fast, accurate voice-to-text for Linux developers\n\
         Exec={} listen\n\
         Icon=audio-input-microphone\n\
         Terminal=false\n\
         Categories=Utility;Accessibility;\n\
         X-GNOME-Autostart-enabled=true\n",
        exe_path.display()
    );

    fs::write(&autostart_file, desktop_content).with_context(|| {
        format!(
            "Failed to write autostart file: {}",
            autostart_file.display()
        )
    })?;

    println!(
        "   ✅ Autostart entry installed: {}",
        autostart_file.display()
    );
    println!("   ℹ️  Hush will start in listen mode on login");
    Ok(())
}

async fn install_desktop_entry() -> Result<()> {
    println!("📥 Installing desktop entry...");

    // Get XDG data directory
    let data_home = env::var("XDG_DATA_HOME").unwrap_or_else(|_| {
        env::var("HOME")
            .map(|home| format!("{}/.local/share", home))
            .unwrap_or_else(|_| ".local/share".to_string())
    });
    let applications_dir = PathBuf::from(data_home).join("applications");

    // Create applications directory if it doesn't exist
    fs::create_dir_all(&applications_dir).with_context(|| {
        format!(
            "Failed to create applications directory: {}",
            applications_dir.display()
        )
    })?;

    let desktop_file = applications_dir.join("hush.desktop");

    // Get current executable path
    let exe_path = env::current_exe().with_context(|| "Failed to determine executable path")?;

    // Create desktop entry
    let desktop_content = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Hush Voice-to-Text\n\
         GenericName=Voice-to-Text\n\
         Comment=Fast, accurate voice-to-text with GPU acceleration\n\
         Exec={} listen\n\
         Icon=audio-input-microphone\n\
         Terminal=false\n\
         Categories=Utility;Accessibility;AudioVideo;\n\
         Keywords=voice;speech;dictation;transcription;whisper;\n\
         StartupNotify=false\n\
         Actions=Record;Listen;Status;\n\
         \n\
         [Desktop Action Record]\n\
         Name=Quick Record\n\
         Exec={} record --duration 10\n\
         \n\
         [Desktop Action Listen]\n\
         Name=Start Listening Mode\n\
         Exec={} listen\n\
         \n\
         [Desktop Action Status]\n\
         Name=Check Status\n\
         Exec={} status --full\n",
        exe_path.display(),
        exe_path.display(),
        exe_path.display(),
        exe_path.display()
    );

    fs::write(&desktop_file, desktop_content)
        .with_context(|| format!("Failed to write desktop file: {}", desktop_file.display()))?;

    println!("   ✅ Desktop entry installed: {}", desktop_file.display());
    println!("   ℹ️  Hush should now appear in your application menu");

    // Try to update desktop database
    if let Ok(output) = std::process::Command::new("update-desktop-database")
        .arg(&applications_dir)
        .output()
    {
        if output.status.success() {
            println!("   ✅ Desktop database updated");
        }
    }

    Ok(())
}

async fn install_system_wide() -> Result<()> {
    println!("📥 Installing system-wide...");

    // Get current executable path
    let exe_path = env::current_exe().with_context(|| "Failed to determine executable path")?;

    let target_path = PathBuf::from("/usr/local/bin/hush");

    // Check if we need sudo
    if target_path.exists() {
        println!("   ⚠️  {} already exists", target_path.display());
        print!("   Continue and overwrite? [y/N] ");
        use std::io::{self, Write};
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("   ❌ Installation cancelled");
            return Ok(());
        }
    }

    println!("   ℹ️  This operation requires sudo privileges");
    println!(
        "   Running: sudo cp {} {}",
        exe_path.display(),
        target_path.display()
    );

    let status = std::process::Command::new("sudo")
        .arg("cp")
        .arg(&exe_path)
        .arg(&target_path)
        .status()
        .with_context(|| "Failed to execute sudo cp command")?;

    if !status.success() {
        anyhow::bail!("Failed to copy executable to {}", target_path.display());
    }

    // Make it executable
    let status = std::process::Command::new("sudo")
        .arg("chmod")
        .arg("+x")
        .arg(&target_path)
        .status()
        .with_context(|| "Failed to execute sudo chmod command")?;

    if !status.success() {
        anyhow::bail!("Failed to make {} executable", target_path.display());
    }

    println!(
        "   ✅ System-wide installation complete: {}",
        target_path.display()
    );
    println!("   ℹ️  You can now run 'hush' from anywhere");
    Ok(())
}

async fn remove_autostart() -> Result<()> {
    println!("🗑️  Removing autostart entry...");

    let config_home = env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
        env::var("HOME")
            .map(|home| format!("{}/.config", home))
            .unwrap_or_else(|_| ".config".to_string())
    });
    let autostart_file = PathBuf::from(config_home)
        .join("autostart")
        .join("hush.desktop");

    if autostart_file.exists() {
        fs::remove_file(&autostart_file).with_context(|| {
            format!(
                "Failed to remove autostart file: {}",
                autostart_file.display()
            )
        })?;
        println!(
            "   ✅ Autostart entry removed: {}",
            autostart_file.display()
        );
    } else {
        println!("   ℹ️  No autostart entry found");
    }

    Ok(())
}

async fn remove_desktop_entry() -> Result<()> {
    println!("🗑️  Removing desktop entry...");

    let data_home = env::var("XDG_DATA_HOME").unwrap_or_else(|_| {
        env::var("HOME")
            .map(|home| format!("{}/.local/share", home))
            .unwrap_or_else(|_| ".local/share".to_string())
    });
    let desktop_file = PathBuf::from(&data_home)
        .join("applications")
        .join("hush.desktop");

    if desktop_file.exists() {
        fs::remove_file(&desktop_file).with_context(|| {
            format!("Failed to remove desktop file: {}", desktop_file.display())
        })?;
        println!("   ✅ Desktop entry removed: {}", desktop_file.display());

        // Try to update desktop database
        let applications_dir = PathBuf::from(&data_home).join("applications");
        if let Ok(output) = std::process::Command::new("update-desktop-database")
            .arg(&applications_dir)
            .output()
        {
            if output.status.success() {
                println!("   ✅ Desktop database updated");
            }
        }
    } else {
        println!("   ℹ️  No desktop entry found");
    }

    Ok(())
}

async fn remove_system_wide() -> Result<()> {
    println!("🗑️  Removing system-wide installation...");

    let target_path = PathBuf::from("/usr/local/bin/hush");

    if !target_path.exists() {
        println!("   ℹ️  No system-wide installation found");
        return Ok(());
    }

    println!("   ℹ️  This operation requires sudo privileges");
    println!("   Running: sudo rm {}", target_path.display());

    let status = std::process::Command::new("sudo")
        .arg("rm")
        .arg(&target_path)
        .status()
        .with_context(|| "Failed to execute sudo rm command")?;

    if !status.success() {
        anyhow::bail!("Failed to remove {}", target_path.display());
    }

    println!(
        "   ✅ System-wide installation removed: {}",
        target_path.display()
    );
    Ok(())
}
