//! The transcriber the config asks for: Whisper through transcribe.cpp on
//! this machine, or the service or server the user chose. An HTTP backend
//! never loads the model, and the local one opens a connection only to fetch
//! a missing language-ID model.
//!
//! Every transcript then goes through the checks in [`guard`]: a transcript
//! that echoes the prompt is made again without the prompt, and text that
//! Whisper invents for silence is dropped.

use crate::config::settings::TranscriptionConfig;
use crate::transcription::cloud::CloudTranscriber;
use crate::transcription::cpu;
use crate::transcription::guard::{self, EchoChoice};
use crate::transcription::language::Language;
use crate::transcription::models::{self, ModelManager, ModelSize};
use crate::transcription::whisper::{TranscriptionResult, WhisperTranscriber};
use crate::Result;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// A prompt longer than this is cut when no tokenizer can count it. Whisper
/// keeps about 224 tokens, and even names and paths make fewer tokens than
/// half their characters.
pub const PROMPT_MAX_CHARS: usize = 400;

pub struct SpeechToText {
    backend: Backend,
    drop_phrases: Vec<String>,
}

enum Backend {
    Local(WhisperTranscriber),
    Cloud(CloudTranscriber),
}

impl SpeechToText {
    /// Load the local model from `models_dir`, or prepare the client for the
    /// chosen service with its key from the environment or `env_file`. Call
    /// [`crate::config::keys::load_env_files`] first when the key may live
    /// in `./.env`.
    pub async fn from_config(
        config: &TranscriptionConfig,
        models_dir: &Path,
        env_file: &Path,
    ) -> Result<Self> {
        let drop_phrases = guard::drop_list(config.drop_phrases.as_deref());
        if config.backend.is_cloud() {
            return Ok(Self {
                backend: Backend::Cloud(CloudTranscriber::from_config(config, env_file)?),
                drop_phrases,
            });
        }
        cpu::ensure_supported()?;
        let model = local_model_path(config, models_dir);
        let mut whisper = WhisperTranscriber::new(&model, config.use_gpu)
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "{}\nDownload a model with: hush models download {}",
                    e,
                    config.model_size
                )
            })?
            .with_language(&config.language)
            .with_detect_languages(&config.detect_languages);
        if config.vad {
            load_vad(&mut whisper, models_dir, &config.model_size);
        }
        if !config.detect_languages.is_empty() {
            load_langid(&mut whisper, models_dir, &config.model_size).await;
        }
        Ok(Self {
            backend: Backend::Local(whisper),
            drop_phrases,
        })
    }

    pub fn is_cloud(&self) -> bool {
        matches!(self.backend, Backend::Cloud(_))
    }

    /// Where the audio goes, for logs and status lines.
    pub fn describe(&self) -> String {
        match &self.backend {
            Backend::Local(whisper) if whisper.has_vad() => format!(
                "Whisper on {}, speech found by Silero VAD",
                whisper.get_device_info()
            ),
            Backend::Local(whisper) => format!("Whisper on {}", whisper.get_device_info()),
            Backend::Cloud(cloud) => cloud.describe(),
        }
    }

    /// Whether `prompt` fits the part of Whisper's context that holds the
    /// prompt. The local model counts its tokens; for a cloud service, and
    /// when the tokenizer fails, a character limit stands in.
    pub fn prompt_fits(&self, prompt: &str) -> bool {
        match &self.backend {
            Backend::Local(whisper) => match whisper.prompt_tokens(prompt) {
                Some(tokens) => tokens <= whisper.prompt_token_budget(),
                None => prompt.chars().count() <= PROMPT_MAX_CHARS,
            },
            Backend::Cloud(_) => prompt.chars().count() <= PROMPT_MAX_CHARS,
        }
    }

    /// Transcribe, then check the text. When the text looks like an echo of
    /// `prompt`, transcribe again without it, and let [`guard::after_echo`]
    /// pick the result. Text that Whisper invents for silence comes back
    /// empty.
    pub async fn transcribe_with_prompt(
        &self,
        audio: &[f32],
        sample_rate: u32,
        prompt: Option<&str>,
    ) -> Result<TranscriptionResult> {
        self.transcribe_in(audio, sample_rate, prompt, None).await
    }

    /// [`Self::transcribe_with_prompt`] in `language`; `None` is
    /// `transcription.language`.
    pub async fn transcribe_in(
        &self,
        audio: &[f32],
        sample_rate: u32,
        prompt: Option<&str>,
        language: Option<Language>,
    ) -> Result<TranscriptionResult> {
        self.transcribe_part(audio, sample_rate, prompt, prompt, language)
            .await
    }

    /// [`Self::transcribe_in`] for one part of a take. Whisper gets
    /// `prompt`, which starts with `terms` (the window title and the learned
    /// terms) and ends with the text before the part. Speech can repeat that
    /// text, so the echo check counts it only when one of its words loops.
    pub async fn transcribe_part(
        &self,
        audio: &[f32],
        sample_rate: u32,
        prompt: Option<&str>,
        terms: Option<&str>,
        language: Option<Language>,
    ) -> Result<TranscriptionResult> {
        let prompt = prompt.map(str::trim).filter(|p| !p.is_empty());
        let terms = terms.map(str::trim).filter(|t| !t.is_empty());
        let mut result = self
            .transcribe_once(audio, sample_rate, prompt, language)
            .await?;

        if let Some(echo) = guard::echo_in(&result.text, prompt, terms) {
            info!("The transcript looks like an echo of the prompt, transcribing again without it");
            let detected = result.detected.and_then(Language::parse);
            let retry = self
                .transcribe_once(audio, sample_rate, None, detected.or(language))
                .await?;
            result.processing_time += retry.processing_time;
            match echo.choose(&result.text, &retry.text) {
                EchoChoice::Drop => {
                    info!("Dropped the transcript: without the prompt, it had no speech");
                    result.text.clear();
                },
                EchoChoice::Retry => {
                    info!("Kept the transcript made without the prompt");
                    result.text = retry.text;
                    result.confidence = retry.confidence;
                },
                EchoChoice::Original => {
                    info!("Kept the first transcript: the prompt terms in it were spoken");
                },
            }
        }

        if !result.text.is_empty() && guard::is_invented(&result.text, &self.drop_phrases) {
            info!("Dropped text that Whisper invents for silence");
            debug!(text = %result.text, "Dropped transcript");
            result.text.clear();
        }
        if result.text.is_empty() {
            result.confidence = 0.0;
        }
        Ok(result)
    }

    pub async fn transcribe_async(
        &self,
        audio: &[f32],
        sample_rate: u32,
    ) -> Result<TranscriptionResult> {
        self.transcribe_with_prompt(audio, sample_rate, None).await
    }

    async fn transcribe_once(
        &self,
        audio: &[f32],
        sample_rate: u32,
        prompt: Option<&str>,
        language: Option<Language>,
    ) -> Result<TranscriptionResult> {
        let language = language.map(Language::code);
        match &self.backend {
            Backend::Local(whisper) => {
                whisper
                    .transcribe_in(audio, sample_rate, prompt, language)
                    .await
            },
            Backend::Cloud(cloud) => {
                cloud
                    .transcribe_in(audio, sample_rate, prompt, language)
                    .await
            },
        }
    }
}

