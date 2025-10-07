use anyhow::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use std::path::PathBuf;
use crate::{Config, AudioCapture, WhisperTranscriber, TextInserter};
use std::sync::Mutex;

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

pub struct SimpleApp {
    // App state
    pub mode: AppMode,
    pub current_screen: AppScreen,
    pub previous_screen: Option<AppScreen>,
    pub should_quit: bool,
    pub notifications_enabled: bool,
    
    // Real Hush components
    pub config: Config,
    pub audio_capture: Option<AudioCapture>,
    pub transcriber: Option<WhisperTranscriber>,
    pub text_inserter: Option<TextInserter>,
    
    // Recording state (real)
    pub is_recording: Arc<AtomicBool>,
    pub recording_start_time: Option<Instant>,
    pub last_transcription: String,
    pub recording_error: Option<String>,
    pub current_audio_data: Arc<Mutex<Vec<f32>>>,
    
    // UI navigation state
    pub focused_widget: FocusedWidget,
    pub selected_mode_index: usize,
    pub selected_audio_device_index: usize,
    pub selected_model_index: usize,
    pub selected_config_file_index: usize,
    pub config_tree_selected: usize,
    
    // Available options (mock data)
    pub available_audio_devices: Vec<String>,
    pub available_models: Vec<ModelInfo>,
    pub available_config_files: Vec<PathBuf>,
    
    // Status information (mock)
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

impl SimpleApp {
    pub fn new() -> Result<Self> {
        // Load real configuration
        let config = Config::load()?;
        
        // Initialize audio capture
        let audio_capture = match AudioCapture::new(config.audio.device.as_deref()) {
            Ok(capture) => {
                Some(capture)
            }
            Err(e) => {
                eprintln!("Warning: Failed to initialize audio capture: {}", e);
                None
            }
        };
        
        // Initialize text inserter
        let text_inserter = match TextInserter::new() {
            Ok(inserter) => Some(inserter),
            Err(e) => {
                eprintln!("Warning: Failed to initialize text inserter: {}", e);
                None
            }
        };
        
        let mut app = Self {
            mode: AppMode::default(),
            current_screen: AppScreen::Dashboard,
            previous_screen: None,
            should_quit: false,
            notifications_enabled: true,
            
            config: config.clone(),
            audio_capture,
            transcriber: None, // Initialize lazily when needed
            text_inserter,
            
            is_recording: Arc::new(AtomicBool::new(false)),
            recording_start_time: None,
            last_transcription: "Welcome to Hush TUI!".to_string(),
            recording_error: None,
            current_audio_data: Arc::new(Mutex::new(Vec::new())),
            
            focused_widget: FocusedWidget::ModeList,
            selected_mode_index: 0,
            selected_audio_device_index: 0,
            selected_model_index: 0,
            selected_config_file_index: 0,
            config_tree_selected: 0,
            
            available_audio_devices: Self::get_real_audio_devices(),
            available_models: Self::get_available_models(),
            available_config_files: vec![
                PathBuf::from("config/default.toml"),
                PathBuf::from("~/.config/hush/config.toml"),
            ],
            
            system_status: SystemStatus::default(),
            logs: Vec::new(),
            
            oneshot_duration: 30,
            oneshot_print_only: false,
            daemon_elevated: false,
            temp_hotkey_combination: "Ctrl+Shift+Space".to_string(),
            
            show_help_overlay: false,
            current_error: None,
        };
        
        // Add initial logs
        app.add_log(LogLevel::Info, "Hush TUI started successfully".to_string());
        if app.audio_capture.is_some() {
            app.add_log(LogLevel::Info, "Audio capture initialized".to_string());
        } else {
            app.add_log(LogLevel::Warn, "Audio capture initialization failed".to_string());
        }
        if app.text_inserter.is_some() {
            app.add_log(LogLevel::Info, "Text inserter initialized".to_string());
        } else {
            app.add_log(LogLevel::Warn, "Text inserter initialization failed".to_string());
        }
        app.add_log(LogLevel::Info, "Whisper transcriber will be initialized on first use".to_string());
        
        // Update system status
        app.update_system_status();
        
        Ok(app)
    }
    
    fn get_real_audio_devices() -> Vec<String> {
        match AudioCapture::list_devices() {
            Ok(devices) => {
                if devices.is_empty() {
                    vec!["Default Audio Device".to_string()]
                } else {
                    devices
                }
            }
            Err(_) => vec!["Default Audio Device".to_string(), "Built-in Microphone".to_string()],
        }
    }
    
    fn get_available_models() -> Vec<ModelInfo> {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("hush")
            .join("models");
            
        let models = vec![
            ("tiny", "39MB", 39_000_000),
            ("base", "142MB", 142_000_000),
            ("small", "244MB", 244_000_000),
            ("medium", "769MB", 769_000_000),
            ("large", "1550MB", 1_550_000_000),
        ];
        
        models
            .into_iter()
            .map(|(name, size, file_size)| {
                let model_path = cache_dir.join(format!("ggml-{}.bin", name));
                let is_available = model_path.exists();
                
                ModelInfo {
                    name: name.to_string(),
                    size: size.to_string(),
                    path: model_path,
                    file_size,
                    is_available,
                }
            })
            .collect()
    }
    
