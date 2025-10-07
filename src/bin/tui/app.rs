use crate::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use std::path::PathBuf;
use tracing::{info, warn};

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Daemon { elevated: bool },
    OneShot { duration: u64, print_only: bool },
    Manual,
    Status,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppScreen {
    Dashboard,
    ModeSelection,
    AudioDeviceSelection,
    ModelSelection,
    ConfigFileSelection, 
    HotkeyConfiguration,
    Recording,
    ConfigEditor,
    Help,
}

#[derive(Debug, Clone)]
pub enum FocusedWidget {
    ModeList,
    AudioDeviceList,
    ModelList,
    ConfigFileList,
    HotkeyInput,
    ConfigTree,
    RecordingControls,
}

pub struct App {
    // Core Hush components
    pub config: Config,
    pub audio_capture: Option<AudioCapture>,
    pub transcriber: Option<WhisperTranscriber>,
    pub text_inserter: Option<TextInserter>,
    pub hotkey_manager: Option<HotkeyManager>,
    pub hotkey_receiver: Option<Receiver<HotkeyEvent>>,
    
    // App state
    pub mode: AppMode,
    pub current_screen: AppScreen,
    pub previous_screen: Option<AppScreen>,
    pub should_quit: bool,
    pub notifications_enabled: bool,
    
    // Recording state
    pub is_recording: Arc<AtomicBool>,
    pub recording_start_time: Option<Instant>,
    pub last_transcription: String,
    pub recording_error: Option<String>,
    
    // UI navigation state
    pub focused_widget: FocusedWidget,
    pub selected_mode_index: usize,
    pub selected_audio_device_index: usize,
    pub selected_model_index: usize,
    pub selected_config_file_index: usize,
    pub config_tree_selected: usize,
    
    // Available options (populated during init)
    pub available_audio_devices: Vec<String>,
    pub available_models: Vec<ModelInfo>,
    pub available_config_files: Vec<PathBuf>,
    
    // Status information
    pub system_status: SystemStatus,
    pub logs: Vec<LogEntry>,
    
    // Configuration parameters for mode selection
    pub oneshot_duration: u64,
    pub oneshot_print_only: bool,
    pub daemon_elevated: bool,
    pub temp_hotkey_combination: String,
    
    // Help and error display
    pub show_help_overlay: bool,
    pub current_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub name: String,
    pub size: String,
    pub path: PathBuf,
    pub file_size: u64,
    pub is_available: bool,
}

#[derive(Debug, Clone)]
pub struct SystemStatus {
    pub audio_device_name: String,
    pub audio_device_available: bool,
    pub transcriber_ready: bool,
    pub transcriber_device: String,
    pub transcriber_cuda_enabled: bool,
    pub hotkey_available: bool,
    pub hotkey_combination: String,
    pub text_insertion_available: bool,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: Instant,
    pub level: LogLevel,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Debug,
}

impl Default for AppMode {
    fn default() -> Self {
        AppMode::Daemon { elevated: false }
    }
}

impl App {
    pub async fn new() -> Result<Self> {
        info!("🤫 Initializing Hush TUI Application");
        
        // Load configuration
        let config = Config::load()?;
        
        // Initialize available options
        let available_audio_devices = Self::get_available_audio_devices();
        let available_models = Self::get_available_models();
        let available_config_files = Self::get_available_config_files();
        
        // Create initial app state
        let mut app = Self {
            config: config.clone(),
            audio_capture: None,
            transcriber: None,
            text_inserter: None,
            hotkey_manager: None,
            hotkey_receiver: None,
            
            mode: AppMode::default(),
            current_screen: AppScreen::Dashboard,
            previous_screen: None,
            should_quit: false,
            notifications_enabled: true,
            
            is_recording: Arc::new(AtomicBool::new(false)),
            recording_start_time: None,
            last_transcription: String::new(),
            recording_error: None,
            
            focused_widget: FocusedWidget::ModeList,
            selected_mode_index: 0,
            selected_audio_device_index: 0,
            selected_model_index: 0,
            selected_config_file_index: 0,
            config_tree_selected: 0,
            
            available_audio_devices,
            available_models,
            available_config_files,
            
            system_status: SystemStatus::default(),
            logs: Vec::new(),
            
            oneshot_duration: 30,
            oneshot_print_only: false,
            daemon_elevated: false,
            temp_hotkey_combination: config.hotkey.combination.clone(),
            
            show_help_overlay: false,
            current_error: None,
        };
        
        // Initialize components
        app.initialize_components().await?;
        
        // Update system status
        app.update_system_status();
        
        info!("✅ Hush TUI Application initialized successfully");
        Ok(app)
    }
    
