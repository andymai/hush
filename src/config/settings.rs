use crate::config::paths;
use crate::hotkey::{HotkeyBackend, HotkeyMode, KeyCombination};
use crate::text::InsertionMethod;
use crate::transcription::models::ModelSize;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const DEFAULTS: &str = include_str!("../../config/default.toml");

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
    /// How text reaches the focused window
    #[serde(default)]
    pub insertion: InsertionConfig,
    /// Tone by application
    #[serde(default = "crate::text_processing::ProfilesConfig::defaults")]
    pub profiles: crate::text_processing::ProfilesConfig,
    /// LLM polishing and Command Mode
    #[serde(default)]
    pub llm: LlmConfig,
}

/// Which LLM to use
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LlmSetting {
    /// Ollama when reachable, else Anthropic when a key exists, else none
    #[default]
    Auto,
    Ollama,
    Anthropic,
    None,
}

/// LLM settings; the Anthropic key comes from `ANTHROPIC_API_KEY`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LlmConfig {
    pub provider: LlmSetting,
    pub ollama_url: String,
    pub ollama_model: String,
    pub anthropic_model: String,
    /// Polish every transcript with the LLM; Command Mode works either way
    pub polish: bool,
    pub max_tokens: u32,
    pub temperature: f32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

impl LlmConfig {
    pub fn defaults() -> Self {
        Self {
            provider: LlmSetting::Auto,
            ollama_url: "http://localhost:11434".to_string(),
            ollama_model: "llama3.2".to_string(),
            anthropic_model: "claude-haiku-4-5".to_string(),
            polish: true,
            max_tokens: 400,
            temperature: 0.3,
        }
    }
}

/// Text insertion configuration
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct InsertionConfig {
    /// auto, uinput, or clipboard
    pub method: InsertionMethod,
    /// Delay between typed keys in milliseconds
    pub typing_delay_ms: u64,
}

impl Default for InsertionConfig {
    fn default() -> Self {
        Self {
            method: InsertionMethod::Auto,
            typing_delay_ms: 10,
        }
    }
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
    /// A recording stops and transcribes after this long, with a warning a
    /// minute earlier; 0 disables the cap
    #[serde(default = "default_max_recording_secs")]
    pub max_recording_secs: u64,
}

fn default_max_recording_secs() -> u64 {
    600
}

/// Whisper transcription model configuration
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranscriptionConfig {
    /// Explicit model file. When unset, the file for `model_size` inside the
    /// models directory is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_path: Option<PathBuf>,
    /// Model size identifier (tiny, base, small, medium, large, large-v2, large-v3)
    pub model_size: String,
    /// Language code for transcription (e.g., "en" for English)
    pub language: String,
    /// Use the GPU backend the binary was built with (`vulkan` or `cuda`)
    pub use_gpu: bool,
    /// Beam search width (1-20, higher = more accurate but slower)
    pub beam_size: usize,
    /// Threshold for detecting silence/no speech (0.0-1.0)
    pub no_speech_threshold: f32,
    /// Prime Whisper with the focused window's title and learned terms
    #[serde(default = "default_true")]
    pub context_prompt: bool,
}

fn default_true() -> bool {
    true
}

impl TranscriptionConfig {
    /// The model file to load: the explicit path when set, otherwise the
    /// catalogue file for `model_size` in the models directory.
    pub fn model_path(&self) -> PathBuf {
        match &self.model_path {
            Some(path) => path.clone(),
            None => {
                let filename = self
                    .model_size
                    .parse::<ModelSize>()
                    .map(|size| size.filename())
                    .unwrap_or_else(|_| format!("ggml-{}.bin", self.model_size));
                paths::models_dir().join(filename)
            },
        }
    }
}

/// Global hotkey listener configuration
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HotkeyConfig {
    /// Whether hotkey listening is enabled
    pub enabled: bool,
    /// Hotkey combination string (e.g., "Ctrl+Shift+Space")
    pub combination: String,
    /// Hold-to-talk or toggle
    #[serde(default)]
    pub mode: HotkeyMode,
    /// Which backend reads the keyboard
    #[serde(default)]
    pub backend: HotkeyBackend,
    /// Grab the hotkey's device so applications never see the hotkey (evdev)
    #[serde(default)]
    pub exclusive: bool,
    /// Key that discards a recording in progress; empty disables it
    #[serde(default = "default_cancel_key")]
    pub cancel: String,
    /// A press shorter than this is a tap: in hold mode a tap discards and a
    /// double-tap locks hands-free, in toggle mode a tap locks
    #[serde(default = "default_tap_ms")]
    pub tap_ms: u64,
    /// Chord that types the last transcript again; empty leaves it unbound
    #[serde(default)]
    pub paste_last: String,
    /// Chord that adds the selected text to the vocabulary; empty leaves it unbound
    #[serde(default)]
    pub learn: String,
    /// Command Mode chord: hold, speak an instruction, release; empty disables it
    #[serde(default = "default_command_chord")]
    pub command: String,
}

