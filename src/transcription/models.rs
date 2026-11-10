use anyhow::{Context, Result};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, OnceLock};
use std::time::SystemTime;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub name: String,
    pub repo_id: String,
    pub filename: String,
    pub expected_size: u64,
    /// SHA256 checksum for model integrity verification (None for custom models)
    pub sha256: Option<String>,
}

/// The Silero voice activity detection model that whisper.cpp's
/// `models/download-vad-model.sh` fetches.
pub const VAD_FILENAME: &str = "ggml-silero-v6.2.0.bin";

/// The catalogue entry for the voice activity detection model. The checksum
/// is the Git LFS object ID that Hugging Face lists for the file.
pub fn vad_model() -> ModelInfo {
    ModelInfo {
        name: "silero-vad".to_string(),
        repo_id: "ggml-org/whisper-vad".to_string(),
        filename: VAD_FILENAME.to_string(),
        expected_size: 885_098,
        sha256: Some(
            "2aa269b785eeb53a82983a20501ddf7c1d9c48e33ab63a41391ac6c9f7fb6987".to_string(),
        ),
    }
}

/// Where the voice activity detection model lives in `models_dir`,
/// downloaded or not.
pub fn vad_model_path(models_dir: &Path) -> PathBuf {
    models_dir.join(VAD_FILENAME)
}

/// SpeechBrain's VoxLingua107 ECAPA-TDNN language-ID model, converted to
/// GGUF with transcribe.cpp. `transcription.detect_languages` uses it.
pub const LANGID_FILENAME: &str = "lang-id-voxlingua107-ecapa-Q8_0.gguf";

/// The catalogue entry for the language-ID model.
pub fn langid_model() -> ModelInfo {
    ModelInfo {
        name: "voxlingua107-langid".to_string(),
        repo_id: "collectivecontext/hush-models".to_string(),
        filename: LANGID_FILENAME.to_string(),
        expected_size: 26_693_632,
        sha256: Some(
            "60176ea5a4d3a33d3ec2d9608d984a43d57307daf5bf22f6cd724b696989b8fa".to_string(),
        ),
    }
}

/// The catalogue entry for Parakeet TDT 0.6B v3.
pub fn parakeet_v3_model() -> ModelInfo {
    ModelInfo {
        name: "parakeet-tdt-0.6b-v3".to_string(),
        repo_id: "collectivecontext/hush-models".to_string(),
        filename: PARAKEET_V3_FILENAME.to_string(),
        expected_size: 739_508_576,
        sha256: Some(
            "5859f77944efcd8eafa23a6350731960b2b55b2203df51f319665c807d802cc7".to_string(),
        ),
    }
}

/// Where the language-ID model lives in `models_dir`, downloaded or not.
pub fn langid_model_path(models_dir: &Path) -> PathBuf {
    models_dir.join(LANGID_FILENAME)
}

/// The size and modification time of a file. A file with the same stamp as
/// at its last check is taken to be the same file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileStamp {
    len: u64,
    modified: SystemTime,
}

impl FileStamp {
    fn of(path: &Path) -> Option<Self> {
        let metadata = std::fs::metadata(path).ok()?;
        Some(Self {
            len: metadata.len(),
            modified: metadata.modified().ok()?,
        })
    }
}

/// The last size and SHA-256 check of each model file, so that a file is
/// hashed again only when its size or modification time changes. The
/// settings window asks every few hundred milliseconds.
static CHECKED: LazyLock<parking_lot::Mutex<HashMap<PathBuf, (FileStamp, bool)>>> =
    LazyLock::new(Default::default);

/// What the size and SHA-256 check of a model file found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Valid,
    Missing,
    WrongSize,
    WrongHash,
}

/// A download with the expected size but another SHA-256: the published file
/// differs from the pinned one, so a new try gets the same bytes.
#[derive(Debug, thiserror::Error)]
#[error("The download of {name} failed its SHA-256 check")]
pub struct ChecksumMismatch {
    pub name: String,
}

