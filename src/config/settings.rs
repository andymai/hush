use crate::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub audio: AudioConfig,
    pub transcription: TranscriptionConfig,
    pub hotkey: HotkeyConfig,
    pub wakeword: WakeWordConfig,
    pub feedback: FeedbackConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_size: usize,
    pub device: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranscriptionConfig {
    pub model_path: PathBuf,
    pub model_size: String,
    pub language: String,
    pub use_cuda: bool,
    pub beam_size: usize,
    pub no_speech_threshold: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HotkeyConfig {
    pub enabled: bool,
    pub combination: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WakeWordConfig {
    pub enabled: bool,
    pub phrase: String,
    pub sensitivity: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeedbackConfig {
    pub audio_enabled: bool,
    pub start_sound: PathBuf,
    pub stop_sound: PathBuf,
    pub error_sound: PathBuf,
}

impl Config {
    pub fn load() -> Result<Self> {
        Self::load_from_file("config/default.toml")
    }

    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        use std::fs;

        let config_str = fs::read_to_string(path.as_ref()).map_err(|e| {
            anyhow::anyhow!(
                "Failed to read config file {}: {}",
                path.as_ref().display(),
                e
            )
        })?;

        let config: Config = toml::from_str(&config_str)
            .map_err(|e| anyhow::anyhow!("Failed to parse TOML config: {}", e))?;

        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        // Validate audio configuration
        if self.audio.sample_rate < 8000 || self.audio.sample_rate > 48000 {
            return Err(anyhow::anyhow!(
                "Invalid sample rate: must be between 8000 and 48000 Hz"
            ));
        }
        if self.audio.channels == 0 || self.audio.channels > 2 {
            return Err(anyhow::anyhow!("Invalid channels: must be 1 or 2"));
        }
        if self.audio.buffer_size < 64 || self.audio.buffer_size > 8192 {
            return Err(anyhow::anyhow!(
                "Invalid buffer size: must be between 64 and 8192"
            ));
        }

        // Validate transcription configuration
        let valid_models = ["tiny", "base", "small", "medium", "large"];
        if !valid_models.contains(&self.transcription.model_size.as_str()) {
            return Err(anyhow::anyhow!(
                "Invalid model size: must be one of {:?}",
                valid_models
            ));
        }
        if self.transcription.beam_size < 1 || self.transcription.beam_size > 20 {
            return Err(anyhow::anyhow!(
                "Invalid beam size: must be between 1 and 20"
            ));
        }

        // Validate hotkey configuration
        if self.hotkey.enabled && self.hotkey.combination.is_empty() {
            return Err(anyhow::anyhow!(
                "Hotkey combination cannot be empty when hotkey is enabled"
            ));
        }

        // Validate wakeword configuration
        if self.wakeword.enabled && self.wakeword.phrase.is_empty() {
            return Err(anyhow::anyhow!(
                "Wake phrase cannot be empty when wake word is enabled"
            ));
        }
        if self.wakeword.sensitivity < 0.0 || self.wakeword.sensitivity > 1.0 {
            return Err(anyhow::anyhow!(
                "Wake word sensitivity must be between 0.0 and 1.0"
            ));
        }

        Ok(())
    }

    /// Create a default configuration programmatically
    pub fn default() -> Self {
        Config {
            audio: AudioConfig {
                sample_rate: 16000,
                channels: 1,
                buffer_size: 1024,
                device: None,
            },
            transcription: TranscriptionConfig {
                model_path: PathBuf::from("models/whisper-medium.bin"),
                model_size: "medium".to_string(),
                language: "en".to_string(),
                use_cuda: true,
                beam_size: 5,
                no_speech_threshold: 0.6,
            },
            hotkey: HotkeyConfig {
                enabled: true,
                #[cfg(target_os = "macos")]
                combination: "Cmd+Shift+V".to_string(),
                #[cfg(not(target_os = "macos"))]
                combination: "Ctrl+Alt+V".to_string(),
            },
            wakeword: WakeWordConfig {
                enabled: false,
                phrase: "Hey Hush".to_string(),
                sensitivity: 0.8,
            },
            feedback: FeedbackConfig {
                audio_enabled: true,
                start_sound: PathBuf::from("assets/sounds/start.wav"),
                stop_sound: PathBuf::from("assets/sounds/stop.wav"),
                error_sound: PathBuf::from("assets/sounds/error.wav"),
            },
        }
    }
}

pub struct ConfigWatcher {
    // Will be implemented later
}

impl ConfigWatcher {
    pub fn new() -> Result<Self> {
        todo!("Implement in later task")
    }
}
