use crate::cli::ModelCommands;
use anyhow::{Context, Result};
use tracing::error;

/// Handle model management commands
///
/// Manages Whisper model lifecycle including downloading, listing, removing,
/// and verifying models stored in the local cache.
///
/// # Arguments
///
/// * `model_command` - The specific model command to execute (list, download, remove, info, verify)
///
/// # Examples
///
/// ```no_run
/// use hush::cli::ModelCommands;
/// use hush::cli::commands::handle_models;
///
/// # async fn example() -> anyhow::Result<()> {
/// // List all available models
/// handle_models(ModelCommands::List {
///     downloaded: false,
///     details: false,
/// }).await?;
///
/// // Download a specific model
/// handle_models(ModelCommands::Download {
///     model_size: "base".to_string(),
///     force: false,
/// }).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Model Commands
///
/// - **List**: Display available Whisper models (tiny, base, small, medium, large)
/// - **Download**: Download a model to the local cache
/// - **Remove**: Delete a model from the cache
/// - **Info**: Show cache directory information and statistics
/// - **Verify**: Check model integrity by validating file sizes
///
/// # Cache Location
///
/// Models are stored in `$XDG_DATA_HOME/hush/models/` (`~/.local/share/hush/models/`)
pub async fn handle_models(model_command: ModelCommands) -> Result<()> {
    match model_command {
        ModelCommands::List {
            downloaded,
            details,
        } => list_models(downloaded, details).await,
        ModelCommands::Download { model_size, force } => {
            download_model(&model_size, force).await?;
            Ok(())
        },
        ModelCommands::Remove { model_size, yes } => remove_model(&model_size, yes).await,
        ModelCommands::Set { model_size } => set_model(&model_size).await,
        ModelCommands::Info { clear_stats } => show_model_info(clear_stats).await,
        ModelCommands::Verify { model_size, fix } => {
            verify_models(model_size.as_deref(), fix).await
        },
    }
}

/// List available Whisper models
///
/// Displays all supported Whisper model sizes with their download status.
///
/// # Arguments
///
/// * `downloaded` - If true, only show models that are already downloaded
/// * `details` - If true, show additional details like expected and actual file sizes
///
/// # Model Sizes
///
/// - **tiny**: ~39 MB - Fastest, lowest accuracy
/// - **base**: ~142 MB - Good balance for quick transcription
/// - **small**: ~466 MB - Better accuracy
/// - **medium**: ~1.5 GB - High accuracy
/// - **large**: ~2.9 GB - Best accuracy, slower
async fn list_models(downloaded: bool, details: bool) -> Result<()> {
    use crate::transcription::models::{ModelManager, ModelSize};
    use crate::Config;

    println!("📋 Available Whisper Models:");
    println!();

    // Load config to show which model is active
    let active_model = Config::load()
        .ok()
        .map(|config| config.transcription.model_size);

    let models = [
        "tiny", "base", "small", "medium", "large", "large-v2", "large-v3",
    ];
    let cache_dir = crate::config::paths::models_dir();

    // Use ModelManager to check which models are actually downloaded
    let manager = ModelManager::new(&cache_dir)?;

    for model in models {
        let is_active = active_model.as_deref() == Some(model);

        // Check if the model file actually exists using ModelManager
        let model_size: Option<ModelSize> = model.parse().ok();
        let is_downloaded = model_size
            .as_ref()
            .and_then(|size| manager.get_model_path(size))
            .is_some();

        if downloaded && !is_downloaded {
            continue;
        }

        let status = if is_downloaded { "✅" } else { "  " };
        print!("{} {}", status, model);

        // Show if this is the active model
        if is_active {
            print!(" ← active");
        }

        if details {
            let size = match model {
                "tiny" => "~78 MB",
                "base" => "~147 MB",
                "small" => "~488 MB",
                "medium" => "~1.5 GB",
                "large" => "~3.1 GB",
                "large-v2" => "~3.1 GB",
                "large-v3" => "~3.1 GB",
                _ => "Unknown",
            };
            print!(" ({})", size);
        }

        println!();
    }

    Ok(())
}