/// A download that cannot connect, or that gets no bytes for this long,
/// fails instead of waiting without end.
const DOWNLOAD_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
const DOWNLOAD_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelSize {
    Tiny,
    Base,
    Small,
    Medium,
    Large,
    LargeV2,
    LargeV3,
    /// NVIDIA Parakeet TDT 0.6B v3, opt-in: 25 European languages, no prompt
    ParakeetV3,
}

/// The languages of Parakeet TDT 0.6B v3, which it tells apart on its own.
pub const PARAKEET_V3_LANGUAGES: [&str; 25] = [
    "bg", "hr", "cs", "da", "nl", "en", "et", "fi", "fr", "de", "el", "hu", "it", "lv", "lt", "mt",
    "pl", "pt", "ro", "ru", "sk", "sl", "es", "sv", "uk",
];

impl ModelSize {
    /// File name of this model in its catalogue.
    pub fn filename(&self) -> String {
        match self {
            ModelSize::Tiny => "ggml-tiny.bin",
            ModelSize::Base => "ggml-base.bin",
            ModelSize::Small => "ggml-small.bin",
            ModelSize::Medium => "ggml-medium.bin",
            ModelSize::Large => "ggml-large-v1.bin",
            ModelSize::LargeV2 => "ggml-large-v2.bin",
            ModelSize::LargeV3 => "ggml-large-v3.bin",
            ModelSize::ParakeetV3 => PARAKEET_V3_FILENAME,
        }
        .to_string()
    }

    pub fn is_parakeet(&self) -> bool {
        matches!(self, ModelSize::ParakeetV3)
    }
}

/// NVIDIA's Parakeet TDT 0.6B v3 (CC BY 4.0), converted to GGUF with
/// transcribe.cpp and quantized to Q8_0.
pub const PARAKEET_V3_FILENAME: &str = "parakeet-tdt-0.6b-v3-Q8_0.gguf";

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
            "parakeet-v3" => Ok(ModelSize::ParakeetV3),
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
            ModelSize::ParakeetV3 => write!(f, "parakeet-v3"),
        }
    }
}

/// The model this machine should start with: a GPU can carry a larger one,
/// and a small machine should not swap while transcribing.
pub fn recommended_model() -> ModelSize {
    static RECOMMENDED: OnceLock<ModelSize> = OnceLock::new();
    *RECOMMENDED.get_or_init(|| {
        let gpu = crate::transcription::device::GpuAvailability::detect().available;
        let memory_gb = sysinfo::System::new().map_or(0, |mut system| {
            system.refresh_memory();
            system.total_memory() / 1024 / 1024 / 1024
        });
        match (gpu, memory_gb) {
            (true, 16..) => ModelSize::Medium,
            (true, _) => ModelSize::Small,
            (false, 16..) => ModelSize::Base,
            (false, _) => ModelSize::Tiny,
        }
    })
}

/// The most header bytes [`file_architecture`] reads.
const HEADER_LIMIT: u64 = 16 << 20;

/// The longest GGUF key or architecture name that [`file_architecture`] reads.
const NAME_LIMIT: u64 = 256;

const GGUF_STRING: u32 = 8;
const GGUF_ARRAY: u32 = 9;

/// The architecture of a model file without a model load: the
/// `general.architecture` of a GGUF file, or `whisper` for the ggml `.bin`
/// format of whisper.cpp. `None` when the file is missing or is neither.
pub fn file_architecture(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    architecture_in(&mut std::io::BufReader::new(file).take(HEADER_LIMIT))
}

fn architecture_in(reader: &mut impl Read) -> Option<String> {
    let mut magic = [0; 4];
    reader.read_exact(&mut magic).ok()?;
    match &magic {
        b"lmgg" => return Some("whisper".to_string()),
        b"GGUF" => {},
        _ => return None,
    }
    // Version 1 used 32-bit counts.
    if read_u32(reader)? < 2 {
        return None;
    }
    let _tensors = read_u64(reader)?;
    let pairs = read_u64(reader)?;
    for _ in 0..pairs {
        let key = read_name(reader)?;
        let kind = read_u32(reader)?;
        if key == "general.architecture" {
            return if kind == GGUF_STRING {
                read_name(reader)
            } else {
                None
            };
        }
        skip_value(reader, kind)?;
    }
    None
}

