use anyhow::{Context, Result};
use hf_hub::api::tokio::Api;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use tracing::{info, warn, debug};

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub name: String,
    pub size: ModelSize,
    pub repo_id: String,
    pub filename: String,
    pub expected_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ModelSize {
    Tiny,
    Base,
    Small,
    Medium,
    Large,
    LargeV2,
    LargeV3,
}

impl ModelSize {
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "tiny" => Ok(ModelSize::Tiny),
            "base" => Ok(ModelSize::Base),
            "small" => Ok(ModelSize::Small),
            "medium" => Ok(ModelSize::Medium),
            "large" => Ok(ModelSize::Large),
            "large-v2" => Ok(ModelSize::LargeV2),
            "large-v3" => Ok(ModelSize::LargeV3),
            _ => Err(anyhow::anyhow!("Unknown model size: {}", s)),
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            ModelSize::Tiny => "tiny".to_string(),
            ModelSize::Base => "base".to_string(),
            ModelSize::Small => "small".to_string(),
            ModelSize::Medium => "medium".to_string(),
            ModelSize::Large => "large".to_string(),
            ModelSize::LargeV2 => "large-v2".to_string(),
            ModelSize::LargeV3 => "large-v3".to_string(),
        }
    }
}

pub struct ModelManager {
    pub cache_dir: PathBuf,
    available_models: HashMap<ModelSize, ModelInfo>,
}

impl ModelManager {
    pub fn new<P: AsRef<Path>>(cache_dir: P) -> Result<Self> {
        let cache_dir = cache_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&cache_dir)
            .context("Failed to create model cache directory")?;

        let available_models = Self::init_available_models();

