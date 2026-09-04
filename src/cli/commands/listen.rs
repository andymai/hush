use anyhow::Result;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use tracing::{error, info, warn};

// Import Hush components
use crate::audio::AudioFeedback;
use crate::cli::commands::daemon::{self, SessionOptions};
use crate::config::Config;
use crate::hotkey::{
    GestureAction, GestureDetector, HotkeyAction, HotkeyBindings, HotkeyEvent, HotkeyManager,
    HotkeyMode,
};
use crate::ipc::protocol::DaemonState;
use crate::ipc::SessionCommand;
use crate::notify::notify;
use crate::overlay::{OverlayState, OverlayWindowBuilder};
use crate::text::WindowInfo;
use crate::text_processing::{learn, llm, prompt, ProcessingContext};
use crate::text_processing::{
    CommandExecutor, CommandParser, EditingMode, InsertionHistory, LlmProvider, ProcessingConfig,
    TextProcessor,
};
use crate::transcription::models::{ModelManager, ModelSize};
use crate::transcription::{WhisperTranscriber, WHISPER_SAMPLE_RATE};
use crate::AudioCapture;

#[cfg(target_os = "linux")]
use crate::TextInserter;

/// What a Command Mode instruction applies to, captured when the chord went down.
struct CommandTarget {
    selection: Option<String>,
    last_text: Option<String>,
}