    async fn initialize_components(&mut self) -> Result<()> {
        // Initialize audio capture
        match AudioCapture::new(self.config.audio.device.as_deref()) {
            Ok(audio_capture) => {
                info!("✅ Audio capture initialized: {}", audio_capture.get_device_name());
                self.audio_capture = Some(audio_capture);
            }
            Err(e) => {
                warn!("⚠️  Audio capture failed: {}", e);
                self.add_log(LogLevel::Warn, format!("Audio capture failed: {}", e));
            }
        }
        
        // Initialize transcriber
        match WhisperTranscriber::new(&self.config.transcription.model_path, self.config.transcription.use_cuda).await {
            Ok(transcriber) => {
                info!("✅ Whisper transcriber initialized: {}", transcriber.get_device_info());
                self.transcriber = Some(transcriber);
            }
            Err(e) => {
                warn!("⚠️  Transcriber failed: {}", e);
                self.add_log(LogLevel::Warn, format!("Transcriber failed: {}", e));
            }
        }
        
        // Initialize text inserter
        match TextInserter::new() {
            Ok(text_inserter) => {
                info!("✅ Text inserter initialized");
                self.text_inserter = Some(text_inserter);
            }
            Err(e) => {
                warn!("⚠️  Text inserter failed: {}", e);
                self.add_log(LogLevel::Warn, format!("Text inserter failed: {}", e));
            }
        }
        
        // Initialize hotkey manager
        match HotkeyManager::new(&self.config.hotkey.combination) {
            Ok((manager, receiver)) => {
                info!("✅ Hotkey manager initialized: {}", manager.get_combination());
                self.hotkey_manager = Some(manager);
                self.hotkey_receiver = Some(receiver);
            }
            Err(e) => {
                warn!("⚠️  Hotkey manager failed: {}", e);
                self.add_log(LogLevel::Warn, format!("Hotkey manager failed: {}", e));
            }
        }
        
        Ok(())
    }
    
    fn get_available_audio_devices() -> Vec<String> {
        match AudioCapture::list_devices() {
            Ok(devices) => devices,
            Err(_) => vec!["Default".to_string()],
        }
    }
    
    fn get_available_models() -> Vec<ModelInfo> {
        // This would normally scan for available models
        vec![
            ModelInfo {
                name: "Tiny".to_string(),
                size: "39MB".to_string(),
                path: PathBuf::from("models/whisper-tiny.bin"),
                file_size: 39_000_000,
                is_available: std::path::Path::new("models/whisper-tiny.bin").exists(),
            },
            ModelInfo {
                name: "Small".to_string(),
                size: "244MB".to_string(),
                path: PathBuf::from("models/whisper-small.bin"),
                file_size: 244_000_000,
                is_available: std::path::Path::new("models/whisper-small.bin").exists(),
            },
            ModelInfo {
                name: "Medium".to_string(),
                size: "769MB".to_string(),
                path: PathBuf::from("models/whisper-medium.bin"),
                file_size: 769_000_000,
                is_available: std::path::Path::new("models/whisper-medium.bin").exists(),
            },
            ModelInfo {
                name: "Large".to_string(),
                size: "1550MB".to_string(),
                path: PathBuf::from("models/whisper-large.bin"),
                file_size: 1_550_000_000,
                is_available: std::path::Path::new("models/whisper-large.bin").exists(),
            },
        ]
    }
    
    fn get_available_config_files() -> Vec<PathBuf> {
        let mut configs = Vec::new();
        
        // Default config location
        if let Some(config_dir) = dirs::config_dir() {
            let hush_config = config_dir.join("hush").join("config.toml");
            if hush_config.exists() {
                configs.push(hush_config);
            }
        }
        
        // Project config
        let project_config = PathBuf::from("config/default.toml");
        if project_config.exists() {
            configs.push(project_config);
        }
        
        if configs.is_empty() {
            configs.push(PathBuf::from("config/default.toml")); // Show default even if not exists
        }
        
        configs
    }
    