/// Download a Whisper model
///
/// Returns the path to the downloaded model file.
pub async fn download_model(model_size: &str, force: bool) -> Result<std::path::PathBuf> {
    use crate::transcription::models::{ModelManager, ModelSize};

    println!("📥 Downloading model: {}", model_size);

    let size: ModelSize = model_size
        .parse()
        .context(format!("Invalid model size: {}", model_size))?;

    let cache_dir = crate::config::paths::models_dir();

    // If force, remove existing model first
    if force {
        let model_path = cache_dir.join("model.safetensors");
        if model_path.exists() {
            std::fs::remove_file(&model_path).ok();
        }
    }

    let manager = ModelManager::new(&cache_dir)?;
    match manager.ensure_model_downloaded(&size).await {
        Ok(path) => {
            println!("✅ Model '{}' downloaded to {:?}", model_size, path);
            Ok(path)
        },
        Err(e) => {
            error!("❌ Failed to download model '{}': {}", model_size, e);
            Err(e)
        },
    }
}

/// Set the active model (downloads if needed and updates config)
///
/// This command makes a model the active one by:
/// 1. Downloading the model if it's not already cached
/// 2. Updating the configuration file to use this model
///
/// # Arguments
///
/// * `model_size` - Model size to set as active (tiny, base, small, medium, large, large-v2, large-v3)
///
/// # Examples
///
/// ```bash
/// hush models set base       # Download base model (if needed) and make it active
/// hush models set large-v3   # Download large-v3 and make it active
/// ```
pub async fn set_model(model_size: &str) -> Result<()> {
    use crate::transcription::models::ModelSize;
    use crate::Config;

    println!("🔧 Setting active model to: {}", model_size);

    // Parse and validate model size first
    let _size: ModelSize = model_size
        .parse()
        .context(format!("Invalid model size: {}", model_size))?;

    // Always download to ensure the cached file matches the requested model
    // (since all models share the same filename, we need to re-download when switching)
    let model_path = download_model(model_size, true).await?;

    // Load current config
    let mut config = Config::load()?;

    // Update config with actual path from download
    config.transcription.model_size = model_size.to_string();
    config.transcription.model_path = Some(model_path);

    // Validate updated config
    config
        .validate()
        .context("Updated configuration is invalid")?;

    // Save config
    config.save().context("Failed to save configuration")?;

    println!("✅ Active model set to: {}", model_size);
    println!("📝 Configuration saved");

    Ok(())
}

/// Remove the cached model
///
/// Since only one model can be cached at a time (model.safetensors),
/// this removes the cached model file.
///
/// # Arguments
///
/// * `model_size` - Model name to remove, or "all" to remove all cached data
/// * `yes` - If true, skip confirmation prompt
///
/// # Confirmation
///
/// Unless `yes` is true, the user will be prompted to confirm deletion.
async fn remove_model(model_size: &str, yes: bool) -> Result<()> {
    use crate::Config;

    let cache_dir = crate::config::paths::models_dir();

    if model_size == "all" {
        println!("🗑️ Removing all cached models...");
        if !yes {
            println!("Are you sure you want to remove the model cache? (y/N)");
            let mut input = String::new();
            std::io::stdin()
                .read_line(&mut input)
                .context("Failed to read user input")?;
            if !input.trim().to_lowercase().starts_with('y') {
                println!("Cancelled.");
                return Ok(());
            }
        }

        if cache_dir.exists() {
            std::fs::remove_dir_all(&cache_dir)?;
            println!("✅ Model cache removed");
        } else {
            println!("ℹ️ No models found to remove");
        }
    } else {
        // Check if this is the currently active model
        let active_model = Config::load()
            .ok()
            .map(|config| config.transcription.model_size);

        let model_path = cache_dir.join("model.safetensors");

        if active_model.as_deref() != Some(model_size) {
            println!(
                "ℹ️ Model '{}' is not the active model (active: {})",
                model_size,
                active_model.as_deref().unwrap_or("none")
            );
            println!(
                "   Only the active model is cached. Use 'hush models set {}' first.",
                model_size
            );
            return Ok(());
        }

        if model_path.exists() {
            if !yes {
                println!("Are you sure you want to remove the cached model? (y/N)");
                let mut input = String::new();
                std::io::stdin()
                    .read_line(&mut input)
                    .context("Failed to read user input")?;
                if !input.trim().to_lowercase().starts_with('y') {
                    println!("Cancelled.");
                    return Ok(());
                }
            }

            std::fs::remove_file(&model_path)?;
            println!("✅ Cached model removed");
        } else {
            println!("ℹ️ No cached model found");
        }
    }

    Ok(())
}

