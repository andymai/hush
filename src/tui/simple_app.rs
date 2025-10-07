use anyhow::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use std::path::PathBuf;

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
    
    // Recording state (simulated)
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
        let mut app = Self {
            mode: AppMode::default(),
            current_screen: AppScreen::Dashboard,
            previous_screen: None,
            should_quit: false,
            notifications_enabled: true,
            
            is_recording: Arc::new(AtomicBool::new(false)),
            recording_start_time: None,
            last_transcription: "Welcome to Hush TUI! This is a demonstration.".to_string(),
            recording_error: None,
            
            focused_widget: FocusedWidget::ModeList,
            selected_mode_index: 0,
            selected_audio_device_index: 0,
            selected_model_index: 0,
            selected_config_file_index: 0,
            config_tree_selected: 0,
            
            available_audio_devices: vec![
                "Default Audio Device".to_string(),
                "Built-in Microphone".to_string(),
                "USB Headset".to_string(),
            ],
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
        app.add_log(LogLevel::Info, "Audio capture initialized (demo mode)".to_string());
        app.add_log(LogLevel::Info, "Whisper transcriber ready (demo mode)".to_string());
        
        // Update system status
        app.update_system_status();
        
        Ok(app)
    }
    
    fn get_available_models() -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                name: "Tiny".to_string(),
                size: "39MB".to_string(),
                path: PathBuf::from("models/whisper-tiny.bin"),
                file_size: 39_000_000,
                is_available: true, // Mock as available
            },
            ModelInfo {
                name: "Small".to_string(),
                size: "244MB".to_string(),
                path: PathBuf::from("models/whisper-small.bin"),
                file_size: 244_000_000,
                is_available: false,
            },
            ModelInfo {
                name: "Medium".to_string(),
                size: "769MB".to_string(),
                path: PathBuf::from("models/whisper-medium.bin"),
                file_size: 769_000_000,
                is_available: false,
            },
            ModelInfo {
                name: "Large".to_string(),
                size: "1550MB".to_string(),
                path: PathBuf::from("models/whisper-large.bin"),
                file_size: 1_550_000_000,
                is_available: false,
            },
        ]
    }
    
    pub fn update_system_status(&mut self) {
        self.system_status = SystemStatus {
            audio_device_name: self.available_audio_devices.get(self.selected_audio_device_index)
                .cloned()
                .unwrap_or_else(|| "Default Audio Device".to_string()),
            audio_device_available: true, // Mock as available
            transcriber_ready: true,      // Mock as ready
            transcriber_device: "CPU (demo mode)".to_string(),
            transcriber_cuda_enabled: false,
            hotkey_available: true,       // Mock as available
            hotkey_combination: self.temp_hotkey_combination.clone(),
            text_insertion_available: true, // Mock as available
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
            self.is_recording.store(true, Ordering::Relaxed);
            self.recording_start_time = Some(Instant::now());
            self.recording_error = None;
            self.add_log(LogLevel::Info, "Recording started (demo mode)".to_string());
        }
        Ok(())
    }
    
    pub fn stop_recording(&mut self) -> Result<()> {
        if self.is_recording.load(Ordering::Relaxed) {
            self.is_recording.store(false, Ordering::Relaxed);
            let duration = self.get_recording_duration();
            self.recording_start_time = None;
            
            // Simulate transcription
            let demo_transcriptions = vec![
                "Hello, this is a demonstration of the Hush TUI interface.",
                "The recording feature is working in demo mode.",
                "You can navigate between different screens using number keys.",
                "Press question mark for help or ESC to go back.",
                "This shows how the transcription results would appear.",
            ];
            
            let transcription = demo_transcriptions[self.logs.len() % demo_transcriptions.len()];
            self.last_transcription = format!("Demo transcription ({:.1}s): {}", 
                                            duration.as_secs_f32(), transcription);
            
            self.add_log(LogLevel::Info, format!("Recording stopped after {:.1}s", duration.as_secs_f32()));
            self.add_log(LogLevel::Info, format!("Transcribed: {}", transcription));
        }
        Ok(())
    }
    
    pub fn get_recording_duration(&self) -> Duration {
        self.recording_start_time
            .map(|start| start.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0))
    }
    
    pub fn toggle_recording(&mut self) -> Result<()> {
        if self.is_recording.load(Ordering::Relaxed) {
            self.stop_recording()
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