/// Load the Silero model when a copy that passes its checksum is there.
/// Without it, Whisper gets the whole take, and the log says once how to get
/// the model.
fn load_vad(whisper: &mut WhisperTranscriber, models_dir: &Path, model_size: &str) {
    let Some(path) = ModelManager::new(models_dir)
        .ok()
        .and_then(|manager| manager.vad_model_path())
    else {
        warn!(
            "Voice activity detection is off: {} is missing or damaged. Get it with: hush models download {}",
            models::vad_model_path(models_dir).display(),
            model_size
        );
        return;
    };
    if let Err(e) = whisper.load_vad(&path) {
        warn!(
            "Voice activity detection is off: {:#}. Get the model again with: hush models download {}",
            e, model_size
        );
    }
}

/// Load the language-ID model for `transcription.detect_languages`, and
/// download it first when it is missing. Without it, a take in `auto` lets
/// Whisper pick from every language.
async fn load_langid(whisper: &mut WhisperTranscriber, models_dir: &Path, model_size: &str) {
    let path = models::langid_model_path(models_dir);
    if !path.is_file() {
        info!("Downloading the language-ID model that detect_languages needs");
        let downloaded = match ModelManager::new(models_dir) {
            Ok(manager) => manager.ensure_langid_downloaded_with(&|_, _| {}).await,
            Err(e) => Err(e),
        };
        if let Err(e) = downloaded {
            warn!(
                "Limited language detection is off: the language-ID model did not download \
                 ({:#}). Whisper picks from every language. Get the model with: hush models download {}",
                e, model_size
            );
            return;
        }
    }
    if let Err(e) = whisper.load_langid(&path) {
        warn!(
            "Limited language detection is off: {:#}. Whisper picks from every language",
            e
        );
    }
}

