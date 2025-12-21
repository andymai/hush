use anyhow::{Context, Result};
use hf_hub::api::tokio::Api;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub name: String,
    pub size: ModelSize,
    pub repo_id: String,
    pub filename: String,
    pub expected_size: u64,
    /// SHA256 checksum for model integrity verification (None for custom models)
    pub sha256: Option<String>,
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

impl std::str::FromStr for ModelSize {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
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
}

impl std::fmt::Display for ModelSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelSize::Tiny => write!(f, "tiny"),
            ModelSize::Base => write!(f, "base"),
            ModelSize::Small => write!(f, "small"),
            ModelSize::Medium => write!(f, "medium"),
            ModelSize::Large => write!(f, "large"),
            ModelSize::LargeV2 => write!(f, "large-v2"),
            ModelSize::LargeV3 => write!(f, "large-v3"),
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
        std::fs::create_dir_all(&cache_dir).context("Failed to create model cache directory")?;

        let available_models = Self::init_available_models();

        Ok(ModelManager {
            cache_dir,
            available_models,
        })
    }

    fn init_available_models() -> HashMap<ModelSize, ModelInfo> {
        let mut models = HashMap::new();

        // Using ggml format from ggerganov/whisper.cpp - compatible with whisper-rs
        models.insert(
            ModelSize::Tiny,
            ModelInfo {
                name: "whisper-tiny".to_string(),
                size: ModelSize::Tiny,
                repo_id: "ggerganov/whisper.cpp".to_string(),
                filename: "ggml-tiny.bin".to_string(),
                expected_size: 77_700_000, // ~77.7MB
                sha256: Some(
                    "be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21".to_string(),
                ),
            },
        );

        models.insert(
            ModelSize::Base,
            ModelInfo {
                name: "whisper-base".to_string(),
                size: ModelSize::Base,
                repo_id: "ggerganov/whisper.cpp".to_string(),
                filename: "ggml-base.bin".to_string(),
                expected_size: 147_000_000, // ~147MB
                sha256: Some(
                    "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe".to_string(),
                ),
            },
        );

        models.insert(
            ModelSize::Small,
            ModelInfo {
                name: "whisper-small".to_string(),
                size: ModelSize::Small,
                repo_id: "ggerganov/whisper.cpp".to_string(),
                filename: "ggml-small.bin".to_string(),
                expected_size: 488_000_000, // ~488MB
                sha256: Some(
                    "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b".to_string(),
                ),
            },
        );

        models.insert(
            ModelSize::Medium,
            ModelInfo {
                name: "whisper-medium".to_string(),
                size: ModelSize::Medium,
                repo_id: "ggerganov/whisper.cpp".to_string(),
                filename: "ggml-medium.bin".to_string(),
                expected_size: 1_533_000_000, // ~1.53GB
                sha256: Some(
                    "6c14d5adee5f86394037b4e4e8b59f1673b6cee10e3cf0b11bbdbee79c156208".to_string(),
                ),
            },
        );

        models.insert(
            ModelSize::Large,
            ModelInfo {
                name: "whisper-large".to_string(),
                size: ModelSize::Large,
                repo_id: "ggerganov/whisper.cpp".to_string(),
                filename: "ggml-large-v1.bin".to_string(),
                expected_size: 3_094_000_000, // ~3.09GB
                sha256: Some(
                    "0f4c8c42c8b00e600cfe32f73b8e53fdffc92cc91a2e5abdf6e13ae6d1ed9d44".to_string(),
                ),
            },
        );

        models.insert(
            ModelSize::LargeV2,
            ModelInfo {
                name: "whisper-large-v2".to_string(),
                size: ModelSize::LargeV2,
                repo_id: "ggerganov/whisper.cpp".to_string(),
                filename: "ggml-large-v2.bin".to_string(),
                expected_size: 3_094_000_000, // ~3.09GB
                sha256: Some(
                    "81f94ac64f4d28d8d2264b9d3823d5d0c9a8bdcd8e80baff97c1eed1cee8c549".to_string(),
                ),
            },
        );

        models.insert(
            ModelSize::LargeV3,
            ModelInfo {
                name: "whisper-large-v3".to_string(),
                size: ModelSize::LargeV3,
                repo_id: "ggerganov/whisper.cpp".to_string(),
                filename: "ggml-large-v3.bin".to_string(),
                expected_size: 3_094_000_000, // ~3.09GB
                sha256: Some(
                    "ad82bf6a9043ceed055076d0fd39f5f186ff8062f19f9e9c50c5c8fb3a9a6e9f".to_string(),
                ),
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

        let model_info = self
            .get_model_info(size)
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
        info!(
            "Starting download of {} ({:.1}MB)",
            info.name,
            info.expected_size as f64 / 1_000_000.0
        );

        let api = Api::new()?;
        let repo = api.model(info.repo_id.clone());

        // Download the main model file
        let model_file = repo
            .get(&info.filename)
            .await
            .context("Failed to download model file")?;

        // Also download config and tokenizer files if they exist
        let _config_file = repo.get("config.json").await.unwrap_or_else(|_| {
            debug!("config.json not found, using defaults");
            self.cache_dir.join("config.json") // placeholder
        });

        let _tokenizer_file = repo.get("tokenizer.json").await.unwrap_or_else(|_| {
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

    /// Computes the SHA256 checksum of a file
    fn compute_sha256(path: &Path) -> Result<String> {
        use std::io::Read;

        let mut file =
            std::fs::File::open(path).context("Failed to open file for checksum computation")?;

        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192]; // 8KB buffer

        loop {
            let bytes_read = file
                .read(&mut buffer)
                .context("Failed to read file for checksum computation")?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        let result = hasher.finalize();
        Ok(format!("{:x}", result))
    }

    async fn is_model_valid(&self, path: &Path, info: &ModelInfo) -> Result<bool> {
        if !path.exists() {
            return Ok(false);
        }

        // Check file size
        let metadata = std::fs::metadata(path).context("Failed to read model file metadata")?;

        let file_size = metadata.len();

        // Allow some variance in file size (±10%)
        let min_size = (info.expected_size as f64 * 0.9) as u64;
        let max_size = (info.expected_size as f64 * 1.1) as u64;

        if file_size < min_size || file_size > max_size {
            warn!(
                "Model file size mismatch: expected ~{}, got {}",
                info.expected_size, file_size
            );
            return Ok(false);
        }

        // Verify SHA256 checksum if available
        if let Some(expected_sha256) = &info.sha256 {
            debug!("Validating SHA256 checksum for {:?}", path);
            let actual_sha256 =
                Self::compute_sha256(path).context("Failed to compute model checksum")?;

            if actual_sha256 != *expected_sha256 {
                warn!(
                    "Model checksum mismatch for {:?}: expected {}, got {}",
                    path, expected_sha256, actual_sha256
                );
                return Ok(false);
            }
            debug!("SHA256 checksum validated successfully");
        } else {
            debug!("No SHA256 checksum available, skipping checksum validation");
        }

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
            std::fs::remove_dir_all(&self.cache_dir).context("Failed to remove cache directory")?;
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
        assert_eq!("tiny".parse::<ModelSize>().unwrap(), ModelSize::Tiny);
        assert_eq!("LARGE".parse::<ModelSize>().unwrap(), ModelSize::Large);
        assert_eq!("large-v3".parse::<ModelSize>().unwrap(), ModelSize::LargeV3);
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

    #[test]
    fn test_compute_sha256() {
        use std::io::Write;

        // Create a temporary file with known content
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        let mut file = std::fs::File::create(&test_file).unwrap();
        file.write_all(b"Hello, World!").unwrap();
        drop(file);

        // Compute SHA256
        let checksum = ModelManager::compute_sha256(&test_file).unwrap();

        // Expected SHA256 for "Hello, World!"
        // echo -n "Hello, World!" | sha256sum
        let expected = "dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f";

        assert_eq!(checksum, expected);
    }

    #[test]
    fn test_model_has_sha256_checksums() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ModelManager::new(temp_dir.path()).unwrap();

        // Verify that Base and LargeV3 models have checksums
        let base_info = manager.get_model_info(&ModelSize::Base).unwrap();
        assert!(base_info.sha256.is_some());
        assert_eq!(
            base_info.sha256.as_ref().unwrap(),
            "07cadb9f25677c8d50df603e66a98fbd842cce45047139baeb16e6219a1e807b"
        );

        let large_v3_info = manager.get_model_info(&ModelSize::LargeV3).unwrap();
        assert!(large_v3_info.sha256.is_some());
        assert_eq!(
            large_v3_info.sha256.as_ref().unwrap(),
            "a8e94b85976e5864ba3e9525c7e6c83b2a1eca42d4b797a0c7c24d778e40fd95"
        );
    }
}
