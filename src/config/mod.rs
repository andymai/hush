/// Configuration management with hot-reload support
///
/// This module handles loading, parsing, and watching configuration files
/// for the Hush application. Supports TOML format with automatic reload.
///
/// # Components
///
/// - [`Config`] - Application configuration structure
/// - [`ConfigWatcher`] - Monitors config file for changes and reloads automatically
///
/// # Configuration File
///
/// Default location: `~/.config/hush/config.toml`
///
/// Example configuration:
///
/// ```toml
/// [audio]
/// sample_rate = 16000
/// device = "default"
///
/// [transcription]
/// model_size = "base"
/// use_gpu = true
///
/// [hotkeys]
/// record = "Ctrl+Shift+Space"
///
/// [text_processing]
/// editing_mode = "medium"
/// enable_llm = false
/// ```
///
/// # Examples
///
/// ```no_run
/// use hush::config::Config;
///
/// // Load configuration
/// let config = Config::load().expect("Failed to load config");
///
/// println!("Model size: {:?}", config.transcription.model_size);
/// println!("Sample rate: {}", config.audio.sample_rate);
/// ```
///
/// # Hot Reload
///
/// Use `ConfigWatcher` to automatically reload when config file changes:
///
/// ```no_run
/// use hush::config::ConfigWatcher;
///
/// let watcher = ConfigWatcher::new().expect("Failed to create watcher");
///
/// // Check for config updates
/// if let Some(new_config) = watcher.check_for_updates() {
///     println!("Config reloaded!");
/// }
/// ```
pub mod settings;

pub use settings::{Config, ConfigWatcher};