/// The configured model file, or the catalogue's copy of `model_size` when
/// that file is absent.
fn local_model_path(config: &TranscriptionConfig, models_dir: &Path) -> PathBuf {
    let configured = config.model_path_in(models_dir);
    if configured.exists() {
        return configured;
    }
    let size: ModelSize = config.model_size.parse().unwrap_or(ModelSize::Base);
    ModelManager::new(models_dir)
        .ok()
        .and_then(|manager| manager.get_model_path(&size))
        .unwrap_or(configured)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::TranscriptionBackend;
    use crate::config::Config;
    use tempfile::TempDir;

    async fn engine_in(config: &TranscriptionConfig, home: &TempDir) -> Result<SpeechToText> {
        let models = home.path().join("models");
        SpeechToText::from_config(config, &models, &home.path().join(".env")).await
    }

    #[tokio::test]
    async fn a_cloud_backend_needs_its_key_and_never_loads_the_model() {
        let home = TempDir::new().unwrap();
        let mut config = Config::defaults().unwrap().transcription;
        config.backend = TranscriptionBackend::Groq;
        config.model_path = Some("/nonexistent/ggml-base.bin".into());
        match engine_in(&config, &home).await {
            Ok(engine) => {
                assert!(engine.is_cloud(), "a key in the environment was used");
                assert!(engine.describe().contains("audio leaves this machine"));
            },
            Err(e) => {
                let message = e.to_string();
                assert!(message.contains("GROQ_API_KEY"), "{message}");
                assert!(
                    !message.contains("hush models download"),
                    "the local model was not touched: {message}"
                );
            },
        }
    }

    #[tokio::test]
    async fn a_server_backend_never_loads_the_model_and_says_where_it_runs() {
        let mut config = Config::defaults().unwrap().transcription;
        config.backend = TranscriptionBackend::OpenAiCompatible;
        config.server_url = "http://127.0.0.1:9/v1".into();
        config.server_model = "whisper-1".into();
        config.model_path = Some("/nonexistent/ggml-base.bin".into());
        let engine = engine_in(&config, &TempDir::new().unwrap()).await.unwrap();
        assert!(engine.is_cloud());
        assert_eq!(
            engine.describe(),
            "whisper-1 on the local server 127.0.0.1:9"
        );
    }

    #[tokio::test]
    async fn a_failing_server_reports_its_error_and_nothing_else_runs() {
        let mut config = Config::defaults().unwrap().transcription;
        config.backend = TranscriptionBackend::OpenAiCompatible;
        config.server_url = "http://127.0.0.1:1".into();
        config.server_model = "whisper-1".into();
        let engine = engine_in(&config, &TempDir::new().unwrap()).await.unwrap();
        let message = engine
            .transcribe_with_prompt(&[0.1; 160], 16_000, None)
            .await
            .err()
            .map(|e| format!("{:#}", e))
            .unwrap_or_default();
        assert!(message.contains("127.0.0.1:1 did not answer"), "{message}");
    }

    #[tokio::test]
    async fn the_local_backend_names_the_download_when_the_model_is_missing() {
        let home = TempDir::new().unwrap();
        let mut config = Config::defaults().unwrap().transcription;
        config.model_path = Some("/nonexistent/ggml-base.bin".into());
        config.model_size = "large-v3".into();
        let message = engine_in(&config, &home)
            .await
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(
            message.contains("hush models download large-v3"),
            "{message}"
        );
    }

    fn cloud(drop_phrases: Option<Vec<String>>) -> SpeechToText {
        let backend = CloudTranscriber::new(
            TranscriptionBackend::Groq,
            "whisper-large-v3-turbo",
            "en",
            Some("gsk_test".to_string()),
        )
        .unwrap();
        SpeechToText {
            backend: Backend::Cloud(backend),
            drop_phrases: guard::drop_list(drop_phrases.as_deref()),
        }
    }

    #[test]
    fn a_cloud_prompt_is_capped_by_characters() {
        let engine = cloud(None);
        assert!(engine.prompt_fits(&"x".repeat(PROMPT_MAX_CHARS)));
        assert!(!engine.prompt_fits(&"x".repeat(PROMPT_MAX_CHARS + 1)));
    }

    #[tokio::test]
    async fn an_empty_take_needs_no_request_and_no_retry() {
        let result = cloud(None)
            .transcribe_with_prompt(&[], 16_000, Some("Kubernetes, PostgreSQL."))
            .await
            .unwrap();
        assert!(result.text.is_empty());
        assert_eq!(result.confidence, 0.0);
    }
}
