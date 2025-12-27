use crate::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main configuration structure for the Hush application
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    /// Audio capture settings
    pub audio: AudioConfig,
    /// Whisper transcription settings
    pub transcription: TranscriptionConfig,
    /// Global hotkey configuration
    pub hotkey: HotkeyConfig,
    /// User feedback settings (audio cues, notifications)
    pub feedback: FeedbackConfig,
}

/// Audio capture configuration
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AudioConfig {
    /// Sample rate in Hz (8000-48000, default 16000 for Whisper)
    pub sample_rate: u32,
    /// Number of audio channels (1=mono, 2=stereo)
    pub channels: u16,
    /// Audio buffer size in samples (power of 2, 64-8192)
    pub buffer_size: usize,
    /// Optional specific audio device name (None uses system default)
    pub device: Option<String>,
}

/// Whisper transcription model configuration
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranscriptionConfig {
    /// Path to the Whisper model binary file
    pub model_path: PathBuf,
    /// Model size identifier (tiny, base, small, medium, large, large-v2, large-v3)
    pub model_size: String,
    /// Language code for transcription (e.g., "en" for English)
    pub language: String,
    /// Enable CUDA GPU acceleration (requires CUDA-enabled build)
    pub use_cuda: bool,
    /// Beam search width (1-20, higher = more accurate but slower)
    pub beam_size: usize,
    /// Threshold for detecting silence/no speech (0.0-1.0)
    pub no_speech_threshold: f32,
}

/// Global hotkey listener configuration
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HotkeyConfig {
    /// Whether hotkey listening is enabled
    pub enabled: bool,
    /// Hotkey combination string (e.g., "Ctrl+Shift+Space")
    pub combination: String,
}

/// User feedback and notification settings
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeedbackConfig {
    /// Enable audio feedback sounds (beeps, clicks)
    pub audio_enabled: bool,
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
        let valid_models = [
            "tiny", "base", "small", "medium", "large", "large-v2", "large-v3",
        ];
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

        Ok(())
    }

    /// Save configuration to the default config file
    pub fn save(&self) -> Result<()> {
        self.save_to_file("config/default.toml")
    }

    /// Save configuration to a specific file path
    pub fn save_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<()> {
        use std::fs;

        // Validate before saving
        self.validate()?;

        // Serialize to TOML
        let toml_string = toml::to_string_pretty(self)
            .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;

        // Write to file
        fs::write(path.as_ref(), toml_string).map_err(|e| {
            anyhow::anyhow!(
                "Failed to write config file {}: {}",
                path.as_ref().display(),
                e
            )
        })?;

        Ok(())
    }

    /// Create a default configuration programmatically
    pub fn programmatic_default() -> Self {
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
                combination: "Ctrl+Shift+Space".to_string(),
            },
            feedback: FeedbackConfig {
                audio_enabled: true,
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