    pub fn update_system_status(&mut self) {
        self.system_status = SystemStatus {
            audio_device_name: self.available_audio_devices.get(self.selected_audio_device_index)
                .cloned()
                .unwrap_or_else(|| "Default Audio Device".to_string()),
            audio_device_available: self.audio_capture.is_some(),
            transcriber_ready: self.transcriber.is_some(),
            transcriber_device: if self.transcriber.is_some() {
                if self.config.transcription.use_cuda {
                    "CUDA".to_string()
                } else {
                    "CPU".to_string()
                }
            } else {
                "Not initialized".to_string()
            },
            transcriber_cuda_enabled: self.config.transcription.use_cuda,
            hotkey_available: false, // TODO: Connect to real hotkey system
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
        
        // Keep only last 50 log entries for demo
        if self.logs.len() > 50 {
            self.logs.drain(0..self.logs.len() - 50);
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
    
    pub fn start_recording(&mut self) -> Result<()> {
        if !self.is_recording.load(Ordering::Relaxed) {
            if let Some(ref mut audio_capture) = self.audio_capture {
                match audio_capture.start_recording() {
                    Ok(()) => {
                        self.is_recording.store(true, Ordering::Relaxed);
                        self.recording_start_time = Some(Instant::now());
                        self.recording_error = None;
                        // Clear previous audio data
                        if let Ok(mut audio_data) = self.current_audio_data.lock() {
                            audio_data.clear();
                        }
                        self.add_log(LogLevel::Info, "Recording started".to_string());
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to start recording: {}", e);
                        self.recording_error = Some(error_msg.clone());
                        self.add_log(LogLevel::Error, error_msg);
                        return Err(e);
                    }
                }
            } else {
                let error_msg = "Audio capture not initialized".to_string();
                self.recording_error = Some(error_msg.clone());
                self.add_log(LogLevel::Error, error_msg.clone());
                return Err(anyhow::anyhow!(error_msg));
            }
        }
        Ok(())
    }
    
    pub async fn stop_recording(&mut self) -> Result<()> {
        if self.is_recording.load(Ordering::Relaxed) {
            self.is_recording.store(false, Ordering::Relaxed);
            let duration = self.get_recording_duration();
            self.recording_start_time = None;
            
            if let Some(ref mut audio_capture) = self.audio_capture {
                match audio_capture.stop_recording() {
                    Ok(audio_data) => {
                        self.add_log(LogLevel::Info, format!("Recording stopped after {:.1}s ({} samples)", 
                                    duration.as_secs_f32(), audio_data.len()));
                        
                        // Store audio data for potential transcription
                        if let Ok(mut stored_data) = self.current_audio_data.lock() {
                            *stored_data = audio_data.clone();
                        }
                        
                        // Initialize transcriber if needed
                        if self.transcriber.is_none() {
                            self.add_log(LogLevel::Info, "Initializing transcriber...".to_string());
                            match WhisperTranscriber::new(&self.config.transcription.model_path, 
                                                         self.config.transcription.use_cuda).await {
                                Ok(transcriber) => {
                                    self.transcriber = Some(transcriber);
                                    self.add_log(LogLevel::Info, "Transcriber initialized successfully".to_string());
                                }
                                Err(e) => {
                                    let error_msg = format!("Failed to initialize transcriber: {}", e);
                                    self.add_log(LogLevel::Error, error_msg.clone());
                                    self.recording_error = Some(error_msg);
                                    return Ok(()); // Don't fail completely, just skip transcription
                                }
                            }
                        }
                        
                        // Transcribe audio
                        if self.transcriber.is_some() {
                            self.add_log(LogLevel::Info, "Transcribing audio...".to_string());
                            
                            // Extract transcriber to avoid borrow checker issues
                            let transcription_result = if let Some(ref transcriber) = self.transcriber {
                                transcriber.transcribe(&audio_data)
                            } else {
                                return Ok(()); // Should not happen due to is_some() check above
                            };
                            
                            match transcription_result {
                                Ok(text) => {
                                    self.last_transcription = format!("({:.1}s): {}", 
                                                                     duration.as_secs_f32(), text);
                                    self.add_log(LogLevel::Info, format!("Transcribed: {}", text));
                                    
                                    // Insert text if text inserter is available
                                    if let Some(ref mut text_inserter) = self.text_inserter {
                                        match text_inserter.insert_text(&text) {
                                            Ok(()) => {
                                                self.add_log(LogLevel::Info, "Text inserted successfully".to_string());
                                            }
                                            Err(e) => {
                                                self.add_log(LogLevel::Warn, format!("Text insertion failed: {}", e));
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    let error_msg = format!("Transcription failed: {}", e);
                                    self.add_log(LogLevel::Error, error_msg.clone());
                                    self.recording_error = Some(error_msg);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to stop recording: {}", e);
                        self.recording_error = Some(error_msg.clone());
                        self.add_log(LogLevel::Error, error_msg);
                        return Err(e);
                    }
                }
            }
        }
        Ok(())
    }
    
    pub fn get_recording_duration(&self) -> Duration {
        self.recording_start_time
            .map(|start| start.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0))
    }
    
    pub async fn toggle_recording(&mut self) -> Result<()> {
        if self.is_recording.load(Ordering::Relaxed) {
            self.stop_recording().await
        } else {
            self.start_recording()
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