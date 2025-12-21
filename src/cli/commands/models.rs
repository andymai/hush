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
        ModelCommands::Download { model_size, force } => download_model(&model_size, force).await,
        ModelCommands::Remove { model_size, yes } => remove_model(&model_size, yes).await,
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
    // Use existing simple-model-manager functionality
    println!("📋 Available Whisper Models:");

    let models = ["tiny", "base", "small", "medium", "large"];
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("hush")
        .join("models");

    for model in models {
        let model_path = cache_dir.join(format!("ggml-{}.bin", model));
        let is_downloaded = model_path.exists();

        if downloaded && !is_downloaded {
            continue;
        }

        let status = if is_downloaded { "✅" } else { "📥" };
        print!("{} {} ", status, model);

        if details {
            let size = match model {
                "tiny" => "~39 MB",
                "base" => "~142 MB",
                "small" => "~466 MB",
                "medium" => "~1.5 GB",
                "large" => "~2.9 GB",
                _ => "Unknown",
            };
            print!("({})", size);
        }

        if is_downloaded && details {
            if let Ok(metadata) = std::fs::metadata(&model_path) {
                print!(" - {} MB", metadata.len() / 1024 / 1024);
            }
        }

        println!();
    }

    Ok(())
}

/// Download a Whisper model
pub async fn download_model(model_size: &str, force: bool) -> Result<()> {
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
        let model_path = cache_dir.join(format!("ggml-{}.bin", model_size));
        if model_path.exists() {
            std::fs::remove_file(&model_path).ok();
        }
    }

    let manager = ModelManager::new(&cache_dir)?;
    match manager.ensure_model_downloaded(&size).await {
        Ok(path) => {
            println!("✅ Model '{}' downloaded to {:?}", model_size, path);
            Ok(())
        },
        Err(e) => {
            error!("❌ Failed to download model '{}': {}", model_size, e);
            Err(e)
        },
    }
}

/// Remove a Whisper model from the cache
///
/// Deletes the specified model file from the local cache directory.
/// Can remove individual models or all models at once.
///
/// # Arguments
///
/// * `model_size` - Model name to remove, or "all" to remove all models
/// * `yes` - If true, skip confirmation prompt
///
/// # Confirmation
///
/// Unless `yes` is true, the user will be prompted to confirm deletion.
async fn remove_model(model_size: &str, yes: bool) -> Result<()> {
    if model_size == "all" {
        println!("🗑️ Removing all models...");
        if !yes {
            println!("Are you sure you want to remove all models? (y/N)");
            let mut input = String::new();
            std::io::stdin()
                .read_line(&mut input)
                .context("Failed to read user input")?;
            if !input.trim().to_lowercase().starts_with('y') {
                println!("Cancelled.");
                return Ok(());
            }
        }

        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("hush")
            .join("models");

        if cache_dir.exists() {
            std::fs::remove_dir_all(&cache_dir)?;
            println!("✅ All models removed");
        } else {
            println!("ℹ️ No models found to remove");
        }
    } else {
        println!("🗑️ Removing model: {}", model_size);

        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("hush")
            .join("models");
        let model_path = cache_dir.join(format!("ggml-{}.bin", model_size));

        if model_path.exists() {
            if !yes {
                println!("Are you sure you want to remove '{}'? (y/N)", model_size);
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
            println!("✅ Model '{}' removed", model_size);
        } else {
            println!("ℹ️ Model '{}' not found", model_size);
        }
    }

    Ok(())
}

/// Show model cache information
///
/// Displays information about the model cache directory including:
/// - Cache directory path
/// - Number of downloaded models
/// - Total disk space used
///
/// # Arguments
///
/// * `clear_stats` - If true, clear cache statistics (not yet implemented)
async fn show_model_info(clear_stats: bool) -> Result<()> {
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("hush")
        .join("models");

    println!("📊 Model Cache Information:");
    println!("Cache directory: {}", cache_dir.display());

    if cache_dir.exists() {
        let mut total_size = 0u64;
        let mut model_count = 0;

        for entry in std::fs::read_dir(&cache_dir)? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().starts_with("ggml-") {
                let metadata = entry.metadata()?;
                total_size += metadata.len();
                model_count += 1;
            }
        }

        println!("Models downloaded: {}", model_count);
        println!("Total size: {} MB", total_size / 1024 / 1024);
    } else {
        println!("No models downloaded yet.");
    }

    if clear_stats {
        println!("🚧 Statistics clearing not yet implemented");
    }

    Ok(())
}

/// Verify model integrity
///
/// Checks the integrity of downloaded models by validating their file sizes
/// against expected values. Allows a 10% variance in file size.
///
/// # Arguments
///
/// * `model_size` - Specific model to verify, or None to verify all models
/// * `fix` - If true, re-download corrupted models (not yet implemented)
///
/// # Integrity Checks
///
/// - Verifies file exists
/// - Checks file size is within expected range (±10%)
/// - Future: Could add SHA256 checksum verification
async fn verify_models(model_size: Option<&str>, fix: bool) -> Result<()> {
    println!("🔍 Verifying model integrity...");

    if fix {
        println!("🚧 Model fixing not yet implemented");
    }

    // Basic file existence check
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("hush")
        .join("models");

    let models_to_check = if let Some(size) = model_size {
        vec![size.to_string()]
    } else {
        vec![
            "tiny".to_string(),
            "base".to_string(),
            "small".to_string(),
            "medium".to_string(),
            "large".to_string(),
        ]
    };

    // Expected sizes for whisper models (approximate, in MB)
    let expected_sizes: std::collections::HashMap<&str, u64> = [
        ("tiny", 75),
        ("tiny.en", 75),
        ("base", 145),
        ("base.en", 145),
        ("small", 466),
        ("small.en", 466),
        ("medium", 1500),
        ("medium.en", 1500),
        ("large", 2900),
    ]
    .iter()
    .cloned()
    .collect();

    for model in models_to_check {
        let model_path = cache_dir.join(format!("ggml-{}.bin", model));
        if model_path.exists() {
            // Verify file size as basic integrity check
            let metadata = std::fs::metadata(&model_path)?;
            let size_mb = metadata.len() / (1024 * 1024);

            if let Some(expected_size) = expected_sizes.get(model.as_str()) {
                // Allow 10% variance in file size
                let min_size = expected_size * 9 / 10;
                let max_size = expected_size * 11 / 10;

                if size_mb >= min_size && size_mb <= max_size {
                    println!("✅ {} - Present ({} MB, size OK)", model, size_mb);
                } else {
                    println!(
                        "⚠️  {} - Present ({} MB, expected ~{} MB - may be corrupted)",
                        model, size_mb, expected_size
                    );
                }
            } else {
                println!("✅ {} - Present ({} MB, unknown model)", model, size_mb);
            }

            // Note: For production use, consider adding SHA256 checksum verification
            // by adding the `sha2` crate and storing known checksums
        } else {
            println!("❌ {} - Missing", model);
        }
    }

    Ok(())
}