fn default_command_chord() -> String {
    "Ctrl+RightAlt".to_string()
}

fn default_cancel_key() -> String {
    "Escape".to_string()
}

fn default_tap_ms() -> u64 {
    300
}

/// User feedback and notification settings
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeedbackConfig {
    /// Enable audio feedback sounds (beeps, clicks)
    pub audio_enabled: bool,
}

impl Config {
    /// The compiled-in defaults from `config/default.toml`.
    pub fn defaults() -> Result<Self> {
        Self::parse_over_defaults("")
    }

    /// Where the user configuration file lives.
    pub fn path() -> PathBuf {
        paths::config_path()
    }

    /// Load the user configuration, falling back to defaults when no file exists.
    pub fn load() -> Result<Self> {
        Self::load_from(None)
    }

    /// Load from an explicit file, or from the resolved location when `explicit`
    /// is `None`. A file named explicitly, by argument or by `HUSH_CONFIG`, must
    /// exist; the default location may be absent.
    pub fn load_from(explicit: Option<&Path>) -> Result<Self> {
        match explicit {
            Some(path) if !path.exists() => {
                Err(anyhow::anyhow!("Config file not found: {}", path.display()))
            },
            Some(path) => Self::load_from_file(path),
            None => {
                let path = paths::config_path();
                if path.exists() {
                    Self::load_from_file(&path)
                } else if paths::config_path_is_explicit() {
                    Err(anyhow::anyhow!("Config file not found: {}", path.display()))
                } else {
                    Self::defaults()
                }
            },
        }
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let user = std::fs::read_to_string(path.as_ref()).map_err(|e| {
            anyhow::anyhow!(
                "Failed to read config file {}: {}",
                path.as_ref().display(),
                e
            )
        })?;
        Self::parse_over_defaults(&user)
            .map_err(|e| anyhow::anyhow!("Invalid config file {}: {}", path.as_ref().display(), e))
    }

    fn parse_over_defaults(user_toml: &str) -> Result<Self> {
        let mut merged: toml::Table = toml::from_str(DEFAULTS)
            .map_err(|e| anyhow::anyhow!("Built-in defaults are invalid TOML: {}", e))?;
        let mut user: toml::Table = toml::from_str(user_toml)
            .map_err(|e| anyhow::anyhow!("Failed to parse TOML config: {}", e))?;
        rename_legacy_keys(&mut user);
        merge_tables(&mut merged, user);
        let config: Config = toml::Value::Table(merged)
            .try_into()
            .map_err(|e| anyhow::anyhow!("Failed to parse TOML config: {}", e))?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        if self.hotkey.tap_ms > 2000 {
            return Err(anyhow::anyhow!(
                "Invalid hotkey.tap_ms: must be 2000 or less"
            ));
        }
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

        if self.insertion.typing_delay_ms > 200 {
            return Err(anyhow::anyhow!(
                "Invalid typing delay: must be at most 200 ms"
            ));
        }

        // Validate hotkey configuration
        if self.hotkey.enabled {
            if self.hotkey.combination.is_empty() {
                return Err(anyhow::anyhow!(
                    "Hotkey combination cannot be empty when hotkey is enabled"
                ));
            }
            KeyCombination::parse(&self.hotkey.combination)?;
        }

        Ok(())
    }

    /// Save to the user configuration file, creating its directory as needed.
    pub fn save(&self) -> Result<()> {
        self.save_to_file(paths::config_path())
    }

    /// Save configuration to a specific file path
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        self.validate()?;

        let toml_string = toml::to_string_pretty(self)
            .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;

        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to create config directory {}: {}",
                    parent.display(),
                    e
                )
            })?;
        }
        std::fs::write(path.as_ref(), toml_string).map_err(|e| {
            anyhow::anyhow!(
                "Failed to write config file {}: {}",
                path.as_ref().display(),
                e
            )
        })?;

        Ok(())
    }
}

/// Keys from earlier config layouts, renamed before the merge so they neither
/// collide with the current key nor fail as unknown.
fn rename_legacy_keys(user: &mut toml::Table) {
    if let Some(toml::Value::Table(transcription)) = user.get_mut("transcription") {
        if let Some(value) = transcription.remove("use_cuda") {
            transcription.entry("use_gpu").or_insert(value);
        }
    }
}