        Ok(ModelManager {
            cache_dir,
            available_models,
        })
    }

    fn init_available_models() -> HashMap<ModelSize, ModelInfo> {
        let mut models = HashMap::new();

        models.insert(
            ModelSize::Tiny,
            ModelInfo {
                name: "whisper-tiny".to_string(),
                size: ModelSize::Tiny,
                repo_id: "openai/whisper-tiny".to_string(),
                filename: "model.safetensors".to_string(),
                expected_size: 39_000_000, // ~39MB
            },
        );

        models.insert(
            ModelSize::Base,
            ModelInfo {
                name: "whisper-base".to_string(),
                size: ModelSize::Base,
                repo_id: "openai/whisper-base".to_string(),
                filename: "model.safetensors".to_string(),
                expected_size: 74_000_000, // ~74MB
            },
        );

        models.insert(
            ModelSize::Small,
            ModelInfo {
                name: "whisper-small".to_string(),
                size: ModelSize::Small,
                repo_id: "openai/whisper-small".to_string(),
                filename: "model.safetensors".to_string(),
                expected_size: 244_000_000, // ~244MB
            },
        );

        models.insert(
            ModelSize::Medium,
            ModelInfo {
                name: "whisper-medium".to_string(),
                size: ModelSize::Medium,
                repo_id: "openai/whisper-medium".to_string(),
                filename: "model.safetensors".to_string(),
                expected_size: 769_000_000, // ~769MB
            },
        );

        models.insert(
            ModelSize::Large,
            ModelInfo {
                name: "whisper-large".to_string(),
                size: ModelSize::Large,
                repo_id: "openai/whisper-large".to_string(),
                filename: "model.safetensors".to_string(),
                expected_size: 1_550_000_000, // ~1.55GB
            },
        );

        models.insert(
            ModelSize::LargeV3,
            ModelInfo {
                name: "whisper-large-v3".to_string(),
                size: ModelSize::LargeV3,
                repo_id: "openai/whisper-large-v3".to_string(),
                filename: "model.safetensors".to_string(),
                expected_size: 1_550_000_000, // ~1.55GB
            },
        );

        models
    }

    pub fn get_model_info(&self, size: &ModelSize) -> Option<&ModelInfo> {
        self.available_models.get(size)
    }

    pub fn get_model_path(&self, size: &ModelSize) -> Option<PathBuf> {
        if let Some(info) = self.get_model_info(size) {
            let path = self.cache_dir.join(&info.filename);
            if path.exists() {
                Some(path)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub async fn ensure_model_downloaded(&self, size: &ModelSize) -> Result<PathBuf> {
        info!("Ensuring model {:?} is available", size);

        let model_info = self.get_model_info(size)
            .ok_or_else(|| anyhow::anyhow!("Unknown model size: {:?}", size))?;

        let model_path = self.cache_dir.join(&model_info.filename);

        // Check if model already exists and is valid
        if self.is_model_valid(&model_path, model_info).await? {
            info!("Model {:?} already cached at {:?}", size, model_path);
            return Ok(model_path);
        }

        info!("Downloading model {:?} from {}", size, model_info.repo_id);
        self.download_model(model_info).await?;

        // Verify the downloaded model
        if !self.is_model_valid(&model_path, model_info).await? {
            return Err(anyhow::anyhow!("Downloaded model failed validation"));
        }

        info!("Model {:?} successfully downloaded and validated", size);
        Ok(model_path)
    }

    async fn download_model(&self, info: &ModelInfo) -> Result<()> {
        info!("Starting download of {} ({:.1}MB)", info.name, info.expected_size as f64 / 1_000_000.0);

        let api = Api::new()?;
        let repo = api.model(info.repo_id.clone());

        // Download the main model file
        let model_file = repo.get(&info.filename).await
            .context("Failed to download model file")?;

        // Also download config and tokenizer files if they exist
        let _config_file = repo.get("config.json").await
            .unwrap_or_else(|_| {
                debug!("config.json not found, using defaults");
                self.cache_dir.join("config.json") // placeholder
            });

        let _tokenizer_file = repo.get("tokenizer.json").await
            .unwrap_or_else(|_| {
                debug!("tokenizer.json not found, using defaults");  
                self.cache_dir.join("tokenizer.json") // placeholder
            });

        // Copy the model to our cache directory with the expected filename
        let target_path = self.cache_dir.join(&info.filename);
        std::fs::copy(&model_file, &target_path)
            .context("Failed to copy model to cache directory")?;

        info!("Model downloaded successfully to {:?}", target_path);
        Ok(())
    }

    async fn is_model_valid(&self, path: &Path, info: &ModelInfo) -> Result<bool> {
        if !path.exists() {
            return Ok(false);
        }

        // Check file size
        let metadata = std::fs::metadata(path)
            .context("Failed to read model file metadata")?;

        let file_size = metadata.len();
        
        // Allow some variance in file size (±10%)
        let min_size = (info.expected_size as f64 * 0.9) as u64;
        let max_size = (info.expected_size as f64 * 1.1) as u64;

        if file_size < min_size || file_size > max_size {
            warn!("Model file size mismatch: expected ~{}, got {}", info.expected_size, file_size);
            return Ok(false);
        }

        // TODO: Add SHA256 checksum validation if we have the hashes
        debug!("Model validation passed for {:?}", path);
        Ok(true)
    }

    pub fn list_available_models(&self) -> Vec<&ModelInfo> {
        self.available_models.values().collect()
    }

    pub fn list_cached_models(&self) -> Vec<ModelSize> {
        self.available_models
            .keys()
            .filter(|&size| self.get_model_path(size).is_some())
            .cloned()
            .collect()
    }

    pub async fn clear_cache(&self) -> Result<()> {
        info!("Clearing model cache at {:?}", self.cache_dir);
        
        if self.cache_dir.exists() {
            std::fs::remove_dir_all(&self.cache_dir)
                .context("Failed to remove cache directory")?;
            std::fs::create_dir_all(&self.cache_dir)
                .context("Failed to recreate cache directory")?;
        }

        Ok(())
    }

    pub fn get_cache_size(&self) -> Result<u64> {
        if !self.cache_dir.exists() {
            return Ok(0);
        }

        let mut total_size = 0u64;
        
        for entry in std::fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_file() {
                total_size += metadata.len();
            }
        }

        Ok(total_size)
    }

    pub fn format_size(bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
        let mut size = bytes as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_model_size_parsing() {
        assert_eq!(ModelSize::from_str("tiny").unwrap(), ModelSize::Tiny);
        assert_eq!(ModelSize::from_str("LARGE").unwrap(), ModelSize::Large);
        assert_eq!(ModelSize::from_str("large-v3").unwrap(), ModelSize::LargeV3);
    }

    #[test]
    fn test_model_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ModelManager::new(temp_dir.path()).unwrap();
        
        assert!(manager.cache_dir.exists());
        assert!(!manager.available_models.is_empty());
    }

    #[test]
    fn test_model_info_retrieval() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ModelManager::new(temp_dir.path()).unwrap();
        
        let info = manager.get_model_info(&ModelSize::Tiny);
        assert!(info.is_some());
        assert_eq!(info.unwrap().size, ModelSize::Tiny);
    }

    #[test]
    fn test_format_size() {
        assert_eq!(ModelManager::format_size(512), "512.0 B");
        assert_eq!(ModelManager::format_size(1536), "1.5 KB");
        assert_eq!(ModelManager::format_size(1_048_576), "1.0 MB");
        assert_eq!(ModelManager::format_size(1_073_741_824), "1.0 GB");
    }
}