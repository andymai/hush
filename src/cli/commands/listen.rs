use anyhow::Result;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use std::thread;
use tracing::{error, info, warn};

// Import Hush components
use crate::hotkey::{HotkeyEvent, HotkeyManager};
use crate::overlay::{OverlayState, OverlayWindowBuilder};
use crate::text_processing::{
    CommandExecutor, CommandParser, EditingMode, InsertionHistory, LlmProvider, ProcessingConfig,
    TextProcessor,
};
use crate::transcription::SimpleWhisperTranscriber;
use crate::{AudioCapture, TextInserter};

/// Listen command implementation
///
/// Handles intelligent listening mode with hotkey-activated recording,
/// real-time transcription, text processing, and automated text insertion.

/// Handle the listen command
///
/// Starts intelligent listening mode with hotkey-activated voice recording.
/// This is the main interactive mode for Hush, providing real-time voice-to-text
/// with optional LLM-based text processing and voice commands.
///
/// # Arguments
///
/// * `editing_mode_str` - Text editing aggressiveness: "light", "medium", or "aggressive"
/// * `no_processing` - If true, skip LLM-based text processing (rule-based only)
/// * `no_button` - If true, hide the idle overlay button
///
/// # Examples
///
/// ```no_run
/// // Start with medium editing and LLM processing
/// handle_listen("medium".to_string(), false, false).await?;
///
/// // Start with light editing, no processing, no button
/// handle_listen("light".to_string(), true, true).await?;
/// ```
///
/// # Workflow
///
/// 1. **Initialization**
///    - Parse editing mode (light/medium/aggressive)
///    - Initialize hotkey manager (Ctrl+Alt+V)
///    - Setup audio capture with amplitude monitoring
///    - Load Whisper transcriber model
///    - Initialize text processor with LLM (if enabled)
///    - Initialize text inserter
///    - Setup voice command system (parser, executor, history)
///    - Create overlay window with tiny design
///
/// 2. **Multi-threaded Execution**
///    - **Hotkey Thread**: Listens for Ctrl+Alt+V press/release
///      - On press: Start recording, update overlay
///      - On release: Stop recording
///    - **Result Handler Thread**: Processes transcription results
///      - Parse voice commands (undo, new line, etc.)
///      - Execute commands
///      - Process text with LLM (if enabled)
///      - Insert text with 200ms delay
///      - Record in history for undo
///      - Update overlay state
///    - **Overlay Thread**: Manages visual feedback window
///    - **Main Thread**: Handles audio recording pipeline
///      - Start/stop recording on commands
///      - Monitor amplitude during recording
///      - Transcribe audio with Whisper
///      - Send results to handler thread
///
/// 3. **Voice Commands**
///    - "undo" or "scratch that": Undo last text insertion
///    - "new line": Insert line break
///    - Commands prevent LLM processing
///
/// 4. **Cleanup**
///    - On Ctrl+C: Drop all resources and join threads
///
/// # Error Handling
///
/// - Invalid editing mode: Returns error immediately
/// - Transcriber initialization failure: Continues with simulated mode
/// - Text processor initialization failure: Falls back to rule-based only
/// - Text inserter initialization failure: Only shows text in overlay
/// - Recording/transcription errors: Displays error in overlay
///
/// # Performance Notes
///
/// - Amplitude updates batched at 20 Hz to match overlay repaint rate
/// - Text insertion delayed 200ms for UI responsiveness
/// - Multi-threaded design keeps hotkey response instant
/// - Overlay runs in separate thread for smooth visuals
///
/// # Privacy
///
/// - All processing is local-first
/// - LLM calls only if ANTHROPIC_API_KEY is set
/// - Audio never leaves device unless LLM enabled
/// - No telemetry or external connections (except optional LLM)
pub async fn handle_listen(
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
        let result_runtime = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(e) => {
                error!(
                    "Failed to create tokio runtime for result processing: {}",
                    e
                );
                return;
            },
        };

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

                                match inserter.lock().undo_last_insertion(last_entry.char_count) {
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
                    let command_text = match exec_result.text {
                        Some(text) => text,
                        None => continue,
                    };

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