fn merge_tables(base: &mut toml::Table, overlay: toml::Table) {
    for (key, value) in overlay {
        match (base.get_mut(&key), value) {
            (Some(toml::Value::Table(existing)), toml::Value::Table(incoming)) => {
                merge_tables(existing, incoming)
            },
            (_, value) => {
                base.insert(key, value);
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_parse_and_validate() {
        let config = Config::defaults().unwrap();
        assert_eq!(config.audio.sample_rate, 16000);
        assert_eq!(config.transcription.model_size, "base");
        assert_eq!(config.hotkey.combination, "Ctrl+Shift+Space");
        assert!(config.transcription.model_path.is_none());
        assert!(config.audio.device.is_none());
    }

    #[test]
    fn partial_user_file_merges_over_defaults() {
        let config = Config::parse_over_defaults(
            r#"
[hotkey]
combination = "F12"

[transcription]
model_size = "small"
"#,
        )
        .unwrap();
        assert_eq!(config.hotkey.combination, "F12");
        assert!(config.hotkey.enabled);
        assert_eq!(config.transcription.model_size, "small");
        assert_eq!(config.transcription.beam_size, 5);
        assert_eq!(config.audio.sample_rate, 16000);
    }

    #[test]
    fn legacy_use_cuda_key_is_renamed() {
        let config = Config::parse_over_defaults("[transcription]\nuse_cuda = false\n").unwrap();
        assert!(!config.transcription.use_gpu);

        let both = "[transcription]\nuse_cuda = false\nuse_gpu = true\n";
        assert!(
            Config::parse_over_defaults(both)
                .unwrap()
                .transcription
                .use_gpu
        );
    }

    #[test]
    fn hotkey_mode_and_backend_default_and_parse() {
        let config = Config::defaults().unwrap();
        assert_eq!(config.hotkey.mode, HotkeyMode::Hold);
        assert_eq!(config.hotkey.backend, HotkeyBackend::Auto);
        assert!(!config.hotkey.exclusive);
        let config =
            Config::parse_over_defaults("[hotkey]\nmode = \"toggle\"\nbackend = \"x11\"\n")
                .unwrap();
        assert_eq!(config.hotkey.mode, HotkeyMode::Toggle);
        assert_eq!(config.hotkey.backend, HotkeyBackend::X11);
        assert!(Config::parse_over_defaults("[hotkey]\ncombination = \"Ctrl+Bogus\"\n").is_err());
    }

    #[test]
    fn insertion_section_defaults_and_parses() {
        let config = Config::defaults().unwrap();
        assert_eq!(config.insertion.method, InsertionMethod::Auto);
        assert_eq!(config.insertion.typing_delay_ms, 10);
        let config = Config::parse_over_defaults("[insertion]\nmethod = \"clipboard\"\n").unwrap();
        assert_eq!(config.insertion.method, InsertionMethod::Clipboard);
        assert!(Config::parse_over_defaults("[insertion]\ntyping_delay_ms = 999\n").is_err());
    }

    #[test]
    fn invalid_user_value_is_rejected() {
        let err = Config::parse_over_defaults("[audio]\nsample_rate = 1\n").unwrap_err();
        assert!(err.to_string().contains("sample rate"));
    }

    #[test]
    fn model_path_is_derived_from_the_catalogue() {
        let mut config = Config::defaults().unwrap();
        config.transcription.model_size = "large".into();
        assert!(config
            .transcription
            .model_path()
            .ends_with("ggml-large-v1.bin"));

        config.transcription.model_path = Some(PathBuf::from("/tmp/custom.bin"));
        assert_eq!(
            config.transcription.model_path(),
            PathBuf::from("/tmp/custom.bin")
        );
    }

    #[test]
    fn explicit_missing_file_is_an_error() {
        let err = Config::load_from(Some(Path::new("/nonexistent/hush.toml"))).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn save_round_trips_and_omits_unset_model_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/config.toml");
        let mut config = Config::defaults().unwrap();
        config.hotkey.combination = "Ctrl+Alt+Space".into();
        config.save_to_file(&path).unwrap();

        let written = std::fs::read_to_string(&path).unwrap();
        assert!(!written.contains("model_path"));

        let reloaded = Config::load_from(Some(&path)).unwrap();
        assert_eq!(reloaded.hotkey.combination, "Ctrl+Alt+Space");
    }

    #[test]
    fn gesture_cap_and_profile_defaults() {
        let config = Config::defaults().unwrap();
        assert_eq!(config.hotkey.cancel, "Escape");
        assert_eq!(config.hotkey.tap_ms, 300);
        assert_eq!(config.audio.max_recording_secs, 600);
        assert!(config.feedback.audio_enabled);
        assert!(config.transcription.context_prompt);
        assert!(config.profiles.enabled);
        assert_eq!(config.hotkey.command, "Ctrl+RightAlt");
        assert_eq!(config.llm.provider, LlmSetting::Auto);
        assert_eq!(config.llm.ollama_model, "llama3.2");
        let config =
            Config::parse_over_defaults("[llm]\nprovider = \"none\"\npolish = false\n").unwrap();
        assert_eq!(config.llm.provider, LlmSetting::None);
        assert!(!config.llm.polish);
        assert!(Config::parse_over_defaults("[hotkey]\ntap_ms = 5000\n").is_err());
        let config = Config::parse_over_defaults("[profiles]\nchat = [\"myapp\"]\n").unwrap();
        assert_eq!(config.profiles.chat, vec!["myapp".to_string()]);
        assert!(config.profiles.enabled);
    }
}