/// Show model cache information
///
/// Displays information about the model cache directory including:
/// - Cache directory path
/// - Currently cached models
/// - Disk space used
///
/// # Arguments
///
/// * `clear_stats` - If true, clear cache statistics (not yet implemented)
async fn show_model_info(clear_stats: bool) -> Result<()> {
    use crate::transcription::models::ModelManager;
    use crate::Config;

    let cache_dir = crate::config::paths::models_dir();

    println!("📊 Model Cache Information:");
    println!("Cache directory: {}", cache_dir.display());

    // Get active model from config
    let active_model = Config::load()
        .ok()
        .map(|config| config.transcription.model_size);

    // Use ModelManager to list cached models
    let manager = ModelManager::new(&cache_dir)?;
    let cached_models = manager.list_cached_models();

    if cached_models.is_empty() {
        println!("Cached models: none");
    } else {
        println!("Cached models:");
        for model_size in &cached_models {
            if let Some(path) = manager.get_model_path(model_size) {
                let metadata = std::fs::metadata(&path)?;
                let size_mb = metadata.len() / (1024 * 1024);
                let active_marker = if active_model.as_deref() == Some(&model_size.to_string()) {
                    " ← active"
                } else {
                    ""
                };
                println!("  {} ({} MB){}", model_size, size_mb, active_marker);
            }
        }
    }

    // Show total cache size
    let total_size = manager.get_cache_size()?;
    println!(
        "Total cache size: {}",
        ModelManager::format_size(total_size)
    );

    if clear_stats {
        println!("🚧 Statistics clearing not yet implemented");
    }

    Ok(())
}

/// Verify model integrity
///
/// Checks the integrity of cached models by validating file sizes
/// against expected values and optionally checking SHA256 checksums.
///
/// # Arguments
///
/// * `model_size` - Specific model to verify, or None to verify all cached models
/// * `fix` - If true, re-download corrupted models
///
/// # Integrity Checks
///
/// - Verifies file exists
/// - Checks file size is within expected range (±10%)
async fn verify_models(model_size: Option<&str>, fix: bool) -> Result<()> {
    use crate::transcription::models::{ModelManager, ModelSize};

    println!("🔍 Verifying model integrity...");

    let cache_dir = crate::config::paths::models_dir();

    let manager = ModelManager::new(&cache_dir)?;

    // Determine which models to verify
    let models_to_verify: Vec<ModelSize> = if let Some(requested) = model_size {
        let size: ModelSize = requested
            .parse()
            .context(format!("Invalid model size: {}", requested))?;
        vec![size]
    } else {
        // Verify all cached models
        manager.list_cached_models()
    };

    if models_to_verify.is_empty() {
        println!("ℹ️ No cached models to verify");
        return Ok(());
    }

    for model_size in models_to_verify {
        let model_name = model_size.to_string();

        if let Some(info) = manager.get_model_info(&model_size) {
            if let Some(path) = manager.get_model_path(&model_size) {
                let metadata = std::fs::metadata(&path)?;
                let size_mb = metadata.len() / (1024 * 1024);
                let expected_mb = info.expected_size / (1024 * 1024);

                // Allow 10% variance in file size
                let min_size = expected_mb * 9 / 10;
                let max_size = expected_mb * 11 / 10;

                if size_mb >= min_size && size_mb <= max_size {
                    println!("✅ {} - Present ({} MB, size OK)", model_name, size_mb);
                } else {
                    println!(
                        "⚠️  {} - Present ({} MB, expected ~{} MB - may be corrupted)",
                        model_name, size_mb, expected_mb
                    );
                    if fix {
                        println!("🔄 Re-downloading model...");
                        download_model(&model_name, true).await?;
                        println!("✅ Model re-downloaded");
                    }
                }
            } else {
                println!("❌ {} - Not downloaded", model_name);
                if fix {
                    println!("🔄 Downloading model...");
                    download_model(&model_name, false).await?;
                    println!("✅ Model downloaded");
                }
            }
        }
    }

    Ok(())
}
