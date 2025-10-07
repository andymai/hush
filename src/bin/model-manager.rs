use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use hush::transcription::models::{ModelManager, ModelSize};
use std::path::PathBuf;
use tracing::{info, warn};
use tracing_subscriber;

#[derive(Parser)]
#[command(name = "model-manager")]
#[command(about = "Hush Whisper Model Manager")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    /// Model cache directory
    #[arg(long, default_value = "./models")]
    cache_dir: PathBuf,
    
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// List available models
    List,
    /// Download a specific model
    Download {
        /// Model size to download
        #[arg(value_parser = parse_model_size)]
        model: ModelSize,
    },
    /// Show cache information
    Info,
    /// Clear model cache
    Clear {
        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
}

fn parse_model_size(s: &str) -> Result<ModelSize, String> {
    ModelSize::from_str(s).map_err(|e| e.to_string())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize logging
    let level = if cli.verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    
    tracing_subscriber::fmt()
        .with_max_level(level)
        .init();

    let manager = ModelManager::new(&cli.cache_dir)
        .context("Failed to initialize model manager")?;

    match cli.command {
        Commands::List => list_models(&manager).await?,
        Commands::Download { model } => download_model(&manager, model).await?,
        Commands::Info => show_info(&manager).await?,
        Commands::Clear { yes } => clear_cache(&manager, yes).await?,
    }

    Ok(())
}

async fn list_models(manager: &ModelManager) -> Result<()> {
    println!("Available Whisper models:");
    println!();

    let available = manager.list_available_models();
    let cached = manager.list_cached_models();
    
    for model_info in &available {
        let is_cached = cached.contains(&model_info.size);
        let status = if is_cached { "✓ cached" } else { "  not cached" };
        let size_str = ModelManager::format_size(model_info.expected_size);
        
        println!(
            "  {} {} - {} ({})", 
            status,
            model_info.size.to_string(),
            model_info.name,
            size_str
        );
    }
    
    println!();
    let total_cached = cached.len();
    let total_available = available.len();
    println!("Cached: {}/{} models", total_cached, total_available);

    Ok(())
}

async fn download_model(manager: &ModelManager, model: ModelSize) -> Result<()> {
    let model_info = manager.get_model_info(&model)
        .ok_or_else(|| anyhow::anyhow!("Unknown model: {:?}", model))?;
    
    println!("Downloading {} ({})...", 
             model_info.name, 
             ModelManager::format_size(model_info.expected_size));
    
    let path = manager.ensure_model_downloaded(&model).await
        .context("Failed to download model")?;
    
    println!("✓ Model downloaded successfully to: {}", path.display());
    Ok(())
}

async fn show_info(manager: &ModelManager) -> Result<()> {
    let cache_size = manager.get_cache_size()
        .context("Failed to calculate cache size")?;
    
    let cached_models = manager.list_cached_models();
    
    println!("Model Cache Information:");
    println!();
    println!("Cache directory: {}", manager.cache_dir.display());
    println!("Total cache size: {}", ModelManager::format_size(cache_size));
    println!("Cached models: {}", cached_models.len());
    
    if !cached_models.is_empty() {
        println!();
        println!("Cached models:");
        for model_size in &cached_models {
            if let Some(info) = manager.get_model_info(model_size) {
                let path = manager.get_model_path(model_size).unwrap();
                let file_size = std::fs::metadata(&path)
                    .map(|m| ModelManager::format_size(m.len()))
                    .unwrap_or_else(|_| "unknown".to_string());
                
                println!("  {} - {}", info.name, file_size);
            }
        }
    }
    
    Ok(())
}

async fn clear_cache(manager: &ModelManager, skip_confirm: bool) -> Result<()> {
    let cache_size = manager.get_cache_size()?;
    
    if cache_size == 0 {
        println!("Cache is already empty.");
        return Ok(());
    }
    
    if !skip_confirm {
        println!("This will delete {} of cached models.", 
                 ModelManager::format_size(cache_size));
        println!("Are you sure? (y/N)");
        
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        
        if !input.trim().to_lowercase().starts_with('y') {
            println!("Cancelled.");
            return Ok(());
        }
    }
    
    manager.clear_cache().await
        .context("Failed to clear cache")?;
    
    println!("✓ Cache cleared successfully.");
    Ok(())
}

