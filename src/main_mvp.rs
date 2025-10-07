use hush::{Config, Result};
use hush::{AudioCapture, WhisperTranscriber, TextInserter};
use hush::hotkey::{HotkeyManager, HotkeyEvent};
use std::time::Instant;
use tracing::{info, warn, error, debug};
use tokio::time::{sleep, Duration};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use clap::{Parser, Subcommand};
use notify_rust::Notification;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "hush")]
#[command(about = "🤫 Hush - Voice-to-Text for Linux Developers")]
#[command(version = "0.1.0")]
#[command(author = "Andy")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    
    /// Disable desktop notifications
    #[arg(long)]
    no_notifications: bool,
    
    /// Custom configuration file path
    #[arg(short, long)]
    config: Option<PathBuf>,
    
    /// Verbose logging
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(Subcommand)]
enum Commands {
    /// Run in daemon mode with global hotkey support (default)
    Daemon {
        /// Try to run with elevated privileges for hotkey access
        #[arg(long)]
        elevated: bool,
    },
    /// Run in one-shot mode (record once and exit)
    OneShot {
        /// Maximum recording duration in seconds
        #[arg(short, long, default_value = "30")]
        duration: u64,
        /// Don't insert text, just print to stdout
        #[arg(long)]
        print_only: bool,
    },
    /// Manual recording mode (no hotkeys)
    Manual,
    /// Show application status and configuration
    Status,
    /// Install desktop integration
    Install,
    /// Remove desktop integration
    Uninstall,
}

pub struct HushApp {
    config: Config,
    audio_capture: AudioCapture,
    transcriber: WhisperTranscriber,
    text_inserter: TextInserter,
    hotkey_manager: Option<HotkeyManager>,
    hotkey_receiver: Option<std::sync::mpsc::Receiver<HotkeyEvent>>,
    is_recording: Arc<AtomicBool>,
    recording_start_time: Option<Instant>,
    notifications_enabled: bool,
    mode: AppMode,
}

#[derive(Debug, Clone)]
pub enum AppMode {
    Daemon { elevated: bool },
    OneShot { duration: u64, print_only: bool },
    Manual,
    Status,
}