    pub fn update_system_status(&mut self) {
        self.system_status = SystemStatus {
            audio_device_name: self.audio_capture.as_ref()
                .map(|ac| ac.get_device_name())
                .unwrap_or_else(|| "Not available".to_string()),
            audio_device_available: self.audio_capture.is_some(),
            transcriber_ready: self.transcriber.is_some(),
            transcriber_device: self.transcriber.as_ref()
                .map(|t| t.get_device_info())
                .unwrap_or_else(|| "Not available".to_string()),
            transcriber_cuda_enabled: self.transcriber.as_ref()
                .map(|t| t.is_using_cuda())
                .unwrap_or(false),
            hotkey_available: self.hotkey_manager.is_some(),
            hotkey_combination: self.config.hotkey.combination.clone(),
            text_insertion_available: self.text_inserter.is_some(),
        };
    }
    
    pub fn add_log(&mut self, level: LogLevel, message: String) {
        self.logs.push(LogEntry {
            timestamp: Instant::now(),
            level,
            message,
        });
        
        // Keep only last 100 log entries
        if self.logs.len() > 100 {
            self.logs.drain(0..self.logs.len() - 100);
        }
    }
    
    pub fn navigate_to_screen(&mut self, screen: AppScreen) {
        self.previous_screen = Some(self.current_screen.clone());
        self.current_screen = screen;
    }
    
    pub fn navigate_back(&mut self) {
        if let Some(previous) = self.previous_screen.take() {
            self.current_screen = previous;
        }
    }
    
    pub async fn start_recording(&mut self) -> Result<()> {
        if let Some(audio_capture) = &mut self.audio_capture {
            audio_capture.start_recording()?;
            self.is_recording.store(true, Ordering::Relaxed);
            self.recording_start_time = Some(Instant::now());
            self.recording_error = None;
            self.add_log(LogLevel::Info, "Recording started".to_string());
            Ok(())
        } else {
            Err(anyhow::anyhow!("Audio capture not available"))
        }
    }
    
    pub async fn stop_recording(&mut self) -> Result<()> {
        if let Some(audio_capture) = &mut self.audio_capture {
            let audio_data = audio_capture.stop_recording()?;
            self.is_recording.store(false, Ordering::Relaxed);
            self.recording_start_time = None;
            
            if !audio_data.is_empty() {
                if let Some(transcriber) = &self.transcriber {
                    match transcriber.transcribe_async(&audio_data, self.config.audio.sample_rate).await {
                        Ok(result) => {
                            self.last_transcription = result.text.trim().to_string();
                            self.add_log(LogLevel::Info, format!("Transcribed: {}", self.last_transcription));
                            
                            // Insert text if available
                            if let Some(text_inserter) = &self.text_inserter {
                                if let Err(e) = text_inserter.insert_text(&self.last_transcription) {
                                    self.add_log(LogLevel::Error, format!("Failed to insert text: {}", e));
                                }
                            }
                        }
                        Err(e) => {
                            self.recording_error = Some(e.to_string());
                            self.add_log(LogLevel::Error, format!("Transcription failed: {}", e));
                        }
                    }
                }
            } else {
                self.add_log(LogLevel::Warn, "No audio data recorded".to_string());
            }
            
            Ok(())
        } else {
            Err(anyhow::anyhow!("Audio capture not available"))
        }
    }
    
    pub fn get_recording_duration(&self) -> Duration {
        self.recording_start_time
            .map(|start| start.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0))
    }
    
    pub fn check_hotkey_events(&mut self) {
        if let Some(receiver) = &self.hotkey_receiver {
            match receiver.try_recv() {
                Ok(HotkeyEvent::Pressed) => {
                    if !self.is_recording.load(Ordering::Relaxed) {
                        if let Err(e) = tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(self.start_recording())
                        }) {
                            self.add_log(LogLevel::Error, format!("Failed to start recording: {}", e));
                        }
                    }
                }
                Ok(HotkeyEvent::Released) => {
                    if self.is_recording.load(Ordering::Relaxed) {
                        if let Err(e) = tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(self.stop_recording())
                        }) {
                            self.add_log(LogLevel::Error, format!("Failed to stop recording: {}", e));
                        }
                    }
                }
                Err(TryRecvError::Empty) => {
                    // No hotkey events
                }
                Err(TryRecvError::Disconnected) => {
                    self.add_log(LogLevel::Warn, "Hotkey receiver disconnected".to_string());
                }
            }
        }
    }
}

impl Default for SystemStatus {
    fn default() -> Self {
        Self {
            audio_device_name: "Not initialized".to_string(),
            audio_device_available: false,
            transcriber_ready: false,
            transcriber_device: "Not initialized".to_string(),
            transcriber_cuda_enabled: false,
            hotkey_available: false,
            hotkey_combination: "Not configured".to_string(),
            text_insertion_available: false,
        }
    }
}