/// Run the dictation session in the foreground: the daemon body.
///
/// Claims the single-daemon slot (PID file and control socket), then serves
/// hotkey and IPC commands until `hush daemon stop`, Ctrl+C, or SIGTERM.
///
/// # Arguments
///
/// * `options.editing_mode` - Text editing aggressiveness: "light", "medium", or "aggressive"
/// * `options.no_processing` - If true, skip LLM-based text processing (rule-based only)
/// * `options.no_button` - If true, hide the idle overlay button
///
/// # Workflow
///
/// 1. **Initialization**
///    - Parse editing mode (light/medium/aggressive)
///    - Initialize hotkey manager (Ctrl+Shift+Space)
///    - Setup audio capture with amplitude monitoring
///    - Load Whisper transcriber model
///    - Initialize text processor with LLM (if enabled)
///    - Initialize text inserter
///    - Setup voice command system (parser, executor, history)
///    - Create overlay window with tiny design
///
/// 2. **Multi-threaded Execution**
///    - **Hotkey Thread**: Listens for Ctrl+Shift+Space press/release
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
pub async fn handle_listen(options: SessionOptions) -> Result<()> {
    let SessionOptions {
        editing_mode: editing_mode_str,
        no_processing,
        no_button,
    } = options;
    info!("🎧 Starting intelligent listening mode");
    info!("   Editing mode: {}", editing_mode_str);
    info!("   Text processing: {}", !no_processing);
    info!("   Show button: {}", !no_button);

    // Load configuration
    let config = match Config::load() {
        Ok(config) => config,
        Err(e) => {
            warn!("Failed to load config, using defaults: {}", e);
            Config::defaults()?
        },
    };
    let hotkey_combination = config.hotkey.combination.clone();
    let hotkey_mode = config.hotkey.mode;

    println!("🎤 Hush Intelligent Listening Mode");
    println!("═══════════════════════════════════════════════════════");
    match hotkey_mode {
        HotkeyMode::Hold => {
            println!("Hold {} to dictate, release to insert", hotkey_combination);
            println!("Double-tap it to keep recording hands-free, press again to stop");
        },
        HotkeyMode::Toggle => {
            println!(
                "Tap {} to start recording, press again to stop and insert",
                hotkey_combination
            );
            println!("Holding it records only while held");
        },
    }
    if !config.hotkey.cancel.trim().is_empty() {
        println!("{} discards a recording", config.hotkey.cancel.trim());
    }
    println!("`hush toggle` starts and stops from a keybind; Ctrl+C or `hush daemon stop` quits");
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
    enum TranscriptionResult {
        Success {
            text: String,
            window: Option<WindowInfo>,
            command: Option<CommandTarget>,
        },
        Error(String),
    }

    let (audio_cmd_tx, audio_cmd_rx) = mpsc::channel::<SessionCommand>();
    let (transcription_tx, transcription_rx) = mpsc::channel::<TranscriptionResult>();

    // Single instance and control socket, before any device is opened
    let handles = daemon::claim(audio_cmd_tx.clone()).await?;
    let shared = Arc::clone(&handles.shared);

    {
        let quit_tx = audio_cmd_tx.clone();
        tokio::spawn(async move {
            let mut sigterm =
                match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                    Ok(signal) => signal,
                    Err(e) => {
                        warn!("SIGTERM handler unavailable: {}", e);
                        return;
                    },
                };
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {},
                _ = sigterm.recv() => {},
            }
            info!("Shutdown signal received");
            let _ = quit_tx.send(SessionCommand::Quit);
        });
    }

    let bindings = HotkeyBindings::parse(&hotkey_combination, Some(&config.hotkey.cancel))?
        .with_action(HotkeyAction::PasteLast, &config.hotkey.paste_last)?
        .with_action(HotkeyAction::Learn, &config.hotkey.learn)?
        .with_command(&config.hotkey.command)?;
    let (hotkey_manager, hotkey_rx) =
        HotkeyManager::with_backend(bindings, config.hotkey.backend, config.hotkey.exclusive)?;
    let feedback = if config.feedback.audio_enabled {
        AudioFeedback::default()
    } else {
        AudioFeedback::silent()
    };
    hotkey_manager.start_listening()?;
    info!(
        "✅ Hotkey '{}' registered via {}",
        hotkey_combination,
        hotkey_manager.backend_name()
    );

    // Initialize audio capture (main thread - !Send)
    let mut audio_capture = AudioCapture::new(config.audio.device.as_deref())?;
    // Note: amplitude monitoring is now enabled per-recording session to avoid thread accumulation
    info!(
        "✅ Audio capture initialized: {}",
        audio_capture.get_device_name()
    );

    // Initialize transcriber - try config first, then fall back to ModelManager
    let model_path = {
        let configured = config.transcription.model_path();
        if configured.exists() {
            configured
        } else {
            let model_size: ModelSize = config
                .transcription
                .model_size
                .parse()
                .unwrap_or(ModelSize::Base);
            ModelManager::new(crate::config::paths::models_dir())
                .ok()
                .and_then(|manager| manager.get_model_path(&model_size))
                .unwrap_or(configured)
        }
    };

    info!("Using model path: {:?}", model_path);

    let transcriber = WhisperTranscriber::new(&model_path, config.transcription.use_gpu)
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "{}\nDownload a model with: hush models download {}",
                e,
                config.transcription.model_size
            )
        })?
        .with_language(&config.transcription.language);
    info!(
        "✅ Whisper transcriber ready ({})",
        transcriber.get_device_info()
    );

    // API keys: ~/.config/hush/.env for installed daemons, ./.env for source checkouts
    let _ = dotenvy::from_path(crate::config::paths::config_dir().join(".env"));
    let _ = dotenvy::dotenv();

    // Initialize text processor
    let text_processor = if !no_processing {
        let llm_provider = llm::resolve_provider(&config.llm).await;
        if matches!(llm_provider, LlmProvider::None) {
            info!("No LLM: polishing is rule-based and Command Mode is off until Ollama runs or ANTHROPIC_API_KEY is set");
        }

        let processing_config = ProcessingConfig {
            mode: editing_mode,
            llm_provider,
            polish: config.llm.polish,
            max_tokens: config.llm.max_tokens,
            temperature: config.llm.temperature,
        };

        match TextProcessor::new(processing_config) {
            Ok(processor) => {
                match processor.llm_name() {
                    Some(name) => info!("✅ Text processor initialized with {}", name),
                    None => info!("✅ Text processor initialized (rule-based only)"),
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
    #[cfg(target_os = "linux")]
    let text_inserter = match TextInserter::new() {
        Ok(mut inserter) => {
            inserter.set_method(config.insertion.method);
            inserter.set_typing_delay(config.insertion.typing_delay_ms);
            info!("✅ Text inserter initialized");
            Some(Arc::new(Mutex::new(inserter)))
        },
        Err(e) => {
            warn!("Failed to initialize text inserter: {}", e);
            warn!("Transcribed text will only be shown in overlay");
            None
        },
    };

    #[cfg(not(target_os = "linux"))]
    let text_inserter: Option<std::sync::Arc<parking_lot::Mutex<()>>> = {
        warn!("TextInserter not available on this platform (use trait-based adapters instead)");
        None
    };

    // Initialize voice command components
    let command_parser = Arc::new(CommandParser::new());
    let command_executor = Arc::new(CommandExecutor::new());
    let insertion_history = Arc::new(Mutex::new(InsertionHistory::new()));
    info!("✅ Voice command system initialized");

    // Create overlay
    let overlay = OverlayWindowBuilder::new()
        .show_button_when_idle(false)
        .hotkey(&hotkey_combination)
        .build();

    let state_handle = overlay.state();
    let egui_context_handle = overlay.egui_context();

    // Hotkey handler thread: raw presses become gestures, gestures become commands
    let audio_cmd_tx_clone = audio_cmd_tx.clone();
    let state_handle_clone = state_handle.clone();
    let egui_context_clone = egui_context_handle.clone();
    let shared_hotkey = Arc::clone(&shared);
    let tap_window = std::time::Duration::from_millis(config.hotkey.tap_ms);
    let hotkey_thread = thread::spawn(move || {
        let mut gesture = GestureDetector::new(hotkey_mode, tap_window);
        loop {
            let timeout = gesture
                .deadline()
                .map(|deadline| deadline.saturating_duration_since(std::time::Instant::now()))
                .unwrap_or(std::time::Duration::from_secs(3600));
            let event = match hotkey_rx.recv_timeout(timeout) {
                Ok(event) => Some(event),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            };
            let now = std::time::Instant::now();
            if let Some(HotkeyEvent::Command(pressed)) = event {
                let command = if pressed {
                    let mut state = OverlayState::start_recording();
                    state.set_warning(Some("cmd".to_string()));
                    *state_handle_clone.lock() = state;
                    if let Some(ctx) = egui_context_clone.lock().as_ref() {
                        ctx.request_repaint();
                    }
                    SessionCommand::StartCommand
                } else {
                    SessionCommand::StopRecording
                };
                if audio_cmd_tx_clone.send(command).is_err() {
                    break;
                }
                continue;
            }
            if let Some(HotkeyEvent::Action(action)) = event {
                let command = match action {
                    HotkeyAction::PasteLast => SessionCommand::PasteLast,
                    HotkeyAction::Learn => SessionCommand::Learn,
                };
                if audio_cmd_tx_clone.send(command).is_err() {
                    break;
                }
                continue;
            }
            if !matches!(shared_hotkey.snapshot(), DaemonState::Recording { .. }) {
                gesture.session_ended();
            }
            let action = match event {
                Some(event) => gesture.feed(event, now),
                None => {
                    gesture.expire(now);
                    None
                },
            };
            let command = match action {
                Some(GestureAction::Start) => {
                    let mut state = OverlayState::start_recording();
                    state.set_locked(gesture.is_locked());
                    *state_handle_clone.lock() = state;
                    if let Some(ctx) = egui_context_clone.lock().as_ref() {
                        ctx.request_repaint();
                    }
                    Some(SessionCommand::StartRecording)
                },
                // The overlay stays in Recording until transcription completes
                Some(GestureAction::Stop) => Some(SessionCommand::StopRecording),
                Some(GestureAction::Cancel) => Some(SessionCommand::CancelRecording),
                None => None,
            };
            if gesture.is_locked() {
                state_handle_clone.lock().set_locked(true);
            }
            if let Some(command) = command {
                if audio_cmd_tx_clone.send(command).is_err() {
                    warn!("Channel send failed - receiver dropped");
                    break;
                }
            }
        }
    });

    // Result handler thread
    let state_handle_clone2 = state_handle.clone();
    let text_inserter_clone = text_inserter.clone();
    let text_processor_clone = text_processor.clone();
    let profiles = config.profiles.clone();
    let command_parser_clone = command_parser.clone();
    let command_executor_clone = command_executor.clone();
    let insertion_history_clone = insertion_history.clone();
    let shared_result = Arc::clone(&shared);
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
                TranscriptionResult::Success {
                    text: raw_text,
                    window,
                    command,
                } => {
                    shared_result.set_inserting();
                    if let Some(target) = command {
                        run_command_mode(
                            &raw_text,
                            target,
                            text_processor_clone.as_deref(),
                            &result_runtime,
                            text_inserter_clone.as_ref(),
                            &insertion_history_clone,
                            &shared_result,
                            feedback,
                        );
                        *state_handle_clone2.lock() = OverlayState::idle();
                        shared_result.set_idle();
                        continue;
                    }
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
                                let preview = if last_entry.text.chars().count() > 50 {
                                    last_entry.text.chars().take(50).collect::<String>()
                                } else {
                                    last_entry.text.clone()
                                };
                                info!(
                                    "Undoing last insertion: '{}' ({} chars)",
                                    preview, last_entry.char_count
                                );

                                #[cfg(target_os = "linux")]
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

                                #[cfg(not(target_os = "linux"))]
                                {
                                    warn!("Undo not supported on this platform");
                                    *state_handle_clone2.lock() = OverlayState::idle();
                                }
                            } else {
                                warn!("No recent insertion to undo");
                                *state_handle_clone2.lock() = OverlayState::error(
                                    "Nothing to undo",
                                    std::time::Duration::from_secs(2),
                                );
                            }
                        }
                        shared_result.set_idle();
                        continue;
                    }

                    // If no text to insert, skip
                    let command_text = match exec_result.text {
                        Some(text) => text,
                        None => {
                            shared_result.set_idle();
                            continue;
                        },
                    };

                    // Process text (if enabled and should_process is true)
                    let processed_text = if exec_result.should_process {
                        if let Some(ref processor) = text_processor_clone {
                            info!("🔄 Processing text...");
                            // Keep current state (likely Recording) while processing

                            let context = ProcessingContext::new(window.clone(), &profiles);
                            match result_runtime
                                .block_on(processor.process_for(&command_text, &context))
                            {
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

                    shared_result.set_last_text(processed_text.clone());

                    // Insert text
                    #[cfg(target_os = "linux")]
                    if let Some(ref inserter) = text_inserter_clone {
                        thread::sleep(std::time::Duration::from_millis(200));

                        let mut inserter = inserter.lock();
                        match inserter.insert_text(&processed_text) {
                            Ok(_) => {
                                // Record in history for undo
                                insertion_history_clone
                                    .lock()
                                    .record(processed_text.clone());
                            },
                            Err(e) => {
                                error!("Text insertion failed: {}", e);
                                let _ = feedback.play_error();
                                let body = match inserter.copy_to_clipboard(&processed_text) {
                                    Ok(()) => "The text is on the clipboard. `hush paste-last` types it again.",
                                    Err(_) => "`hush paste-last` types it again.",
                                };
                                notify("Hush could not insert the text", body);
                            },
                        }
                    }

                    #[cfg(not(target_os = "linux"))]
                    {
                        info!("Text insertion not available on this platform");
                        info!("Processed text: {}", processed_text);
                    }

                    // Update overlay
                    let _display_text = if processed_text.chars().count() > 50 {
                        let truncated: String = processed_text.chars().take(47).collect();
                        format!("{}...", truncated)
                    } else {
                        processed_text
                    };

                    *state_handle_clone2.lock() = OverlayState::idle();
                    shared_result.set_idle();
                },
                TranscriptionResult::Error(error_msg) => {
                    error!("❌ Transcription failed: {}", error_msg);
                    let _ = feedback.play_error();
                    *state_handle_clone2.lock() =
                        OverlayState::error(&error_msg, std::time::Duration::from_secs(4));
                    shared_result.set_idle();
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

    // Track current amplitude monitoring thread (only one at a time)
    let mut current_amplitude_thread: Option<thread::JoinHandle<()>> = None;
    // Stop signal for amplitude thread - shared between main loop and amplitude thread
    let amplitude_stop_signal = Arc::new(AtomicBool::new(false));

    let max_secs = config.audio.max_recording_secs;
    let warn_secs = if max_secs > 90 {
        max_secs - 60
    } else {
        max_secs * 2 / 3
    };
    let mut cap_warned = false;
    let mut cap_shown: Option<u64> = None;
    let context_prompt = config.transcription.context_prompt;
    let mut current_window: Option<WindowInfo> = None;
    let mut command_target: Option<CommandTarget> = None;

    loop {
        // Use short timeout for responsive hotkey handling (16ms ≈ 60Hz)
        let command = match audio_cmd_rx.recv_timeout(std::time::Duration::from_millis(16)) {
            Ok(cmd) => cmd,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if max_secs == 0 || !audio_capture.is_recording() {
                    continue;
                }
                let DaemonState::Recording { elapsed_ms } = shared.snapshot() else {
                    continue;
                };
                let elapsed = elapsed_ms / 1000;
                if elapsed >= max_secs {
                    info!("Recording reached the {} second cap", max_secs);
                    notify(
                        "Recording stopped",
                        &format!(
                            "Hush stops after {} minutes and inserts what it heard.",
                            max_secs / 60
                        ),
                    );
                    if audio_cmd_tx.send(SessionCommand::StopRecording).is_err() {
                        break;
                    }
                    continue;
                }
                if elapsed >= warn_secs {
                    if !cap_warned {
                        cap_warned = true;
                        let _ = feedback.play_warning();
                        notify(
                            "Recording stops soon",
                            &format!("Hush stops recording in {} seconds.", max_secs - elapsed),
                        );
                    }
                    let remaining = max_secs - elapsed;
                    if cap_shown != Some(remaining) {
                        cap_shown = Some(remaining);
                        state_handle.lock().set_warning(Some(format!(
                            "{}:{:02}",
                            remaining / 60,
                            remaining % 60
                        )));
                    }
                }
                continue;
            },
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        };
        match command {
            SessionCommand::Quit => {
                info!("Quit requested");
                shared.set_stopping();
                break;
            },
            SessionCommand::PasteLast => {
                if audio_capture.is_recording() {
                    warn!("Ignoring paste-last while recording");
                    continue;
                }
                let Some(text) = shared.last_text() else {
                    notify("Nothing to paste yet", "Dictate something first.");
                    continue;
                };
                #[cfg(target_os = "linux")]
                if let Some(inserter) = text_inserter.as_ref() {
                    shared.set_inserting();
                    match inserter.lock().insert_text(&text) {
                        Ok(()) => insertion_history.lock().record(text.clone()),
                        Err(e) => {
                            error!("Paste-last failed: {}", e);
                            let _ = feedback.play_error();
                        },
                    }
                    shared.set_idle();
                }
            },
            SessionCommand::Learn => match learn::learn(None) {
                Ok(term) => {
                    if let Some(processor) = text_processor.as_ref() {
                        processor.reload_vocabulary();
                    }
                    let _ = feedback.play_success();
                    notify(
                        "Learned",
                        &format!("'{}' will be written exactly like that.", term),
                    );
                },
                Err(e) => {
                    let _ = feedback.play_error();
                    notify("Nothing learned", &e.to_string());
                },
            },
            SessionCommand::ReloadVocabulary => {
                if let Some(processor) = text_processor.as_ref() {
                    processor.reload_vocabulary();
                }
            },
            SessionCommand::CancelRecording => {
                if !audio_capture.is_recording() {
                    continue;
                }
                amplitude_stop_signal.store(true, Ordering::Release);
                audio_capture.disable_amplitude_monitoring();
                if let Some(thread) = current_amplitude_thread.take() {
                    let _ = thread.join();
                }
                if let Err(e) = audio_capture.stop_recording() {
                    error!("Failed to stop recording: {}", e);
                }
                info!("Recording cancelled");
                command_target = None;
                let _ = feedback.play_cancel();
                *state_handle.lock() = OverlayState::idle();
                shared.set_idle();
            },
            SessionCommand::StartRecording | SessionCommand::StartCommand => {
                if audio_capture.is_recording() {
                    continue;
                }
                let is_command = command == SessionCommand::StartCommand;
                // Reset stop signal for new recording
                amplitude_stop_signal.store(false, Ordering::Release);

                // Create fresh amplitude channel for this recording session
                let amplitude_rx = audio_capture.enable_amplitude_monitoring();

                if let Err(e) = audio_capture.start_recording() {
                    error!("Failed to start recording: {}", e);
                    audio_capture.disable_amplitude_monitoring();
                    if transcription_tx
                        .send(TranscriptionResult::Error(
                            "Failed to start recording".to_string(),
                        ))
                        .is_err()
                    {
                        warn!("Channel send failed - receiver dropped");
                    }
                } else {
                    shared.set_recording();
                    #[cfg(target_os = "linux")]
                    {
                        current_window = text_inserter
                            .as_ref()
                            .and_then(|inserter| inserter.lock().get_focused_window());
                    }
                    command_target = is_command.then(|| CommandTarget {
                        selection: read_selection(),
                        last_text: shared.last_text(),
                    });
                    cap_warned = false;
                    cap_shown = None;
                    let _ = feedback.play_start();
                    // Start polling amplitude updates with batching for performance
                    let state_handle_amp = state_handle.clone();
                    let stop_signal = amplitude_stop_signal.clone();

                    // Batch amplitude updates to match overlay repaint rate (20 FPS during recording)
                    // This reduces mutex contention and provides smoother averaged values
                    let amplitude_thread = std::thread::spawn(move || {
                        let mut amplitude_buffer = Vec::with_capacity(10);
                        let update_interval = std::time::Duration::from_millis(50); // 20 Hz, matches recording repaint
                        let mut last_update = std::time::Instant::now();

                        loop {
                            // Check stop signal first for clean shutdown
                            if stop_signal.load(Ordering::Acquire) {
                                break;
                            }

                            match amplitude_rx.try_recv() {
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
                                        }
                                        // Don't break here - let stop_signal control exit

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
                    current_amplitude_thread = Some(amplitude_thread);
                }
            },
            SessionCommand::StopRecording => {
                if !audio_capture.is_recording() {
                    continue;
                }
                shared.set_transcribing();
                let _ = feedback.play_stop();
                // Signal amplitude thread to stop and clean up immediately
                amplitude_stop_signal.store(true, Ordering::Release);
                audio_capture.disable_amplitude_monitoring();

                // Join the amplitude thread with timeout to avoid blocking
                if let Some(thread) = current_amplitude_thread.take() {
                    // Give the thread a moment to notice the stop signal
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    let _ = thread.join();
                }

                let audio_data = match audio_capture.stop_recording() {
                    Ok(data) => data,
                    Err(e) => {
                        error!("Failed to stop recording: {}", e);
                        shared.set_idle();
                        if transcription_tx
                            .send(TranscriptionResult::Error(
                                "Failed to stop recording".to_string(),
                            ))
                            .is_err()
                        {
                            warn!("Channel send failed - receiver dropped");
                        }
                        continue;
                    },
                };

                if audio_data.is_empty() {
                    shared.set_idle();
                    if transcription_tx
                        .send(TranscriptionResult::Error("No audio recorded".to_string()))
                        .is_err()
                    {
                        warn!("Channel send failed - receiver dropped");
                    }
                    continue;
                }

                // Silence detection: calculate RMS energy of audio
                let rms = (audio_data.iter().map(|s| s * s).sum::<f32>() / audio_data.len() as f32)
                    .sqrt();
                const SILENCE_THRESHOLD: f32 = 0.005;

                if rms < SILENCE_THRESHOLD {
                    *state_handle.lock() = OverlayState::idle();
                    shared.set_idle();
                    continue;
                }

                let window = current_window.take();
                let command = command_target.take();
                let initial_prompt = if context_prompt {
                    let terms = text_processor
                        .as_ref()
                        .map(|processor| processor.user_terms())
                        .unwrap_or_default();
                    prompt::whisper_prompt(window.as_ref(), &terms)
                } else {
                    None
                };

                // Transcribe
                let transcription_result = match transcriber
                    .transcribe_with_prompt(
                        &audio_data,
                        WHISPER_SAMPLE_RATE,
                        initial_prompt.as_deref(),
                    )
                    .await
                {
                    Ok(result) => {
                        // Filter common Whisper hallucinations on near-silence
                        let text = result.text.trim().to_lowercase();
                        let hallucinations = [
                            "you",
                            "thank you",
                            "thanks",
                            "thank you.",
                            "thanks for watching",
                            "bye",
                            "goodbye",
                            "thank you for watching",
                            "see you next time",
                            "subscribe",
                            "like and subscribe",
                        ];
                        if hallucinations.iter().any(|h| text == *h) {
                            *state_handle.lock() = OverlayState::idle();
                            shared.set_idle();
                            continue;
                        }
                        TranscriptionResult::Success {
                            text: result.text,
                            window,
                            command,
                        }
                    },
                    Err(e) => {
                        error!("Transcription failed: {}", e);
                        TranscriptionResult::Error("Transcription failed".to_string())
                    },
                };

                if transcription_tx.send(transcription_result).is_err() {
                    warn!("Channel send failed - receiver dropped");
                }
            },
        }
    }

    // Cleanup: release the socket and PID file first so a restart can claim them
    drop(handles);
    drop(hotkey_manager);
    drop(audio_cmd_tx);
    drop(transcription_tx);

    // Signal any remaining amplitude thread to stop
    amplitude_stop_signal.store(true, Ordering::Release);
    if let Some(thread) = current_amplitude_thread.take() {
        let _ = thread.join();
    }

    let _ = hotkey_thread.join();
    let _ = result_thread.join();
    // The overlay window only closes when the process exits, so it is not joined.
    drop(overlay_thread);

    info!("Listen mode ended");
    Ok(())
}

#[cfg(target_os = "linux")]
fn read_selection() -> Option<String> {
    crate::text::selection::read_primary()
}

#[cfg(not(target_os = "linux"))]
fn read_selection() -> Option<String> {
    None
}

/// Apply a spoken instruction to the selection, or to the last transcript
/// when nothing is selected, and replace it in place.
#[allow(clippy::too_many_arguments)]
fn run_command_mode(
    instruction: &str,
    target: CommandTarget,
    processor: Option<&TextProcessor>,
    runtime: &tokio::runtime::Runtime,
    inserter: Option<&Arc<Mutex<TextInserter>>>,
    history: &Arc<Mutex<InsertionHistory>>,
    shared: &crate::ipc::SharedState,
    feedback: AudioFeedback,
) {
    let instruction = instruction.trim();
    if instruction.is_empty() {
        return;
    }
    let Some(processor) = processor.filter(|p| p.is_llm_ready()) else {
        let _ = feedback.play_error();
        notify(
            "Command Mode needs an LLM",
            "Run Ollama, or set ANTHROPIC_API_KEY in ~/.config/hush/.env, then restart the daemon.",
        );
        return;
    };
    let (text, replaces_last) = match (target.selection, target.last_text) {
        (Some(selection), _) => (selection, false),
        (None, Some(last)) => (last, true),
        (None, None) => {
            let _ = feedback.play_error();
            notify(
                "Nothing to edit",
                "Select some text or dictate something first.",
            );
            return;
        },
    };
    info!(
        "Command Mode: '{}' on {} chars",
        instruction,
        text.chars().count()
    );
    let rewritten = match runtime.block_on(processor.rewrite(instruction, &text)) {
        Ok(rewritten) => rewritten,
        Err(e) => {
            error!("Command Mode failed: {}", e);
            let _ = feedback.play_error();
            notify("Command failed", &e.to_string());
            return;
        },
    };
    if rewritten.trim().is_empty() || rewritten == text {
        notify(
            "Nothing changed",
            "The instruction left the text as it was.",
        );
        return;
    }
    let Some(inserter) = inserter else {
        info!("Rewritten text: {}", rewritten);
        return;
    };
    let mut inserter = inserter.lock();
    if replaces_last {
        let chars = history
            .lock()
            .pop_last()
            .map(|entry| entry.char_count)
            .unwrap_or_else(|| text.chars().count());
        if let Err(e) = inserter.undo_last_insertion(chars) {
            error!("Could not remove the last transcript: {}", e);
        }
    }
    match inserter.insert_text(&rewritten) {
        Ok(()) => {
            history.lock().record(rewritten.clone());
            shared.set_last_text(rewritten);
            let _ = feedback.play_success();
        },
        Err(e) => {
            error!("Command Mode insertion failed: {}", e);
            let _ = feedback.play_error();
            let _ = inserter.copy_to_clipboard(&rewritten);
            notify(
                "Hush could not insert the text",
                "The result is on the clipboard.",
            );
        },
    }
}
