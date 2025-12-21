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
/// Models are stored in `~/.cache/hush/models/`
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
    use crate::Config;

    println!("📋 Available Whisper Models:");
    println!("   (Only one model can be cached at a time)");
    println!();

    // Load config to show which model is active
    let active_model = Config::load()
        .ok()
        .map(|config| config.transcription.model_size);

    let models = [
        "tiny", "base", "small", "medium", "large", "large-v2", "large-v3",
    ];
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("hush")
        .join("models");

    // Check if the model file exists
    let model_file_exists = cache_dir.join("model.safetensors").exists();

    for model in models {
        let is_active = active_model.as_deref() == Some(model);
        // Model is "downloaded" if it's active AND the file exists
        let is_downloaded = is_active && model_file_exists;

        if downloaded && !is_downloaded {
            continue;
        }

        let status = if is_downloaded {
            "✅"
        } else if is_active && !model_file_exists {
            "⚠️ " // Active but file missing
        } else {
            "  "
        };
        print!("{} {}", status, model);

        // Show if this is the active model
        if is_active {
            if model_file_exists {
                print!(" ← active");
            } else {
                print!(" ← active (not downloaded)");
            }
        }

        if details {
            let size = match model {
                "tiny" => "~151 MB",
                "base" => "~290 MB",
                "small" => "~967 MB",
                "medium" => "~3 GB",
                "large" => "~6.2 GB",
                "large-v2" => "~6.2 GB",
                "large-v3" => "~6.2 GB",
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

    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("hush")
        .join("models");

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
    config.transcription.model_path = model_path;

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

    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("hush")
        .join("models");

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
/// - Currently cached model
/// - Disk space used
///
/// # Arguments
///
/// * `clear_stats` - If true, clear cache statistics (not yet implemented)
async fn show_model_info(clear_stats: bool) -> Result<()> {
    use crate::Config;

    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("hush")
        .join("models");

    println!("📊 Model Cache Information:");
    println!("Cache directory: {}", cache_dir.display());

    // Get active model from config
    let active_model = Config::load()
        .ok()
        .map(|config| config.transcription.model_size);

    let model_path = cache_dir.join("model.safetensors");
    if model_path.exists() {
        let metadata = std::fs::metadata(&model_path)?;
        let size_mb = metadata.len() / (1024 * 1024);
        println!(
            "Cached model: {} ({} MB)",
            active_model.as_deref().unwrap_or("unknown"),
            size_mb
        );
    } else {
        println!("Cached model: none");
    }

    if clear_stats {
        println!("🚧 Statistics clearing not yet implemented");
    }

    Ok(())
}

/// Verify model integrity
///
/// Checks the integrity of the cached model by validating its file size
/// against expected values. Since only one model can be cached at a time,
/// this verifies the currently active model.
///
/// # Arguments
///
/// * `model_size` - Ignored (only the active model can be verified)
/// * `fix` - If true, re-download corrupted model
///
/// # Integrity Checks
///
/// - Verifies file exists
/// - Checks file size is within expected range (±10%)
async fn verify_models(model_size: Option<&str>, fix: bool) -> Result<()> {
    use crate::Config;

    println!("🔍 Verifying model integrity...");

    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("hush")
        .join("models");

    // Get active model from config
    let active_model = Config::load()
        .ok()
        .map(|config| config.transcription.model_size);

    // If user specified a model, check if it matches the active one
    if let Some(requested) = model_size {
        if active_model.as_deref() != Some(requested) {
            println!(
                "ℹ️ Model '{}' is not the active model (active: {})",
                requested,
                active_model.as_deref().unwrap_or("none")
            );
            println!("   Only the active model is cached.");
            return Ok(());
        }
    }

    let model_path = cache_dir.join("model.safetensors");

    // Expected sizes for safetensors whisper models (in MB)
    let expected_sizes: std::collections::HashMap<&str, u64> = [
        ("tiny", 151),
        ("base", 290),
        ("small", 967),
        ("medium", 3055),
        ("large", 6173),
        ("large-v2", 6173),
        ("large-v3", 6173),
    ]
    .iter()
    .cloned()
    .collect();

    if model_path.exists() {
        let model_name = active_model.as_deref().unwrap_or("unknown");
        let metadata = std::fs::metadata(&model_path)?;
        let size_mb = metadata.len() / (1024 * 1024);

        if let Some(expected_size) = expected_sizes.get(model_name) {
            // Allow 10% variance in file size
            let min_size = expected_size * 9 / 10;
            let max_size = expected_size * 11 / 10;

            if size_mb >= min_size && size_mb <= max_size {
                println!("✅ {} - Present ({} MB, size OK)", model_name, size_mb);
            } else {
                println!(
                    "⚠️  {} - Present ({} MB, expected ~{} MB - may be corrupted)",
                    model_name, size_mb, expected_size
                );
                if fix {
                    println!("🔄 Re-downloading model...");
                    download_model(model_name, true).await?;
                    println!("✅ Model re-downloaded");
                }
            }
        } else {
            println!("✅ {} - Present ({} MB)", model_name, size_mb);
        }
    } else {
        let model_name = active_model.as_deref().unwrap_or("none");
        println!("❌ {} - Not downloaded", model_name);
        if fix && active_model.is_some() {
            println!("🔄 Downloading model...");
            download_model(active_model.as_deref().unwrap(), false).await?;
            println!("✅ Model downloaded");
        }
    }

    Ok(())
}
