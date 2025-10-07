/// Utility to download GGML Whisper models compatible with whisper-rs
use crate::Result;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{info, warn, error};

/// Available Whisper models
#[derive(Debug, Clone)]
pub enum WhisperModel {
    Tiny,
    Base,
    Small,
    Medium,
    Large,
}

impl WhisperModel {
    pub fn filename(&self) -> &'static str {
        match self {
            Self::Tiny => "ggml-tiny.bin",
            Self::Base => "ggml-base.bin", 
            Self::Small => "ggml-small.bin",
            Self::Medium => "ggml-medium.bin",
            Self::Large => "ggml-large-v3.bin",
        }
    }
    
    pub fn download_url(&self) -> &'static str {
        match self {
            Self::Tiny => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
            Self::Base => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
            Self::Small => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin", 
            Self::Medium => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin",
            Self::Large => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin",
        }
    }
    
    pub fn description(&self) -> &'static str {
        match self {
            Self::Tiny => "Tiny model (~39 MB) - Fastest, lowest accuracy",
            Self::Base => "Base model (~142 MB) - Good balance of speed/accuracy", 
            Self::Small => "Small model (~466 MB) - Better accuracy",
            Self::Medium => "Medium model (~1.5 GB) - High accuracy",
            Self::Large => "Large model (~2.9 GB) - Highest accuracy",
        }
    }
}

pub struct ModelDownloader {
    models_dir: PathBuf,
}

impl ModelDownloader {
    pub fn new<P: AsRef<Path>>(models_dir: P) -> Self {
        Self {
            models_dir: models_dir.as_ref().to_path_buf(),
        }
    }
    
    /// Get the path where a model would be stored
    pub fn model_path(&self, model: &WhisperModel) -> PathBuf {
        self.models_dir.join(model.filename())
    }
    
    /// Check if a model exists locally
    pub async fn model_exists(&self, model: &WhisperModel) -> bool {
        self.model_path(model).exists()
    }
    
    /// Find the best available model (largest that exists locally)
    pub async fn find_best_available_model(&self) -> Option<WhisperModel> {
        let models = [
            WhisperModel::Large,
            WhisperModel::Medium, 
            WhisperModel::Small,
            WhisperModel::Base,
            WhisperModel::Tiny,
        ];
        
        for model in &models {
            if self.model_exists(model).await {
                return Some(model.clone());
            }
        }
        
        None
    }
    
    /// Download a model if it doesn't exist
    pub async fn ensure_model(&self, model: &WhisperModel) -> Result<PathBuf> {
        let model_path = self.model_path(model);
        
        if model_path.exists() {
            info!("✅ Model {} already exists at {:?}", model.filename(), model_path);
            return Ok(model_path);
        }
        
        info!("📥 Downloading {} model...", model.filename());
        info!("📊 {}", model.description());
        
        // Create models directory if it doesn't exist
        fs::create_dir_all(&self.models_dir).await
            .map_err(|e| anyhow::anyhow!("Failed to create models directory: {}", e))?;
        
        // Download the model
        self.download_model(model).await?;
        
        Ok(model_path)
    }
    
    /// Download a model from the internet
    async fn download_model(&self, model: &WhisperModel) -> Result<()> {
        let url = model.download_url();
        let model_path = self.model_path(model);
        let temp_path = model_path.with_extension("tmp");
        
        info!("🌐 Downloading from: {}", url);
        info!("💾 Saving to: {:?}", model_path);
        
        // Create HTTP client
        let client = reqwest::Client::new();
        let response = client.get(url).send().await
            .map_err(|e| anyhow::anyhow!("Failed to start download: {}", e))?;
        
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Download failed with status: {}", response.status()));
        }
        
        // Get content length for progress tracking
        let total_size = response.content_length().unwrap_or(0);
        info!("📦 Model size: {:.1} MB", total_size as f64 / 1_048_576.0);
        
        // Create the temporary file
        let mut temp_file = fs::File::create(&temp_path).await
            .map_err(|e| anyhow::anyhow!("Failed to create temporary file: {}", e))?;
        
        // Download with progress tracking
        let mut downloaded = 0u64;
        let mut stream = response.bytes_stream();
        
        use futures_util::StreamExt;
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result
                .map_err(|e| anyhow::anyhow!("Download error: {}", e))?;
            
            temp_file.write_all(&chunk).await
                .map_err(|e| anyhow::anyhow!("Failed to write to file: {}", e))?;
            
            downloaded += chunk.len() as u64;
            
            if total_size > 0 {
                let progress = (downloaded as f64 / total_size as f64) * 100.0;
                if downloaded % (1024 * 1024) == 0 || downloaded == total_size {
                    info!("⬇️ Download progress: {:.1}% ({:.1} MB / {:.1} MB)", 
                          progress, 
                          downloaded as f64 / 1_048_576.0,
                          total_size as f64 / 1_048_576.0);
                }
            }
        }
        
        // Ensure all data is written
        temp_file.flush().await
            .map_err(|e| anyhow::anyhow!("Failed to flush file: {}", e))?;
        drop(temp_file);
        
        // Move temp file to final location
        fs::rename(&temp_path, &model_path).await
            .map_err(|e| anyhow::anyhow!("Failed to move downloaded file: {}", e))?;
        
        info!("✅ Model {} downloaded successfully!", model.filename());
        
        Ok(())
    }
    
    /// List all available models and their status
    pub async fn list_models(&self) -> Vec<(WhisperModel, bool)> {
        let models = [
            WhisperModel::Tiny,
            WhisperModel::Base,
            WhisperModel::Small,
            WhisperModel::Medium,
            WhisperModel::Large,
        ];
        
        let mut result = Vec::new();
        for model in &models {
            let exists = self.model_exists(model).await;
            result.push((model.clone(), exists));
        }
        
        result
    }
}