impl HushApp {
    pub async fn new(mode: AppMode, notifications_enabled: bool, config_path: Option<PathBuf>) -> Result<Self> {
        info!("🤫 Initializing Hush Voice-to-Text Application (mode: {:?})", mode);

        // Load configuration
        info!("Loading configuration...");
        let config = if let Some(path) = config_path {
            Config::load_from_file(&path)?
        } else {
            Config::load()?
        };
        info!("✅ Configuration loaded successfully");

        // Initialize audio capture
        info!("Initializing audio capture system...");
        let audio_capture = AudioCapture::new(config.audio.device.as_deref())?;
        info!("✅ Audio capture initialized (device: {})", audio_capture.get_device_name());

        // Initialize Whisper transcriber
        info!("Initializing Whisper transcriber...");
        let transcriber = WhisperTranscriber::new(&config.transcription.model_path, config.transcription.use_cuda).await?;
        info!("✅ Whisper transcriber initialized (device: {}, CUDA: {})", 
               transcriber.get_device_info(), transcriber.is_using_cuda());

        // Initialize text insertion system (only if needed)
        let text_inserter = match &mode {
            AppMode::OneShot { print_only: true, .. } => {
                info!("⏭️  Skipping text insertion (print-only mode)");
                TextInserter::new()? // Still create for consistency, but won't use
            }
            _ => {
                info!("Initializing text insertion system...");
                let inserter = TextInserter::new()?;
                info!("✅ Text insertion system initialized");
                inserter
            }
        };

        // Initialize hotkey management (only for daemon mode)
        let (hotkey_manager, hotkey_receiver) = match &mode {
            AppMode::Daemon { .. } => {
                info!("Initializing hotkey system (combination: '{}')...", config.hotkey.combination);
                match HotkeyManager::new(&config.hotkey.combination) {
                    Ok((manager, receiver)) => {
                        info!("✅ Hotkey system initialized");
                        (Some(manager), Some(receiver))
                    }
                    Err(e) => {
                        warn!("⚠️  Hotkey system failed to initialize: {:?}", e);
                        warn!("Running without global hotkeys - use 'hush manual' mode instead");
                        if notifications_enabled {
                            Self::show_notification(
                                "Hush Warning", 
                                "Global hotkeys unavailable. Use manual mode or check permissions.", 
                                notify_rust::Urgency::Normal
                            );
                        }
                        (None, None)
                    }
                }
            }
            _ => {
                info!("⏭️  Skipping hotkey initialization for this mode");
                (None, None)
            }
        };

        Ok(HushApp {
            config,
            audio_capture,
            transcriber,
            text_inserter,
            hotkey_manager,
            hotkey_receiver,
            is_recording: Arc::new(AtomicBool::new(false)),
            recording_start_time: None,
            notifications_enabled,
            mode,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        match &self.mode {
            AppMode::Daemon { .. } => self.run_daemon_mode().await,
            AppMode::OneShot { duration, print_only } => self.run_oneshot_mode(*duration, *print_only).await,
            AppMode::Manual => self.run_manual_mode().await,
            AppMode::Status => self.show_status().await,
        }
    }

    async fn run_daemon_mode(&mut self) -> Result<()> {
        info!("🚀 Starting Hush in daemon mode");
        
        // Check if hotkey system is available
        if self.hotkey_manager.is_none() || self.hotkey_receiver.is_none() {
            error!("Daemon mode requires functional hotkey system. Use 'hush manual' instead.");
            return Err(anyhow::anyhow!("Hotkey system not available for daemon mode"));
        }
        
        info!("📝 Instructions:");
        info!("   • Press {} to start recording", self.config.hotkey.combination);
        info!("   • Release {} to stop recording and transcribe", self.config.hotkey.combination);
        info!("   • Text will be automatically inserted at cursor position");
        info!("   • Press Ctrl+C to quit");
        
        // Start hotkey listener
        if let Some(hotkey_manager) = &self.hotkey_manager {
            hotkey_manager.start_listening()?;
            info!("🎯 Hotkey listener active - waiting for input...");
        }

        if self.notifications_enabled {
            Self::show_notification("Hush Started", "Voice-to-text is now active. Press Ctrl+Shift+Space to record.", notify_rust::Urgency::Normal);
        }

        // Main event loop
        loop {
            // Check for hotkey events
            if let Some(hotkey_receiver) = &self.hotkey_receiver {
                match hotkey_receiver.try_recv() {
                    Ok(HotkeyEvent::Pressed) => {
                        if let Err(e) = self.handle_hotkey_pressed().await {
                            error!("Error handling hotkey press: {:?}", e);
                        }
                    }
                    Ok(HotkeyEvent::Released) => {
                        if let Err(e) = self.handle_hotkey_released().await {
                            error!("Error handling hotkey release: {:?}", e);
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        // No hotkey events, continue
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        warn!("Hotkey receiver disconnected, shutting down...");
                        break;
                    }
                }
            }

            // Small sleep to prevent busy waiting
            sleep(Duration::from_millis(10)).await;
        }

        info!("👋 Hush daemon shutting down");
        Ok(())
    }

    async fn run_oneshot_mode(&mut self, duration: u64, print_only: bool) -> Result<()> {
        info!("🎤 Running in one-shot mode (max {}s)", duration);
        
        if self.notifications_enabled {
            Self::show_notification("Hush Recording", "Recording started. Speak now...", notify_rust::Urgency::Normal);
        }

        // Start recording immediately
        info!("🎤 Starting recording...");
        self.audio_capture.start_recording()?;
        self.is_recording.store(true, Ordering::Relaxed);
        self.recording_start_time = Some(Instant::now());

        // Record for specified duration
        sleep(Duration::from_secs(duration)).await;

        // Stop and process
        let audio_data = self.audio_capture.stop_recording()?;
        self.is_recording.store(false, Ordering::Relaxed);

        if !audio_data.is_empty() {
            info!("🔄 Transcribing audio...");
            let transcription_result = self.transcriber.transcribe_async(&audio_data, self.config.audio.sample_rate).await?;
            let text = transcription_result.text.trim();

            if !text.is_empty() {
                if print_only {
                    println!("{}", text);
                } else {
                    info!("📝 Transcribed: '{}'", text);
                    self.text_inserter.insert_text(text)?;
                    info!("✅ Text inserted successfully");
                }

                if self.notifications_enabled {
                    Self::show_notification("Hush Complete", &format!("Transcribed: {}", text), notify_rust::Urgency::Normal);
                }
            } else {
                info!("🤐 No speech detected");
                if self.notifications_enabled {
                    Self::show_notification("Hush Complete", "No speech detected in audio", notify_rust::Urgency::Low);
                }
            }
        }

        Ok(())
    }

    async fn run_manual_mode(&mut self) -> Result<()> {
        info!("🔧 Running in manual mode");
        info!("Press Enter to start recording, press Enter again to stop and transcribe, or 'q' to quit.");
        
        use std::io::{self, BufRead};
        let stdin = io::stdin();
        let mut recording = false;

        for line in stdin.lock().lines() {
            let input = line?;
            
            if input.trim() == "q" {
                break;
            }

            if !recording {
                // Start recording
                info!("🎤 Starting recording... Press Enter to stop.");
                self.audio_capture.start_recording()?;
                self.is_recording.store(true, Ordering::Relaxed);
                self.recording_start_time = Some(Instant::now());
                recording = true;
                
                if self.notifications_enabled {
                    Self::show_notification("Hush Recording", "Recording started. Speak now...", notify_rust::Urgency::Normal);
                }
            } else {
                // Stop recording and transcribe
                let audio_data = self.audio_capture.stop_recording()?;
                self.is_recording.store(false, Ordering::Relaxed);
                recording = false;

                if !audio_data.is_empty() {
                    info!("🔄 Transcribing audio...");
                    let transcription_result = self.transcriber.transcribe_async(&audio_data, self.config.audio.sample_rate).await?;
                    let text = transcription_result.text.trim();

                    if !text.is_empty() {
                        info!("📝 Transcribed: '{}'", text);
                        self.text_inserter.insert_text(text)?;
                        info!("✅ Text inserted successfully");
                        
                        if self.notifications_enabled {
                            Self::show_notification("Hush Complete", &format!("Transcribed: {}", text), notify_rust::Urgency::Normal);
                        }
                    } else {
                        info!("🤐 No speech detected");
                    }
                }
                
                info!("Press Enter to record again, or 'q' to quit.");
            }
        }

        info!("👋 Manual mode ended");
        Ok(())
    }

    async fn show_status(&self) -> Result<()> {
        println!("🤫 Hush Voice-to-Text Status");
        println!("==============================");
        println!("Mode: {:?}", self.mode);
        println!("Audio device: {}", self.audio_capture.get_device_name());
        println!("Transcriber: {} (CUDA: {})", self.transcriber.get_device_info(), self.transcriber.is_using_cuda());
        println!("Notifications: {}", if self.notifications_enabled { "enabled" } else { "disabled" });
        
        if let Some(manager) = &self.hotkey_manager {
            println!("Hotkey: {} (available)", manager.get_combination());
        } else {
            println!("Hotkey: not available");
        }
        
        println!("Recording: {}", if self.is_recording.load(Ordering::Relaxed) { "active" } else { "inactive" });
        
        if let Some(start_time) = self.recording_start_time {
            println!("Recording duration: {:.2}s", start_time.elapsed().as_secs_f32());
        }
        
        Ok(())
    }

    fn show_notification(title: &str, message: &str, urgency: notify_rust::Urgency) {
        if let Err(e) = Notification::new()
            .summary(title)
            .body(message)
            .urgency(urgency)
            .timeout(3000)
            .show()
        {
            debug!("Failed to show notification: {:?}", e);
        }
    }

    async fn handle_hotkey_pressed(&mut self) -> Result<()> {
        if self.is_recording.load(Ordering::Relaxed) {
            debug!("Already recording, ignoring hotkey press");
            return Ok(());
        }

        info!("🎙️  Starting voice recording...");
        
        // Start recording
        self.audio_capture.start_recording()?;
        self.is_recording.store(true, Ordering::Relaxed);
        self.recording_start_time = Some(Instant::now());
        
        // Get current window info for context
        match self.text_inserter.get_focused_window() {
            Ok(window) => {
                info!("📝 Target window: '{}' ({})", window.title, window.class);
            }
            Err(e) => {
                warn!("Could not get window info: {:?}", e);
            }
        }

        info!("✅ Recording started - release hotkey to transcribe");
        Ok(())
    }

    async fn handle_hotkey_released(&mut self) -> Result<()> {
        if !self.is_recording.load(Ordering::Relaxed) {
            debug!("Not recording, ignoring hotkey release");
            return Ok(());
        }

        let recording_duration = self.recording_start_time
            .map(|start| start.elapsed())
            .unwrap_or_default();

        info!("🛑 Stopping recording (duration: {:.2}s)...", recording_duration.as_secs_f32());

        // Stop recording and get audio data
        let audio_data = self.audio_capture.stop_recording()?;
        self.is_recording.store(false, Ordering::Relaxed);
        self.recording_start_time = None;

        if audio_data.is_empty() {
            warn!("No audio data captured, skipping transcription");
            return Ok(());
        }

        info!("📊 Audio captured: {} samples ({:.2}s)", 
               audio_data.len(), 
               audio_data.len() as f32 / self.config.audio.sample_rate as f32);

        // Transcribe audio
        info!("🔄 Transcribing audio...");
        let transcription_start = Instant::now();
        
        let transcription_result = self.transcriber.transcribe_async(&audio_data, self.config.audio.sample_rate).await?;
        let transcription_duration = transcription_start.elapsed();
        
        info!("✅ Transcription completed in {:.2}s", transcription_duration.as_secs_f32());
        
        let text = transcription_result.text.trim();
        if text.is_empty() {
            info!("🤐 No speech detected in audio");
            return Ok(());
        }

        info!("📝 Transcribed text: '{}'", text);
        info!("🎯 Confidence: {:.2}", transcription_result.confidence);

        // Insert text at cursor position
        info!("⌨️  Inserting text at cursor...");
        let insertion_start = Instant::now();
        
        self.text_inserter.insert_text(text)?;
        let insertion_duration = insertion_start.elapsed();
        
        info!("✅ Text inserted successfully in {:.2}ms", insertion_duration.as_millis());

        // Calculate total pipeline latency
        let total_latency = transcription_duration + insertion_duration;
        info!("⚡ Total processing time: {:.2}ms", total_latency.as_millis());

        // Log performance metrics
        let words = text.split_whitespace().count();
        if words > 0 {
            info!("📈 Performance: {} words, {:.0} chars/sec transcription", 
                   words, 
                   text.len() as f32 / transcription_duration.as_secs_f32());
        }

        Ok(())
    }

    pub fn get_stats(&self) -> AppStats {
        AppStats {
            is_recording: self.is_recording.load(Ordering::Relaxed),
            recording_duration: self.recording_start_time.map(|start| start.elapsed()),
            audio_device: self.audio_capture.get_device_name().to_string(),
            transcriber_device: self.transcriber.get_device_info().to_string(),
            cuda_enabled: self.transcriber.is_using_cuda(),
            hotkey_combination: self.config.hotkey.combination.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppStats {
    pub is_recording: bool,
    pub recording_duration: Option<std::time::Duration>,
    pub audio_device: String,
    pub transcriber_device: String,
    pub cuda_enabled: bool,
    pub hotkey_combination: String,
}

impl Drop for HushApp {
    fn drop(&mut self) {
        if let Some(hotkey_manager) = &self.hotkey_manager {
            if let Err(e) = hotkey_manager.stop_listening() {
                error!("Error stopping hotkey listener: {:?}", e);
            }
        }
        info!("🧹 Hush application resources cleaned up");
    }
}

async fn install_desktop_integration() -> Result<()> {
    info!("📦 Installing desktop integration...");
    
    // Create .desktop file
    let desktop_dir = dirs::data_local_dir().ok_or_else(|| anyhow::anyhow!("Could not find local data directory"))?;
    let applications_dir = desktop_dir.join("applications");
    std::fs::create_dir_all(&applications_dir)?;
    
    let exe_path = std::env::current_exe()?;
    let desktop_content = format!(r#"[Desktop Entry]
Name=Hush Voice-to-Text
Comment=Voice-to-text transcription for developers
Exec={} one-shot --duration 10
Icon=audio-input-microphone
Type=Application
Categories=AudioVideo;Audio;
Keywords=voice;speech;dictation;
StartupNotify=false
NoDisplay=true
"#, exe_path.display());
    
    let desktop_file = applications_dir.join("hush.desktop");
    std::fs::write(&desktop_file, desktop_content)?;
    info!("✅ Created desktop file: {}", desktop_file.display());
    
    // Instructions for GNOME shortcut
    println!("\n🚀 Desktop integration installed!");
    println!("\nTo set up the keyboard shortcut in GNOME:");
    println!("1. Open Settings → Keyboard → Keyboard Shortcuts");
    println!("2. Click 'Custom Shortcuts' and add a new shortcut");
    println!("3. Name: 'Hush Voice-to-Text'");
    println!("4. Command: hush one-shot --duration 10");
    println!("5. Set shortcut: Ctrl+Shift+Space");
    println!("\nAlternatively, run 'hush manual' for hotkey-free operation.");
    
    Ok(())
}

async fn uninstall_desktop_integration() -> Result<()> {
    info!("🗑️  Removing desktop integration...");
    
    let desktop_dir = dirs::data_local_dir().ok_or_else(|| anyhow::anyhow!("Could not find local data directory"))?;
    let desktop_file = desktop_dir.join("applications").join("hush.desktop");
    
    if desktop_file.exists() {
        std::fs::remove_file(&desktop_file)?;
        info!("✅ Removed desktop file: {}", desktop_file.display());
    } else {
        info!("ℹ️  Desktop file not found, nothing to remove");
    }
    
    println!("\n✅ Desktop integration removed!");
    println!("Remember to remove any custom keyboard shortcuts in GNOME Settings.");
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Set up logging based on verbosity
    let log_level = match cli.verbose {
        0 => "hush=info",
        1 => "hush=debug",
        _ => "hush=trace,debug",
    };
    
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| log_level.into())
        )
        .init();
    
    // Handle special commands that don't need full app initialization
    match &cli.command {
        Some(Commands::Install) => {
            return install_desktop_integration().await;
        }
        Some(Commands::Uninstall) => {
            return uninstall_desktop_integration().await;
        }
        _ => {}
    }
    
    info!("🤫 Hush - Voice-to-Text for Linux Developers v0.1.0");
    
    // Check system dependencies
    if let Err(e) = hush::text::check_dependencies() {
        error!("❌ System dependency check failed: {:?}", e);
        error!("Please install required dependencies and try again");
        std::process::exit(1);
    }
    
    // Determine application mode
    let mode = match &cli.command {
        Some(Commands::Daemon { elevated }) => AppMode::Daemon { elevated: *elevated },
        Some(Commands::OneShot { duration, print_only }) => AppMode::OneShot { duration: *duration, print_only: *print_only },
        Some(Commands::Manual) => AppMode::Manual,
        Some(Commands::Status) => AppMode::Status,
        Some(Commands::Install) | Some(Commands::Uninstall) => unreachable!(), // handled above
        None => AppMode::Daemon { elevated: false }, // Default to daemon mode
    };
    
    let notifications_enabled = !cli.no_notifications;
    
    // Initialize and run the application
    match HushApp::new(mode, notifications_enabled, cli.config).await {
        Ok(mut app) => {
            // Set up Ctrl+C handler for long-running modes
            let running = Arc::new(AtomicBool::new(true));
            let running_clone = running.clone();
            
            tokio::spawn(async move {
                tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl+c");
                info!("📡 Received Ctrl+C, shutting down...");
                running_clone.store(false, Ordering::Relaxed);
            });

            // Run the main application
            if let Err(e) = app.run().await {
                error!("❌ Application error: {:?}", e);
                std::process::exit(1);
            }
        }
        Err(e) => {
            error!("❌ Failed to initialize Hush application: {:?}", e);
            error!("Check your configuration and system dependencies");
            
            // Provide helpful suggestions based on the error
            if e.to_string().contains("Hotkey") || e.to_string().contains("BadAccess") {
                error!("\n💡 Suggestions:");
                error!("   • Try 'hush manual' for hotkey-free operation");
                error!("   • Run 'hush install' to set up GNOME keyboard shortcuts");
                error!("   • Use 'hush one-shot' for single recordings");
            }
            std::process::exit(1);
        }
    }

    Ok(())
}
