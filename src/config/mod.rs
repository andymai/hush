//! Configuration loading and filesystem paths.
//!
//! [`Config::load`] reads the user file named by `--config-file`, then `HUSH_CONFIG`,
//! then `$XDG_CONFIG_HOME/hush/config.toml`, and merges it over the compiled-in
//! defaults from `config/default.toml`. A user file only needs the keys it
//! changes; a missing file means defaults.
//!
//! ```no_run
//! use hush::config::Config;
//!
//! let config = Config::load()?;
//! println!("Model file: {}", config.transcription.model_path().display());
//! # Ok::<(), anyhow::Error>(())
//! ```

pub mod paths;
pub mod settings;

pub use settings::Config;
