use clap::{Parser, Subcommand};
/// Simple model manager for downloading and managing GGML whisper models
use hush::{
    model_downloader::{ModelDownloader, WhisperModel},
    Result,
};
use std::path::PathBuf;
use tracing::{error, info};

#[derive(Parser)]
#[command(name = "simple-model-manager")]
#[command(about = "A simple tool for managing GGML Whisper models")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all models and their status
    List,
    /// Download a specific model
    Download {
        /// Model to download (tiny, base, small, medium, large)
        #[arg(value_enum)]
        model: ModelType,
    },
    /// Show information about models directory
    Info,
}

#[derive(clap::ValueEnum, Clone)]
enum ModelType {
    Tiny,
    Base,
    Small,
    Medium,
    Large,
}

impl From<ModelType> for WhisperModel {
    fn from(model_type: ModelType) -> Self {
        match model_type {
            ModelType::Tiny => WhisperModel::Tiny,
            ModelType::Base => WhisperModel::Base,
            ModelType::Small => WhisperModel::Small,
            ModelType::Medium => WhisperModel::Medium,
            ModelType::Large => WhisperModel::Large,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    let cli = Cli::parse();

    // Set up models directory
    let models_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hush")
        .join("models");

    let downloader = ModelDownloader::new(&models_dir);

    match cli.command {
        Commands::List => {
            info!("📋 Whisper Models Status");
            info!("========================");
            info!("Models directory: {:?}", models_dir);
            info!("");

            let models = downloader.list_models().await;
            for (model, exists) in &models {
                let status = if *exists {
                    "✅ Downloaded"
                } else {
                    "❌ Not available"
                };
                let size_info = match model {
                    WhisperModel::Tiny => " (~39 MB)",
                    WhisperModel::Base => " (~142 MB)",
                    WhisperModel::Small => " (~466 MB)",
                    WhisperModel::Medium => " (~1.5 GB)",
                    WhisperModel::Large => " (~2.9 GB)",
                };

                info!(
                    "  {:<20} {} {}",
                    format!("{}{}", model.filename(), size_info),
                    status,
                    if *exists {
                        ""
                    } else {
                        " - Use 'download' command to get it"
                    }
                );
            }

            if let Some(best_model) = downloader.find_best_available_model().await {
                info!("");
                info!("🎯 Best available model: {}", best_model.filename());
            } else {
                info!("");
                info!("⚠️ No models are currently downloaded.");
                info!("Use 'simple-model-manager download base' to get started.");
            }
        },

        Commands::Download { model } => {
            let whisper_model = WhisperModel::from(model);
            info!("📥 Downloading model: {}", whisper_model.filename());
            info!("Description: {}", whisper_model.description());
            info!("");

            match downloader.ensure_model(&whisper_model).await {
                Ok(model_path) => {
                    info!("✅ Model downloaded successfully!");
                    info!("Location: {:?}", model_path);

                    // Check file size
                    if let Ok(metadata) = std::fs::metadata(&model_path) {
                        let size_mb = metadata.len() as f64 / 1_048_576.0;
                        info!("File size: {:.1} MB", size_mb);
                    }
                },
                Err(e) => {
                    error!("❌ Failed to download model: {}", e);
                    std::process::exit(1);
                },
            }
        },

        Commands::Info => {
            info!("📁 Hush Models Information");
            info!("==========================");
            info!("Models directory: {:?}", models_dir);
            info!("Directory exists: {}", models_dir.exists());

            if models_dir.exists() {
                // Calculate total size of all models
                let mut total_size = 0u64;
                let mut file_count = 0u64;

                if let Ok(entries) = std::fs::read_dir(&models_dir) {
                    for entry in entries.flatten() {
                        if let Ok(metadata) = entry.metadata() {
                            if metadata.is_file() {
                                total_size += metadata.len();
                                file_count += 1;
                            }
                        }
                    }
                }

                info!("Total files: {}", file_count);
                info!("Total size: {:.1} MB", total_size as f64 / 1_048_576.0);
            } else {
                info!("Models directory will be created when first model is downloaded.");
            }

            info!("");
            info!("💡 Quick start:");
            info!("  1. Download a model: simple-model-manager download base");
            info!("  2. Test transcription: cargo run --bin test-simple-whisper");
        },
    }

    Ok(())
}
