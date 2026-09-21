use anyhow::{Context, Result};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tokio::io::AsyncWriteExt;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// File name of this model in the ggerganov/whisper.cpp catalogue.
    pub fn filename(&self) -> String {
        match self {
            ModelSize::Tiny => "ggml-tiny.bin",
            ModelSize::Base => "ggml-base.bin",
            ModelSize::Small => "ggml-small.bin",
            ModelSize::Medium => "ggml-medium.bin",
            ModelSize::Large => "ggml-large-v1.bin",
            ModelSize::LargeV2 => "ggml-large-v2.bin",
            ModelSize::LargeV3 => "ggml-large-v3.bin",
        }
        .to_string()
    }
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

/// The model this machine should start with: a GPU can carry a larger one,
/// and a small machine should not swap while transcribing.
pub fn recommended_model() -> ModelSize {
    static RECOMMENDED: OnceLock<ModelSize> = OnceLock::new();
    *RECOMMENDED.get_or_init(|| {
        let gpu = crate::transcription::device::GpuAvailability::detect().available;
        let mut system = sysinfo::System::new();
        system.refresh_memory();
        let memory_gb = system.total_memory() / 1024 / 1024 / 1024;
        match (gpu, memory_gb) {
            (true, 16..) => ModelSize::Medium,
            (true, _) => ModelSize::Small,
            (false, 16..) => ModelSize::Base,
            (false, _) => ModelSize::Tiny,
        }
    })
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
                    "7d99f41a10525d0206bddadd86760181fa920438b6b33237e3118ff6c83bb53d".to_string(),
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
                    "9a423fe4d40c82774b6af34115b8b935f34152246eb19e80e376071d3f999487".to_string(),
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
                    "64d182b440b98d5203c4f9bd541544d84c605196c4f7b845dfa11fb23594d1e2".to_string(),
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
        let bar = std::sync::Mutex::new(None::<ProgressBar>);
        let name = self
            .get_model_info(size)
            .map(|info| info.name.clone())
            .unwrap_or_default();
        self.ensure_model_downloaded_with(size, &move |done, total| {
            let mut bar = match bar.lock() {
                Ok(bar) => bar,
                Err(_) => return,
            };
            let bar = bar.get_or_insert_with(|| Self::progress_bar(total, &name));
            bar.set_position(done);
            if done >= total {
                bar.finish_and_clear();
            }
        })
        .await
    }

    /// Download with a progress callback of `(downloaded, total)` bytes, for
    /// callers that draw their own progress.
    pub async fn ensure_model_downloaded_with(
        &self,
        size: &ModelSize,
        progress: &(dyn Fn(u64, u64) + Send + Sync),
    ) -> Result<PathBuf> {
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
        self.download_model(model_info, progress).await?;

        // Verify the downloaded model
        if !self.is_model_valid(&model_path, model_info).await? {
            return Err(anyhow::anyhow!("Downloaded model failed validation"));
        }

        info!("Model {:?} successfully downloaded and validated", size);
        Ok(model_path)
    }

    /// Where a catalogue entry is fetched from.
    pub fn download_url(info: &ModelInfo) -> String {
        format!(
            "https://huggingface.co/{}/resolve/main/{}",
            info.repo_id, info.filename
        )
    }

    async fn download_model(
        &self,
        info: &ModelInfo,
        progress: &(dyn Fn(u64, u64) + Send + Sync),
    ) -> Result<()> {
        let url = Self::download_url(info);
        info!(
            "Downloading {} ({:.1} MB) from {}",
            info.name,
            info.expected_size as f64 / 1_000_000.0,
            url
        );

        let target = self.cache_dir.join(&info.filename);
        let partial = self.cache_dir.join(format!("{}.part", info.filename));

        let client = reqwest::Client::builder()
            .user_agent(concat!("hush/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("Failed to build HTTP client")?;
        let response = client
            .get(&url)
            .send()
            .await
            .context("Failed to start model download")?
            .error_for_status()
            .with_context(|| format!("Model download failed: {}", url))?;

        let total = response.content_length().unwrap_or(info.expected_size);
        let mut downloaded = 0u64;
        progress(0, total);

        let mut file = tokio::fs::File::create(&partial)
            .await
            .with_context(|| format!("Failed to create {}", partial.display()))?;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Model download interrupted")?;
            file.write_all(&chunk)
                .await
                .context("Failed to write model file")?;
            downloaded += chunk.len() as u64;
            progress(downloaded, total);
        }
        file.flush().await.context("Failed to flush model file")?;
        drop(file);
        progress(total, total);

        tokio::fs::rename(&partial, &target)
            .await
            .context("Failed to move the downloaded model into place")?;

        info!("Model downloaded to {}", target.display());
        Ok(())
    }

    fn progress_bar(total: u64, name: &str) -> ProgressBar {
        use std::io::IsTerminal;
        if !std::io::stderr().is_terminal() {
            return ProgressBar::hidden();
        }
        let bar = ProgressBar::new(total);
        if let Ok(style) =
            ProgressStyle::with_template("{msg} [{bar:30}] {bytes}/{total_bytes} ({eta})")
        {
            bar.set_style(style.progress_chars("=> "));
        }
        bar.set_message(name.to_owned());
        bar
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
    fn download_url_points_at_the_whisper_cpp_catalogue() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ModelManager::new(temp_dir.path()).unwrap();
        let info = manager.get_model_info(&ModelSize::Base).unwrap();
        assert_eq!(
            ModelManager::download_url(info),
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin"
        );
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

        // Verify that Base and LargeV3 models have checksums (ggml format from whisper.cpp)
        let base_info = manager.get_model_info(&ModelSize::Base).unwrap();
        assert!(base_info.sha256.is_some());
        assert_eq!(
            base_info.sha256.as_ref().unwrap(),
            "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe"
        );

        let large_v3_info = manager.get_model_info(&ModelSize::LargeV3).unwrap();
        assert!(large_v3_info.sha256.is_some());
        assert_eq!(
            large_v3_info.sha256.as_ref().unwrap(),
            "64d182b440b98d5203c4f9bd541544d84c605196c4f7b845dfa11fb23594d1e2"
        );
    }
}
