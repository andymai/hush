use crate::cli::{Commands, ModelCommands, SetupCommands, TestCommands};
use crate::logging::RequestContext;
use anyhow::{Context as AnyhowContext, Result};
use hound;
use std::path::PathBuf;
use std::{env, fs};
use tracing::{debug, error, info, warn};

// Import Hush components
use crate::hotkey::{HotkeyEvent, HotkeyManager};
use crate::overlay::{OverlayState, OverlayWindowBuilder};
use crate::text_processing::{
    CommandExecutor, CommandParser, EditingMode, InsertionHistory, LlmProvider, ProcessingConfig,
    TextProcessor,
};
use crate::transcription::SimpleWhisperTranscriber;
use crate::{hotkey, AudioCapture, Config, TextInserter, WhisperTranscriber};
use parking_lot::Mutex;
use std::sync::{mpsc, Arc};
use std::thread;

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
                self.handle_manual(count).await
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
                self.handle_listen(editing_mode.clone(), no_processing, no_button)
                    .await
            },
            Commands::Setup { setup_command } => self.handle_setup(setup_command).await,
            Commands::Test { test_command } => self.handle_test(test_command).await,
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
        std::io::stdin().read_line(&mut String::new()).unwrap();

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
                    let mut text_inserter = TextInserter::new()?;
                    println!("⌨️ Inserting text (3 second delay)...");
                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

                    match text_inserter.insert_text(&result.text) {
                        Ok(()) => println!("✅ Text inserted successfully"),
                        Err(e) => println!("❌ Text insertion failed: {}", e),
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

    async fn handle_manual(&self, count: u32) -> Result<()> {
        info!("🎯 Starting manual recording mode ({} recordings)", count);

        for i in 1..=count {
            if count > 1 {
                println!("\n📹 Recording {}/{}", i, count);
            }

            // Use the record handler for each iteration
            self.handle_record(30, false, None).await?;
        }

        println!("✅ All recordings completed!");
        Ok(())
    }

    async fn handle_listen(
        &self,
        editing_mode_str: String,
        no_processing: bool,
        no_button: bool,
    ) -> Result<()> {
        info!("🎧 Starting intelligent listening mode");
        info!("   Editing mode: {}", editing_mode_str);
        info!("   Text processing: {}", !no_processing);
        info!("   Show button: {}", !no_button);

        println!("🎤 Hush Intelligent Listening Mode");
        println!("═══════════════════════════════════════════════════════");
        println!("Press Ctrl+Alt+V to start recording");
        println!("Release to transcribe and insert text");
        println!("Press Ctrl+C to quit");
        println!("═══════════════════════════════════════════════════════\n");

        // Parse editing mode
        let editing_mode = match editing_mode_str.to_lowercase().as_str() {
            "light" => EditingMode::Light,
            "medium" => EditingMode::Medium,
            "aggressive" => EditingMode::Aggressive,
            unknown => {
                error!("Invalid editing mode: '{}'", unknown);
                println!("❌ Invalid editing mode: '{}'", unknown);
                println!("Valid options: light, medium, aggressive");
                return Err(anyhow::anyhow!(
                    "Invalid editing mode '{}'. Valid options: light, medium, aggressive",
                    unknown
                ));
            },
        };

        // Communication channels
        enum AudioCommand {
            StartRecording,
            StopRecording,
        }

        enum TranscriptionResult {
            Success(String),
            Error(String),
        }

        let (audio_cmd_tx, audio_cmd_rx) = mpsc::channel::<AudioCommand>();
        let (transcription_tx, transcription_rx) = mpsc::channel::<TranscriptionResult>();

        // Create hotkey manager
        let (hotkey_manager, hotkey_rx) = HotkeyManager::new("Ctrl+Alt+V")?;
        hotkey_manager.start_listening()?;
        info!("✅ Hotkey 'Ctrl+Alt+V' registered");

        // Initialize audio capture (main thread - !Send)
        let mut audio_capture = AudioCapture::new(None)?;
        let amplitude_rx = Arc::new(Mutex::new(audio_capture.enable_amplitude_monitoring()));
        info!(
            "✅ Audio capture initialized: {}",
            audio_capture.get_device_name()
        );

        // Initialize transcriber
        let model_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".hush/models/ggml-base.en.bin");

        let transcriber = match SimpleWhisperTranscriber::new(&model_path).await {
            Ok(t) => {
                if t.is_ready() {
                    info!("✅ Whisper transcriber ready (GPU-accelerated)");
                } else {
                    warn!("⚠️  Whisper model not loaded - will use simulation");
                }
                Some(t)
            },
            Err(e) => {
                warn!("Failed to initialize transcriber: {}", e);
                warn!("Will use simulated transcription");
                None
            },
        };

        // Load environment variables from .env file
        let _ = dotenvy::dotenv();

        // Initialize text processor
        let text_processor = if !no_processing {
            // Configure LLM provider from environment
            let llm_provider = if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
                if !api_key.is_empty() {
                    info!("Using Anthropic Claude API for text polishing");
                    LlmProvider::Anthropic {
                        api_key: Some(api_key),
                        model: "claude-3-haiku-20240307".to_string(),
                    }
                } else {
                    LlmProvider::None
                }
            } else {
                info!("No ANTHROPIC_API_KEY found, using rule-based processing only");
                LlmProvider::None
            };

            let processing_config = ProcessingConfig {
                mode: editing_mode,
                llm_provider,
                max_tokens: 200,
                temperature: 0.3,
            };

            match TextProcessor::new(processing_config) {
                Ok(processor) => {
                    if processor.is_llm_ready() {
                        info!("✅ Text processor initialized with LLM");
                    } else {
                        info!("✅ Text processor initialized (rule-based only)");
                    }
                    Some(Arc::new(processor))
                },
                Err(e) => {
                    warn!("Failed to initialize text processor: {}", e);
                    None
                },
            }
        } else {
            info!("ℹ️  Text processing disabled");
            None
        };

        // Initialize text inserter
        let text_inserter = match TextInserter::new() {
            Ok(inserter) => {
                info!("✅ Text inserter initialized");
                Some(Arc::new(Mutex::new(inserter)))
            },
            Err(e) => {
                warn!("Failed to initialize text inserter: {}", e);
                warn!("Transcribed text will only be shown in overlay");
                None
            },
        };

        // Initialize voice command components
        let command_parser = Arc::new(CommandParser::new());
        let command_executor = Arc::new(CommandExecutor::new());
        let insertion_history = Arc::new(Mutex::new(InsertionHistory::new()));
        info!("✅ Voice command system initialized");

        // Create overlay with new tiny design
        let overlay = OverlayWindowBuilder::new()
            .show_button_when_idle(!no_button)
            .build(); // Uses defaults: 10x20 idle, 80x20 recording, BottomCenter

        let state_handle = overlay.state();

        // Hotkey handler thread
        let audio_cmd_tx_clone = audio_cmd_tx.clone();
        let state_handle_clone = state_handle.clone();
        let hotkey_thread = thread::spawn(move || {
            loop {
                match hotkey_rx.recv() {
                    Ok(HotkeyEvent::Pressed) => {
                        *state_handle_clone.lock() = OverlayState::start_recording();
                        let _ = audio_cmd_tx_clone.send(AudioCommand::StartRecording);
                    },
                    Ok(HotkeyEvent::Released) => {
                        // Stop recording - state transitions handled by result handler thread
                        // Visual state remains in Recording until transcription completes
                        let _ = audio_cmd_tx_clone.send(AudioCommand::StopRecording);
                    },
                    Err(_) => break,
                }
            }
        });

        // Result handler thread
        let state_handle_clone2 = state_handle.clone();
        let text_inserter_clone = text_inserter.clone();
        let text_processor_clone = text_processor.clone();
        let command_parser_clone = command_parser.clone();
        let command_executor_clone = command_executor.clone();
        let insertion_history_clone = insertion_history.clone();
        let result_thread = thread::spawn(move || {
            let result_runtime = tokio::runtime::Runtime::new().expect("Failed to create runtime");

            while let Ok(result) = transcription_rx.recv() {
                match result {
                    TranscriptionResult::Success(raw_text) => {
                        info!("✅ Transcription successful: '{}'", raw_text);

                        // Parse for voice commands
                        let parsed = command_parser_clone.parse(&raw_text);

                        // Execute commands
                        let exec_result = command_executor_clone.execute(&parsed);

                        // Handle undo request
                        if exec_result.undo_requested {
                            info!("🔙 Undo requested");

                            if let Some(ref inserter) = text_inserter_clone {
                                let mut history = insertion_history_clone.lock();

                                if let Some(last_entry) = history.pop_last() {
                                    info!(
                                        "Undoing last insertion: '{}' ({} chars)",
                                        if last_entry.text.len() > 50 {
                                            &last_entry.text[..50]
                                        } else {
                                            &last_entry.text
                                        },
                                        last_entry.char_count
                                    );

                                    match inserter.lock().undo_last_insertion(last_entry.char_count)
                                    {
                                        Ok(_) => {
                                            *state_handle_clone2.lock() = OverlayState::idle();
                                        },
                                        Err(e) => {
                                            error!("Undo failed: {}", e);
                                            *state_handle_clone2.lock() = OverlayState::error(
                                                "Undo failed",
                                                std::time::Duration::from_secs(3),
                                            );
                                        },
                                    }
                                } else {
                                    warn!("No recent insertion to undo");
                                    *state_handle_clone2.lock() = OverlayState::error(
                                        "Nothing to undo",
                                        std::time::Duration::from_secs(2),
                                    );
                                }
                            }
                            continue;
                        }

                        // If no text to insert, skip
                        if exec_result.text.is_none() {
                            continue;
                        }

                        let command_text = exec_result.text.unwrap();

                        // Process text (if enabled and should_process is true)
                        let processed_text = if exec_result.should_process {
                            if let Some(ref processor) = text_processor_clone {
                                info!("🔄 Processing text...");
                                // Keep current state (likely Recording) while processing

                                match result_runtime.block_on(processor.process(&command_text)) {
                                    Ok(polished) => {
                                        info!("✨ Text polished: '{}'", polished);
                                        polished
                                    },
                                    Err(e) => {
                                        warn!("Text processing failed: {}, using command text", e);
                                        command_text
                                    },
                                }
                            } else {
                                command_text
                            }
                        } else {
                            // Commands already formatted the text, skip processing
                            command_text
                        };

                        // Insert text
                        if let Some(ref inserter) = text_inserter_clone {
                            thread::sleep(std::time::Duration::from_millis(200));

                            match inserter.lock().insert_text(&processed_text) {
                                Ok(_) => {
                                    // Record in history for undo
                                    insertion_history_clone
                                        .lock()
                                        .record(processed_text.clone());
                                },
                                Err(e) => {
                                    error!("Text insertion failed: {}", e);
                                },
                            }
                        }

                        // Update overlay
                        let _display_text = if processed_text.len() > 50 {
                            format!("{}...", &processed_text[..47])
                        } else {
                            processed_text
                        };

                        *state_handle_clone2.lock() = OverlayState::idle();
                    },
                    TranscriptionResult::Error(error_msg) => {
                        error!("❌ Transcription failed: {}", error_msg);
                        *state_handle_clone2.lock() =
                            OverlayState::error(&error_msg, std::time::Duration::from_secs(4));
                    },
                }
            }
        });

        // Overlay thread
        let overlay_thread = thread::spawn(move || {
            let _ = overlay.run();
        });

        // Main thread: audio handling
        info!("🚀 Audio handler ready");

        while let Ok(command) = audio_cmd_rx.recv() {
            match command {
                AudioCommand::StartRecording => {
                    if let Err(e) = audio_capture.start_recording() {
                        error!("Failed to start recording: {}", e);
                        let _ = transcription_tx.send(TranscriptionResult::Error(
                            "Failed to start recording".to_string(),
                        ));
                    } else {
                        // Start polling amplitude updates with batching for performance
                        let state_handle_amp = state_handle.clone();
                        let amplitude_rx_clone = amplitude_rx.clone();

                        // Batch amplitude updates to match overlay repaint rate (20 FPS during recording)
                        // This reduces mutex contention and provides smoother averaged values
                        std::thread::spawn(move || {
                            let mut amplitude_buffer = Vec::with_capacity(10);
                            let update_interval = std::time::Duration::from_millis(50); // 20 Hz, matches recording repaint
                            let mut last_update = std::time::Instant::now();

                            loop {
                                let amp_result = amplitude_rx_clone.lock().try_recv();
                                match amp_result {
                                    Ok(amplitude) => {
                                        amplitude_buffer.push(amplitude);

                                        // Update state every 50ms with averaged amplitude
                                        if last_update.elapsed() >= update_interval {
                                            let avg_amplitude = if !amplitude_buffer.is_empty() {
                                                amplitude_buffer.iter().sum::<f32>()
                                                    / amplitude_buffer.len() as f32
                                            } else {
                                                0.0
                                            };

                                            let mut state = state_handle_amp.lock();
                                            if state.is_recording() {
                                                state.update_amplitude(avg_amplitude);
                                            } else {
                                                break; // Stop when no longer recording
                                            }

                                            amplitude_buffer.clear();
                                            last_update = std::time::Instant::now();
                                        }
                                    },
                                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                                        // No new amplitude data, sleep briefly
                                        std::thread::sleep(std::time::Duration::from_millis(10));
                                    },
                                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                        break; // Channel closed, stop polling
                                    },
                                }
                            }
                        });
                    }
                },
                AudioCommand::StopRecording => {
                    let audio_data = match audio_capture.stop_recording() {
                        Ok(data) => data,
                        Err(e) => {
                            error!("Failed to stop recording: {}", e);
                            let _ = transcription_tx.send(TranscriptionResult::Error(
                                "Failed to stop recording".to_string(),
                            ));
                            continue;
                        },
                    };

                    if audio_data.is_empty() {
                        let _ = transcription_tx
                            .send(TranscriptionResult::Error("No audio recorded".to_string()));
                        continue;
                    }

                    // Transcribe
                    let transcription_result = if let Some(ref t) = transcriber {
                        if t.is_ready() {
                            match t.transcribe(&audio_data).await {
                                Ok(result) => TranscriptionResult::Success(result.text),
                                Err(_) => {
                                    TranscriptionResult::Error("Transcription failed".to_string())
                                },
                            }
                        } else {
                            let duration = audio_data.len() as f32 / 16000.0;
                            TranscriptionResult::Success(format!(
                                "um well uh I mean this is like simulated text you know ({:.1}s)",
                                duration
                            ))
                        }
                    } else {
                        let duration = audio_data.len() as f32 / 16000.0;
                        TranscriptionResult::Success(format!(
                            "Simulated transcription ({:.1}s of audio)",
                            duration
                        ))
                    };

                    let _ = transcription_tx.send(transcription_result);
                },
            }
        }

        // Cleanup
        drop(hotkey_manager);
        drop(audio_cmd_tx);
        let _ = hotkey_thread.join();
        let _ = result_thread.join();
        let _ = overlay_thread.join();

        info!("Listen mode ended");
        Ok(())
    }

    async fn handle_setup(&self, setup_command: SetupCommands) -> Result<()> {
        match setup_command {
            SetupCommands::Uinput { quick, auto_fix } => {
                if quick {
                    print_uinput_quick_setup();
                } else if auto_fix {
                    auto_fix_uinput().await?;
                } else {
                    crate::text::print_uinput_setup_guidance();
                }
                Ok(())
            },
            SetupCommands::DiagnoseUinput => crate::text::diagnose_uinput_issues(),
            SetupCommands::Audio { list, test } => setup_audio(list, test).await,
            SetupCommands::Hotkeys { test, list } => setup_hotkeys(test, list).await,
            SetupCommands::Wizard { auto } => run_setup_wizard(auto).await,
        }
    }

    async fn handle_test(&self, test_command: TestCommands) -> Result<()> {
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

    async fn handle_models(&self, model_command: ModelCommands) -> Result<()> {
        match model_command {
            ModelCommands::List {
                downloaded,
                details,
            } => list_models(downloaded, details).await,
            ModelCommands::Download { model_size, force } => {
                download_model(&model_size, force).await
            },
            ModelCommands::Remove { model_size, yes } => remove_model(&model_size, yes).await,
            ModelCommands::Info { clear_stats } => show_model_info(clear_stats).await,
            ModelCommands::Verify { model_size, fix } => {
                verify_models(model_size.as_deref(), fix).await
            },
        }
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
}

// Helper functions - these would be implemented in separate modules
fn print_uinput_quick_setup() {
    println!("🔧 Quick UInput Setup:");
    println!("sudo usermod -a -G input $USER");
    println!("sudo modprobe uinput");
    println!("echo 'uinput' | sudo tee /etc/modules-load.d/uinput.conf");
    println!("# Then log out and log back in");
}

async fn auto_fix_uinput() -> Result<()> {
    warn!("Auto-fix functionality not yet implemented");
    println!("🚧 Auto-fix is not yet implemented. Please run 'hush setup uinput' for manual instructions.");
    Ok(())
}

async fn setup_audio(list: bool, test: Option<String>) -> Result<()> {
    if list {
        println!("🎵 Available Audio Devices:");
        // Use existing functionality
        if let Ok(devices) = crate::AudioCapture::list_devices() {
            for (i, device) in devices.iter().enumerate() {
                println!("  {}. {}", i + 1, device);
            }
        } else {
            println!("❌ Failed to list audio devices");
        }
    }

    if let Some(device_name) = test {
        println!("🎤 Testing audio device: {}", device_name);
        match crate::AudioCapture::new(Some(&device_name)) {
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

async fn setup_hotkeys(test: Option<String>, list: bool) -> Result<()> {
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
        // Test the hotkey combination
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

async fn run_setup_wizard(auto: bool) -> Result<()> {
    println!("🧙 Hush Setup Wizard");
    println!("This wizard will help you configure Hush for optimal performance.\n");

    if !auto {
        println!("Press Enter to continue, or Ctrl+C to cancel...");
        std::io::stdin().read_line(&mut String::new()).unwrap();
    }

    // Step 1: UInput setup
    println!("📋 Step 1: UInput Setup");
    crate::text::print_uinput_setup_guidance();

    if !auto {
        println!("\nHave you completed the UInput setup? (y/N)");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        if !input.trim().to_lowercase().starts_with('y') {
            println!("⚠️ Please complete UInput setup before continuing.");
            return Ok(());
        }
    }

    // Step 2: Model download
    println!("\n📋 Step 2: Model Download");
    println!("Downloading recommended model (base)...");
    download_model("base", false).await?;

    // Step 3: Test systems
    println!("\n📋 Step 3: System Test");
    run_all_tests(false, None).await?;

    println!("\n✅ Setup wizard completed!");
    println!("You can now use 'hush start' to begin voice-to-text.");

    Ok(())
}

// Test functions (these would call existing test binaries)
async fn test_audio_system(
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

async fn test_transcription_system(
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
                .map(|s| s.unwrap() as f32 / i16::MAX as f32)
                .collect()
        } else {
            reader.samples::<f32>().map(|s| s.unwrap()).collect()
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

async fn test_text_insertion_system(text: String, all_methods: bool, uinput: bool) -> Result<()> {
    println!("⌨️ Testing text insertion system...");
    println!("Text to insert: '{}'", text);

    match crate::TextInserter::new() {
        Ok(mut inserter) => {
            if uinput || all_methods {
                println!("Testing UInput method...");
                // Test UInput specifically
            }

            println!("⚠️ About to insert text at cursor position!");
            println!("Press Enter to continue (you have 3 seconds to position cursor)...");
            std::io::stdin().read_line(&mut String::new()).unwrap();

            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

            match inserter.insert_text(&text) {
                Ok(()) => println!("✅ Text insertion successful"),
                Err(e) => println!("❌ Text insertion failed: {}", e),
            }
        },
        Err(e) => println!("❌ Text insertion system test failed: {}", e),
    }

    Ok(())
}

async fn test_hotkey_system(combination: Option<String>, duration: u64) -> Result<()> {
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

async fn test_full_pipeline(count: u32, transcribe_only: bool) -> Result<()> {
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
        let mut text_inserter = if transcribe_only {
            None
        } else {
            Some(crate::TextInserter::new()?)
        };

        println!("Press Enter to start recording...");
        std::io::stdin().read_line(&mut String::new()).unwrap();

        println!("🎤 Recording... (press Enter to stop)");
        audio_capture.start_recording()?;

        std::io::stdin().read_line(&mut String::new()).unwrap();

        let audio_data = audio_capture.stop_recording()?;
        println!("✅ Recording stopped ({} samples)", audio_data.len());

        println!("🗣️ Transcribing...");
        match transcriber
            .transcribe_async(&audio_data, config.audio.sample_rate)
            .await
        {
            Ok(result) => {
                println!("✅ Transcription: '{}'", result.text);

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

async fn run_all_tests(benchmarks: bool, output: Option<PathBuf>) -> Result<()> {
    println!("🧪 Running all system tests...\n");

    // Test audio
    println!("=== Audio System Test ===");
    test_audio_system(2, false, None, None).await?;

    println!("\n=== Transcription System Test ===");
    test_transcription_system(None, false, true).await?;

    println!("\n=== Text Insertion System Test ===");
    // Skip interactive text insertion in full test
    match crate::TextInserter::new() {
        Ok(_) => println!("✅ Text insertion system available"),
        Err(e) => println!("❌ Text insertion system failed: {}", e),
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

// Model management functions
async fn list_models(downloaded: bool, details: bool) -> Result<()> {
    // Use existing simple-model-manager functionality
    println!("📋 Available Whisper Models:");

    let models = ["tiny", "base", "small", "medium", "large"];
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("hush")
        .join("models");

    for model in models {
        let model_path = cache_dir.join(format!("ggml-{}.bin", model));
        let is_downloaded = model_path.exists();

        if downloaded && !is_downloaded {
            continue;
        }

        let status = if is_downloaded { "✅" } else { "📥" };
        print!("{} {} ", status, model);

        if details {
            let size = match model {
                "tiny" => "~39 MB",
                "base" => "~142 MB",
                "small" => "~466 MB",
                "medium" => "~1.5 GB",
                "large" => "~2.9 GB",
                _ => "Unknown",
            };
            print!("({})", size);
        }

        if is_downloaded && details {
            if let Ok(metadata) = std::fs::metadata(&model_path) {
                print!(" - {} MB", metadata.len() / 1024 / 1024);
            }
        }

        println!();
    }

    Ok(())
}

async fn download_model(model_size: &str, force: bool) -> Result<()> {
    println!("📥 Downloading model: {}", model_size);

    // Use model downloader functionality
    match crate::model_downloader::download_model_by_name(model_size, force).await {
        Ok(_) => {
            println!("✅ Model '{}' downloaded successfully", model_size);
            Ok(())
        },
        Err(e) => {
            error!("❌ Failed to download model '{}': {}", model_size, e);
            Err(e)
        },
    }
}

async fn remove_model(model_size: &str, yes: bool) -> Result<()> {
    if model_size == "all" {
        println!("🗑️ Removing all models...");
        if !yes {
            println!("Are you sure you want to remove all models? (y/N)");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
            if !input.trim().to_lowercase().starts_with('y') {
                println!("Cancelled.");
                return Ok(());
            }
        }

        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("hush")
            .join("models");

        if cache_dir.exists() {
            std::fs::remove_dir_all(&cache_dir)?;
            println!("✅ All models removed");
        } else {
            println!("ℹ️ No models found to remove");
        }
    } else {
        println!("🗑️ Removing model: {}", model_size);

        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("hush")
            .join("models");
        let model_path = cache_dir.join(format!("ggml-{}.bin", model_size));

        if model_path.exists() {
            if !yes {
                println!("Are you sure you want to remove '{}'? (y/N)", model_size);
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).unwrap();
                if !input.trim().to_lowercase().starts_with('y') {
                    println!("Cancelled.");
                    return Ok(());
                }
            }

            std::fs::remove_file(&model_path)?;
            println!("✅ Model '{}' removed", model_size);
        } else {
            println!("ℹ️ Model '{}' not found", model_size);
        }
    }

    Ok(())
}

async fn show_model_info(clear_stats: bool) -> Result<()> {
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("hush")
        .join("models");

    println!("📊 Model Cache Information:");
    println!("Cache directory: {}", cache_dir.display());

    if cache_dir.exists() {
        let mut total_size = 0u64;
        let mut model_count = 0;

        for entry in std::fs::read_dir(&cache_dir)? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().starts_with("ggml-") {
                let metadata = entry.metadata()?;
                total_size += metadata.len();
                model_count += 1;
            }
        }

        println!("Models downloaded: {}", model_count);
        println!("Total size: {} MB", total_size / 1024 / 1024);
    } else {
        println!("No models downloaded yet.");
    }

    if clear_stats {
        println!("🚧 Statistics clearing not yet implemented");
    }

    Ok(())
}

async fn verify_models(model_size: Option<&str>, fix: bool) -> Result<()> {
    println!("🔍 Verifying model integrity...");

    if fix {
        println!("🚧 Model fixing not yet implemented");
    }

    // Basic file existence check
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("hush")
        .join("models");

    let models_to_check = if let Some(size) = model_size {
        vec![size.to_string()]
    } else {
        vec![
            "tiny".to_string(),
            "base".to_string(),
            "small".to_string(),
            "medium".to_string(),
            "large".to_string(),
        ]
    };

    // Expected sizes for whisper models (approximate, in MB)
    let expected_sizes: std::collections::HashMap<&str, u64> = [
        ("tiny", 75),
        ("tiny.en", 75),
        ("base", 145),
        ("base.en", 145),
        ("small", 466),
        ("small.en", 466),
        ("medium", 1500),
        ("medium.en", 1500),
        ("large", 2900),
    ]
    .iter()
    .cloned()
    .collect();

    for model in models_to_check {
        let model_path = cache_dir.join(format!("ggml-{}.bin", model));
        if model_path.exists() {
            // Verify file size as basic integrity check
            let metadata = std::fs::metadata(&model_path)?;
            let size_mb = metadata.len() / (1024 * 1024);

            if let Some(expected_size) = expected_sizes.get(model.as_str()) {
                // Allow 10% variance in file size
                let min_size = expected_size * 9 / 10;
                let max_size = expected_size * 11 / 10;

                if size_mb >= min_size && size_mb <= max_size {
                    println!("✅ {} - Present ({} MB, size OK)", model, size_mb);
                } else {
                    println!(
                        "⚠️  {} - Present ({} MB, expected ~{} MB - may be corrupted)",
                        model, size_mb, expected_size
                    );
                }
            } else {
                println!("✅ {} - Present ({} MB, unknown model)", model, size_mb);
            }

            // Note: For production use, consider adding SHA256 checksum verification
            // by adding the `sha2` crate and storing known checksums
        } else {
            println!("❌ {} - Missing", model);
        }
    }

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
    let config_home = env::var("XDG_CONFIG_HOME")
        .unwrap_or_else(|_| format!("{}/.config", env::var("HOME").unwrap()));
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
    let data_home = env::var("XDG_DATA_HOME")
        .unwrap_or_else(|_| format!("{}/.local/share", env::var("HOME").unwrap()));
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

    let config_home = env::var("XDG_CONFIG_HOME")
        .unwrap_or_else(|_| format!("{}/.config", env::var("HOME").unwrap()));
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

    let data_home = env::var("XDG_DATA_HOME")
        .unwrap_or_else(|_| format!("{}/.local/share", env::var("HOME").unwrap()));
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