fn read_u32(reader: &mut impl Read) -> Option<u32> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes).ok()?;
    Some(u32::from_le_bytes(bytes))
}

fn read_u64(reader: &mut impl Read) -> Option<u64> {
    let mut bytes = [0; 8];
    reader.read_exact(&mut bytes).ok()?;
    Some(u64::from_le_bytes(bytes))
}

fn read_name(reader: &mut impl Read) -> Option<String> {
    let len = read_u64(reader)?;
    if len > NAME_LIMIT {
        return None;
    }
    let mut bytes = vec![0; len as usize];
    reader.read_exact(&mut bytes).ok()?;
    String::from_utf8(bytes).ok()
}

fn skip(reader: &mut impl Read, bytes: u64) -> Option<()> {
    let skipped = std::io::copy(&mut reader.by_ref().take(bytes), &mut std::io::sink()).ok()?;
    (skipped == bytes).then_some(())
}

/// The bytes of one GGUF value of a fixed-size type.
fn scalar_size(kind: u32) -> Option<u64> {
    match kind {
        0 | 1 | 7 => Some(1),
        2 | 3 => Some(2),
        4..=6 => Some(4),
        10..=12 => Some(8),
        _ => None,
    }
}

fn skip_value(reader: &mut impl Read, kind: u32) -> Option<()> {
    match kind {
        GGUF_STRING => {
            let len = read_u64(reader)?;
            skip(reader, len)
        },
        GGUF_ARRAY => {
            let element = read_u32(reader)?;
            let count = read_u64(reader)?;
            if let Some(size) = scalar_size(element) {
                return skip(reader, size.checked_mul(count)?);
            }
            if element != GGUF_STRING {
                return None;
            }
            for _ in 0..count {
                skip_value(reader, GGUF_STRING)?;
            }
            Some(())
        },
        kind => skip(reader, scalar_size(kind)?),
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

        // The ggml files of the whisper.cpp catalogue; transcribe.cpp loads them as they are.
        models.insert(
            ModelSize::Tiny,
            ModelInfo {
                name: "whisper-tiny".to_string(),
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
                repo_id: "ggerganov/whisper.cpp".to_string(),
                filename: "ggml-large-v3.bin".to_string(),
                expected_size: 3_094_000_000, // ~3.09GB
                sha256: Some(
                    "64d182b440b98d5203c4f9bd541544d84c605196c4f7b845dfa11fb23594d1e2".to_string(),
                ),
            },
        );

        models.insert(ModelSize::ParakeetV3, parakeet_v3_model());

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
        let name = self
            .get_model_info(size)
            .map(|info| info.name.clone())
            .unwrap_or_default();
        self.ensure_model_downloaded_with(size, &Self::terminal_progress(name))
            .await
    }

    /// Download with a progress callback of `(downloaded, total)` bytes, for
    /// callers that draw their own progress.
    pub async fn ensure_model_downloaded_with(
        &self,
        size: &ModelSize,
        progress: &(dyn Fn(u64, u64) + Send + Sync),
    ) -> Result<PathBuf> {
        let model_info = self
            .get_model_info(size)
            .ok_or_else(|| anyhow::anyhow!("Unknown model size: {:?}", size))?;
        self.ensure_file(model_info, progress).await
    }

    /// The voice activity detection model in this cache, when a copy that
    /// passes the size and SHA-256 check is there.
    pub fn vad_model_path(&self) -> Option<PathBuf> {
        self.valid_path(&vad_model())
    }

    /// Download the voice activity detection model unless a valid copy is
    /// already in the cache.
    pub async fn ensure_vad_downloaded(&self) -> Result<PathBuf> {
        let info = vad_model();
        self.ensure_vad_downloaded_with(&Self::terminal_progress(info.name))
            .await
    }

    pub async fn ensure_vad_downloaded_with(
        &self,
        progress: &(dyn Fn(u64, u64) + Send + Sync),
    ) -> Result<PathBuf> {
        self.ensure_file(&vad_model(), progress).await
    }

    /// The language-ID model in this cache, when a copy that passes the size
    /// and SHA-256 check is there.
    pub fn langid_model_path(&self) -> Option<PathBuf> {
        self.valid_path(&langid_model())
    }

    /// The file of `info` in this cache, when it passes the size and SHA-256
    /// check.
    pub fn valid_path(&self, info: &ModelInfo) -> Option<PathBuf> {
        let path = self.cache_dir.join(&info.filename);
        let stamp = FileStamp::of(&path)?;
        if let Some((checked, valid)) = CHECKED.lock().get(&path) {
            if *checked == stamp {
                return valid.then_some(path);
            }
        }
        let valid = match Self::verify(&path, info) {
            Ok(verdict) => verdict == Verdict::Valid,
            Err(e) => {
                warn!("Could not check {}: {:#}", path.display(), e);
                return None;
            },
        };
        CHECKED.lock().insert(path.clone(), (stamp, valid));
        valid.then_some(path)
    }

    /// Download the language-ID model unless a valid copy is already in the
    /// cache.
    pub async fn ensure_langid_downloaded(&self) -> Result<PathBuf> {
        let info = langid_model();
        self.ensure_langid_downloaded_with(&Self::terminal_progress(info.name))
            .await
    }

    pub async fn ensure_langid_downloaded_with(
        &self,
        progress: &(dyn Fn(u64, u64) + Send + Sync),
    ) -> Result<PathBuf> {
        self.ensure_file(&langid_model(), progress).await
    }

    fn terminal_progress(name: String) -> impl Fn(u64, u64) + Send + Sync {
        let bar = std::sync::Mutex::new(None::<ProgressBar>);
        move |done, total| {
            let mut bar = match bar.lock() {
                Ok(bar) => bar,
                Err(_) => return,
            };
            let bar = bar.get_or_insert_with(|| Self::progress_bar(total, &name));
            bar.set_position(done);
            if done >= total {
                bar.finish_and_clear();
            }
        }
    }

    async fn ensure_file(
        &self,
        model_info: &ModelInfo,
        progress: &(dyn Fn(u64, u64) + Send + Sync),
    ) -> Result<PathBuf> {
        info!("Ensuring model {} is available", model_info.name);
        let model_path = self.cache_dir.join(&model_info.filename);

        if Self::is_valid(&model_path, model_info)? {
            info!(
                "Model {} already cached at {:?}",
                model_info.name, model_path
            );
            return Ok(model_path);
        }

        info!(
            "Downloading model {} from {}",
            model_info.name, model_info.repo_id
        );
        self.download_model(model_info, progress).await?;

        info!(
            "Model {} successfully downloaded and validated",
            model_info.name
        );
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
            .connect_timeout(DOWNLOAD_CONNECT_TIMEOUT)
            .read_timeout(DOWNLOAD_READ_TIMEOUT)
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

        Self::promote(&partial, &target, info)?;
        info!("Model downloaded to {}", target.display());
        Ok(())
    }

    /// Move a finished download into place when it passes the size and
    /// SHA-256 check, and delete it when it does not.
    fn promote(partial: &Path, target: &Path, info: &ModelInfo) -> Result<()> {
        let verdict = Self::verify(partial, info)?;
        if verdict != Verdict::Valid {
            if let Err(e) = std::fs::remove_file(partial) {
                warn!("Could not delete {}: {}", partial.display(), e);
            }
            return Err(match verdict {
                Verdict::WrongHash => ChecksumMismatch {
                    name: info.name.clone(),
                }
                .into(),
                _ => anyhow::anyhow!("The download of {} has the wrong size", info.name),
            });
        }
        std::fs::rename(partial, target)
            .context("Failed to move the downloaded model into place")?;
        if let Some(stamp) = FileStamp::of(target) {
            CHECKED.lock().insert(target.to_path_buf(), (stamp, true));
        }
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
        use std::fmt::Write;

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

        let digest = hasher.finalize();
        let mut hex = String::with_capacity(digest.len() * 2);
        for byte in digest.iter() {
            write!(hex, "{byte:02x}")?;
        }
        Ok(hex)
    }

    fn is_valid(path: &Path, info: &ModelInfo) -> Result<bool> {
        Ok(Self::verify(path, info)? == Verdict::Valid)
    }

    fn verify(path: &Path, info: &ModelInfo) -> Result<Verdict> {
        if !path.exists() {
            return Ok(Verdict::Missing);
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
            return Ok(Verdict::WrongSize);
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
                return Ok(Verdict::WrongHash);
            }
            debug!("SHA256 checksum validated successfully");
        } else {
            debug!("No SHA256 checksum available, skipping checksum validation");
        }

        debug!("Model validation passed for {:?}", path);
        Ok(Verdict::Valid)
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
        assert_eq!(info.unwrap().filename, ModelSize::Tiny.filename());
    }

    #[test]
    fn the_vad_model_comes_from_the_whisper_cpp_vad_catalogue() {
        let info = vad_model();
        assert_eq!(
            ModelManager::download_url(&info),
            "https://huggingface.co/ggml-org/whisper-vad/resolve/main/ggml-silero-v6.2.0.bin"
        );
        assert_eq!(info.sha256.as_deref().map(str::len), Some(64));

        let temp_dir = TempDir::new().unwrap();
        assert_eq!(
            vad_model_path(temp_dir.path()),
            temp_dir.path().join(VAD_FILENAME)
        );
        let manager = ModelManager::new(temp_dir.path()).unwrap();
        assert_eq!(manager.vad_model_path(), None);
        std::fs::write(temp_dir.path().join(VAD_FILENAME), b"x").unwrap();
        assert_eq!(manager.vad_model_path(), None, "a damaged file is no model");
        assert!(
            manager.list_cached_models().is_empty(),
            "the VAD model is not a Whisper model"
        );
    }

    #[test]
    fn the_language_id_model_comes_from_the_hush_model_repo() {
        let info = langid_model();
        assert_eq!(
            ModelManager::download_url(&info),
            "https://huggingface.co/collectivecontext/hush-models/resolve/main/lang-id-voxlingua107-ecapa-Q8_0.gguf"
        );
        assert_eq!(info.sha256.as_deref().map(str::len), Some(64));

        let temp_dir = TempDir::new().unwrap();
        assert_eq!(
            langid_model_path(temp_dir.path()),
            temp_dir.path().join(LANGID_FILENAME)
        );
        let manager = ModelManager::new(temp_dir.path()).unwrap();
        assert_eq!(manager.langid_model_path(), None);
        std::fs::write(temp_dir.path().join(LANGID_FILENAME), b"x").unwrap();
        assert_eq!(
            manager.langid_model_path(),
            None,
            "a damaged file is no model"
        );
        assert!(
            manager.list_cached_models().is_empty(),
            "the language-ID model is not a Whisper model"
        );
    }

    fn hello_model() -> ModelInfo {
        ModelInfo {
            name: "hello".to_string(),
            repo_id: "test/test".to_string(),
            filename: "hello.bin".to_string(),
            expected_size: 13,
            sha256: Some(
                "dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f".to_string(),
            ),
        }
    }

    #[test]
    fn a_download_that_fails_its_checksum_is_deleted_and_never_used() {
        let temp_dir = TempDir::new().unwrap();
        let info = hello_model();
        let partial = temp_dir.path().join("hello.bin.part");
        let target = temp_dir.path().join("hello.bin");
        std::fs::write(&partial, b"Hello, World?").unwrap();
        let err = ModelManager::promote(&partial, &target, &info).unwrap_err();
        assert!(err.downcast_ref::<ChecksumMismatch>().is_some(), "{err}");
        assert!(err.to_string().contains("SHA-256"), "{err}");
        assert!(!partial.exists(), "the bad download is deleted");
        assert!(
            !target.exists(),
            "the bad download never takes the model's place"
        );
    }

    #[test]
    fn a_download_that_passes_its_checksum_takes_its_place() {
        let temp_dir = TempDir::new().unwrap();
        let info = hello_model();
        let partial = temp_dir.path().join("hello.bin.part");
        let target = temp_dir.path().join("hello.bin");
        std::fs::write(&partial, b"Hello, World!").unwrap();
        ModelManager::promote(&partial, &target, &info).unwrap();
        assert!(!partial.exists());
        let manager = ModelManager::new(temp_dir.path()).unwrap();
        assert_eq!(manager.valid_path(&info), Some(target.clone()));
        std::fs::write(&target, b"Hello, World?").unwrap();
        let later = SystemTime::now() + std::time::Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(&target)
            .unwrap()
            .set_modified(later)
            .unwrap();
        assert_eq!(
            manager.valid_path(&info),
            None,
            "a file damaged later fails"
        );
    }

    #[test]
    fn a_short_download_is_no_checksum_mismatch() {
        let temp_dir = TempDir::new().unwrap();
        let partial = temp_dir.path().join("hello.bin.part");
        std::fs::write(&partial, b"Hello").unwrap();
        let err =
            ModelManager::promote(&partial, &temp_dir.path().join("hello.bin"), &hello_model())
                .unwrap_err();
        assert!(err.downcast_ref::<ChecksumMismatch>().is_none(), "{err}");
        assert!(!partial.exists());
    }

    #[test]
    fn a_check_that_cannot_read_the_file_is_not_kept() {
        use std::os::unix::fs::PermissionsExt;
        // SAFETY: geteuid has no preconditions.
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        let temp_dir = TempDir::new().unwrap();
        let info = hello_model();
        let path = temp_dir.path().join("hello.bin");
        std::fs::write(&path, b"Hello, World!").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
        let manager = ModelManager::new(temp_dir.path()).unwrap();
        assert_eq!(manager.valid_path(&info), None, "unreadable");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            manager.valid_path(&info),
            Some(path),
            "a read error is not remembered as a bad file"
        );
    }

    #[test]
    fn a_file_is_hashed_again_only_when_its_stamp_changes() {
        let temp_dir = TempDir::new().unwrap();
        let info = hello_model();
        let path = temp_dir.path().join("hello.bin");
        std::fs::write(&path, b"Hello, World!").unwrap();
        let manager = ModelManager::new(temp_dir.path()).unwrap();
        assert_eq!(manager.valid_path(&info), Some(path.clone()));
        let stamp = FileStamp::of(&path).unwrap();
        let file = std::fs::File::options().write(true).open(&path).unwrap();
        std::io::Write::write_all(&mut &file, b"Hello, World?").unwrap();
        file.set_modified(stamp.modified).unwrap();
        assert_eq!(
            manager.valid_path(&info),
            Some(path.clone()),
            "the same size and time: the cached check stands, with no hash"
        );
        file.set_modified(stamp.modified + std::time::Duration::from_secs(1))
            .unwrap();
        assert_eq!(manager.valid_path(&info), None, "a new time: hashed again");
        std::fs::remove_file(&path).unwrap();
        assert_eq!(manager.valid_path(&info), None, "a missing file");
    }

    #[test]
    fn parakeet_comes_from_the_hush_model_repo() {
        let size: ModelSize = "parakeet-v3".parse().unwrap();
        assert_eq!(size, ModelSize::ParakeetV3);
        assert_eq!(size.to_string(), "parakeet-v3");
        assert!(size.is_parakeet());
        assert!(!ModelSize::Base.is_parakeet());
        let temp_dir = TempDir::new().unwrap();
        let manager = ModelManager::new(temp_dir.path()).unwrap();
        let info = manager.get_model_info(&size).unwrap();
        assert_eq!(info.filename, size.filename());
        assert_eq!(
            ModelManager::download_url(info),
            "https://huggingface.co/collectivecontext/hush-models/resolve/main/parakeet-tdt-0.6b-v3-Q8_0.gguf"
        );
        assert_eq!(info.expected_size, 739_508_576);
        assert_eq!(
            info.sha256.as_deref(),
            Some("5859f77944efcd8eafa23a6350731960b2b55b2203df51f319665c807d802cc7")
        );
        assert_eq!(PARAKEET_V3_LANGUAGES.len(), 25);
        assert!(PARAKEET_V3_LANGUAGES
            .iter()
            .all(|code| crate::transcription::Language::parse(code).is_some()));
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

    fn gguf_string(bytes: &mut Vec<u8>, text: &str) {
        bytes.extend((text.len() as u64).to_le_bytes());
        bytes.extend(text.as_bytes());
    }

    /// A GGUF v3 header with no tensors, whose pairs come from `pairs`.
    fn gguf(pairs: u64, write: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
        let mut bytes = b"GGUF".to_vec();
        bytes.extend(3u32.to_le_bytes());
        bytes.extend(0u64.to_le_bytes());
        bytes.extend(pairs.to_le_bytes());
        write(&mut bytes);
        bytes
    }

    #[test]
    fn the_architecture_comes_from_the_gguf_header() {
        let parakeet = gguf(1, |bytes| {
            gguf_string(bytes, "general.architecture");
            bytes.extend(GGUF_STRING.to_le_bytes());
            gguf_string(bytes, "parakeet");
        });
        assert_eq!(
            architecture_in(&mut parakeet.as_slice()).as_deref(),
            Some("parakeet")
        );

        let later = gguf(4, |bytes| {
            gguf_string(bytes, "general.alignment");
            bytes.extend(4u32.to_le_bytes());
            bytes.extend(32u32.to_le_bytes());
            gguf_string(bytes, "general.name");
            bytes.extend(GGUF_STRING.to_le_bytes());
            gguf_string(bytes, "Whisper Base");
            gguf_string(bytes, "tokenizer.tokens");
            bytes.extend(GGUF_ARRAY.to_le_bytes());
            bytes.extend(GGUF_STRING.to_le_bytes());
            bytes.extend(2u64.to_le_bytes());
            gguf_string(bytes, "hello");
            gguf_string(bytes, "world");
            gguf_string(bytes, "general.architecture");
            bytes.extend(GGUF_STRING.to_le_bytes());
            gguf_string(bytes, "whisper");
        });
        assert_eq!(
            architecture_in(&mut later.as_slice()).as_deref(),
            Some("whisper"),
            "the pairs before it are skipped"
        );
    }

    #[test]
    fn a_ggml_bin_is_whisper_and_other_files_have_no_architecture() {
        let mut bin = b"lmgg".to_vec();
        bin.extend(51_865u32.to_le_bytes());
        assert_eq!(
            architecture_in(&mut bin.as_slice()).as_deref(),
            Some("whisper")
        );

        assert_eq!(architecture_in(&mut &b"PK\x03\x04"[..]), None, "a zip");
        assert_eq!(architecture_in(&mut &b"GG"[..]), None, "too short");
        let no_name = gguf(1, |bytes| {
            gguf_string(bytes, "general.name");
            bytes.extend(GGUF_STRING.to_le_bytes());
            gguf_string(bytes, "Whisper Base");
        });
        assert_eq!(architecture_in(&mut no_name.as_slice()), None);
        let cut = gguf(1, |bytes| gguf_string(bytes, "general.archi"));
        assert_eq!(architecture_in(&mut cut.as_slice()), None, "a cut file");
    }

    #[test]
    fn a_file_on_disk_names_its_architecture() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("model.gguf");
        assert_eq!(file_architecture(&path), None, "a missing file");
        let parakeet = gguf(1, |bytes| {
            gguf_string(bytes, "general.architecture");
            bytes.extend(GGUF_STRING.to_le_bytes());
            gguf_string(bytes, "parakeet");
        });
        std::fs::write(&path, parakeet).unwrap();
        assert_eq!(file_architecture(&path).as_deref(), Some("parakeet"));
    }
}
