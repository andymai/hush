/// Full integration test with intelligent auto-editing
///
/// This demonstrates the complete workflow with AI-powered text polishing:
/// 1. Press Ctrl+Alt+V to start recording (overlay shows "Recording")
/// 2. Speak into your microphone (REAL audio capture)
/// 3. Release key to stop and transcribe (overlay shows "Processing")
/// 4. Text is polished with AI (overlay shows "Editing")
/// 5. Clean text is inserted (overlay shows "Success")
///
/// Usage: cargo run --bin test-intelligent-editing
///
/// Note: Requires Whisper model and optionally an LLM model for polishing.
use hush::audio::AudioCapture;
use hush::hotkey::{HotkeyEvent, HotkeyManager};
use hush::overlay::{OverlayPosition, OverlayState, OverlayWindowBuilder};
use hush::text::TextInserter;
use hush::text_processing::{EditingMode, LlmProvider, ProcessingConfig, TextProcessor};
use hush::transcription::SimpleWhisperTranscriber;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;
use tracing::{error, info, warn, Level};
use tracing_subscriber;

enum AudioCommand {
    StartRecording,
    StopRecording,
}

enum TranscriptionResult {
    Success(String),
    Error(String),
}

fn main() {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("🎤 Hush Intelligent Editing Test (Audio + Transcription + AI Polishing)");
    info!("════════════════════════════════════════════════════════════════════");
    info!("Press Ctrl+Alt+V to start recording");
    info!("Release to transcribe, polish, and insert text");
    info!("════════════════════════════════════════════════════════════════════");

    // Create channels for communication
    let (audio_cmd_tx, audio_cmd_rx) = mpsc::channel::<AudioCommand>();
    let (transcription_tx, transcription_rx) = mpsc::channel::<TranscriptionResult>();

    // Create the hotkey manager
    let (hotkey_manager, hotkey_rx) = match HotkeyManager::new("Ctrl+Alt+V") {
        Ok(result) => result,
        Err(e) => {
            error!("Failed to create hotkey manager: {}", e);
            return;
        },
    };

    // Start listening for hotkeys
    if let Err(e) = hotkey_manager.start_listening() {
        error!("Failed to start hotkey listener: {}", e);
        return;
    }
    info!("✅ Hotkey 'Ctrl+Alt+V' registered");

    // Initialize audio capture (stays in main thread due to !Send trait)
    let mut audio_capture = match AudioCapture::new(None) {
        Ok(capture) => {
            info!(
                "✅ Audio capture initialized: {}",
                capture.get_device_name()
            );
            capture
        },
        Err(e) => {
            error!("Failed to initialize audio capture: {}", e);
            error!("Please ensure your audio device is working");
            return;
        },
    };

    // Initialize transcriber (async)
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let model_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".hush/models/ggml-base.en.bin");

    let transcriber = runtime.block_on(async {
        match SimpleWhisperTranscriber::new(&model_path).await {
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
        }
    });

    // Initialize text processor with LLM provider
    let processing_config = ProcessingConfig {
        mode: EditingMode::Medium,
        llm_provider: LlmProvider::None, // No LLM for this test
        max_tokens: 200,
        temperature: 0.3,
    };

    let text_processor = match TextProcessor::new(processing_config.clone()) {
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
            warn!("Will skip text processing step");
            None
        },
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

    // Create the overlay window
    let overlay = OverlayWindowBuilder::new()
        .width(320.0)
        .height(120.0)
        .position(OverlayPosition::BottomRight)
        .opacity(0.95)
        .show_button_when_idle(true)
        .build();

    // Get a handle to update the overlay state
    let state_handle = overlay.state();

    // Spawn thread to handle hotkey events
    let audio_cmd_tx_clone = audio_cmd_tx.clone();
    let state_handle_clone = state_handle.clone();
    let hotkey_thread = thread::spawn(move || {
        info!("🚀 Hotkey handler thread started");

        loop {
            match hotkey_rx.recv() {
                Ok(HotkeyEvent::Pressed) => {
                    info!("════════════════════════════════════════════════════════");
                    info!("🔴 HOTKEY PRESSED - Starting recording");
                    info!("════════════════════════════════════════════════════════");

                    // Update overlay to recording state
                    *state_handle_clone.lock().unwrap() = OverlayState::start_recording();

                    // Send command to start audio recording
                    if audio_cmd_tx_clone
                        .send(AudioCommand::StartRecording)
                        .is_err()
                    {
                        error!("Failed to send start recording command");
                        break;
                    }
                },
                Ok(HotkeyEvent::Released) => {
                    info!("════════════════════════════════════════════════════════");
                    info!("⏹️  HOTKEY RELEASED - Stopping recording");
                    info!("════════════════════════════════════════════════════════");

                    // Update overlay to processing state
                    *state_handle_clone.lock().unwrap() =
                        OverlayState::processing("Transcribing audio...");

                    // Send command to stop audio recording
                    if audio_cmd_tx_clone
                        .send(AudioCommand::StopRecording)
                        .is_err()
                    {
                        error!("Failed to send stop recording command");
                        break;
                    }
                },
                Err(e) => {
                    error!("Hotkey receiver error: {}", e);
                    break;
                },
            }
        }

        info!("Hotkey handler thread terminated");
    });

    // Spawn thread to handle transcription results and text insertion
    let state_handle_clone2 = state_handle.clone();
    let text_inserter_clone = text_inserter.clone();
    let text_processor_clone = text_processor.clone();
    let result_thread = thread::spawn(move || {
        info!("🚀 Result handler thread started");

        // Create a runtime for this thread
        let result_runtime = tokio::runtime::Runtime::new().expect("Failed to create runtime");

        while let Ok(result) = transcription_rx.recv() {
            match result {
                TranscriptionResult::Success(raw_text) => {
                    info!("✅ Transcription successful: '{}'", raw_text);

                    // Process the text
                    let processed_text = if let Some(ref processor) = text_processor_clone {
                        info!("🔄 Processing text...");
                        *state_handle_clone2.lock().unwrap() =
                            OverlayState::editing("Polishing text");

                        match result_runtime.block_on(processor.process(&raw_text)) {
                            Ok(polished) => {
                                info!("✨ Text polished: '{}'", polished);
                                polished
                            },
                            Err(e) => {
                                warn!("Text processing failed: {}, using raw text", e);
                                raw_text
                            },
                        }
                    } else {
                        info!("ℹ️  Skipping text processing (not available)");
                        raw_text
                    };

                    // Insert text if inserter is available
                    if let Some(ref inserter) = text_inserter_clone {
                        info!("⌨️  Inserting text into focused window...");

                        // Small delay to ensure window focus is stable
                        thread::sleep(Duration::from_millis(200));

                        if let Err(e) = inserter.lock().unwrap().insert_text(&processed_text) {
                            error!("Failed to insert text: {}", e);
                            warn!("Text will only be shown in overlay");
                        } else {
                            info!("✅ Text inserted successfully!");
                        }
                    }

                    // Update overlay to success state
                    let display_text = if processed_text.len() > 50 {
                        format!("{}...", &processed_text[..47])
                    } else {
                        processed_text
                    };

                    *state_handle_clone2.lock().unwrap() =
                        OverlayState::success(&display_text, Duration::from_secs(4));

                    info!("════════════════════════════════════════════════════════");
                    info!("✅ WORKFLOW COMPLETE");
                    info!("════════════════════════════════════════════════════════");
                },
                TranscriptionResult::Error(error_msg) => {
                    error!("❌ Transcription failed: {}", error_msg);

                    *state_handle_clone2.lock().unwrap() =
                        OverlayState::error(&error_msg, Duration::from_secs(4));
                },
            }
        }

        info!("Result handler thread terminated");
    });

    // Spawn overlay in a separate thread so we can handle audio in main thread
    let overlay_thread = thread::spawn(move || {
        info!("Starting overlay window...");
        if let Err(e) = overlay.run() {
            error!("Error running overlay: {}", e);
        }
        info!("Overlay closed");
    });

    // Main thread handles audio capture and transcription (since AudioCapture is !Send)
    info!("🚀 Audio handler (main thread) ready");

    while let Ok(command) = audio_cmd_rx.recv() {
        match command {
            AudioCommand::StartRecording => {
                info!("🎤 Starting audio recording...");

                if let Err(e) = audio_capture.start_recording() {
                    error!("Failed to start recording: {}", e);
                    if transcription_tx
                        .send(TranscriptionResult::Error(
                            "Failed to start recording".to_string(),
                        ))
                        .is_err()
                    {
                        error!("Failed to send error result");
                        break;
                    }
                } else {
                    info!("✅ Recording started - speak now!");
                }
            },

            AudioCommand::StopRecording => {
                info!("🛑 Stopping audio recording...");

                // Stop recording and get audio data
                let audio_data = match audio_capture.stop_recording() {
                    Ok(data) => {
                        info!("✅ Recording stopped - {} samples captured", data.len());
                        data
                    },
                    Err(e) => {
                        error!("Failed to stop recording: {}", e);
                        if transcription_tx
                            .send(TranscriptionResult::Error(
                                "Failed to stop recording".to_string(),
                            ))
                            .is_err()
                        {
                            error!("Failed to send error result");
                            break;
                        }
                        continue;
                    },
                };

                // Check if we have audio data
                if audio_data.is_empty() {
                    warn!("No audio data recorded");
                    if transcription_tx
                        .send(TranscriptionResult::Error("No audio recorded".to_string()))
                        .is_err()
                    {
                        error!("Failed to send error result");
                        break;
                    }
                    continue;
                }

                let duration_sec = audio_data.len() as f32 / 16000.0;
                info!("📊 Audio duration: {:.2}s", duration_sec);

                // Transcribe the audio
                let transcription_result = if let Some(ref transcriber) = transcriber {
                    if transcriber.is_ready() {
                        info!("🎤 Transcribing with Whisper model (GPU-accelerated)...");
                        match runtime.block_on(transcriber.transcribe(&audio_data)) {
                            Ok(result) => {
                                info!("✅ Transcription complete: '{}'", result.text);
                                info!("   Confidence: {:.2}", result.confidence);
                                info!(
                                    "   Processing time: {:.2}s",
                                    result.processing_time.as_secs_f32()
                                );
                                TranscriptionResult::Success(result.text)
                            },
                            Err(e) => {
                                error!("Transcription failed: {}", e);
                                TranscriptionResult::Error("Transcription failed".to_string())
                            },
                        }
                    } else {
                        // Fallback to simulated transcription with filler words for testing
                        warn!("Using simulated transcription (model not loaded)");
                        thread::sleep(Duration::from_millis(500)); // Simulate processing
                        TranscriptionResult::Success(
                            format!("um so like I think uh this is a test you know with filler words basically")
                        )
                    }
                } else {
                    // No transcriber available - use simulation with fillers
                    warn!("No transcriber - using simulation with filler words");
                    thread::sleep(Duration::from_millis(500)); // Simulate processing
                    TranscriptionResult::Success(format!(
                        "um well uh I mean this is like a simulated text you know"
                    ))
                };

                // Send transcription result
                if transcription_tx.send(transcription_result).is_err() {
                    error!("Failed to send transcription result");
                    break;
                }
            },
        }
    }

    // Clean up
    info!("Audio handler shutting down...");
    drop(hotkey_manager); // Unregister hotkey
    drop(audio_cmd_tx); // Close channel to stop threads

    // Wait for threads
    if let Err(e) = hotkey_thread.join() {
        error!("Failed to join hotkey thread: {:?}", e);
    }
    if let Err(e) = result_thread.join() {
        error!("Failed to join result thread: {:?}", e);
    }
    if let Err(e) = overlay_thread.join() {
        error!("Failed to join overlay thread: {:?}", e);
    }

    info!("Test complete");
}
