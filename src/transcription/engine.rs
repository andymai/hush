//! The transcriber the config asks for: whisper.cpp on this machine, or the
//! cloud service the user chose. A cloud backend never loads the model, and
//! the local one never opens a connection.

use crate::config::paths;
use crate::config::settings::TranscriptionConfig;
use crate::transcription::cloud::CloudTranscriber;
use crate::transcription::cpu;
use crate::transcription::models::{ModelManager, ModelSize};
use crate::transcription::whisper::{TranscriptionResult, WhisperTranscriber};
use crate::Result;
use std::path::PathBuf;

pub enum SpeechToText {
    Local(WhisperTranscriber),
    Cloud(CloudTranscriber),
}

impl SpeechToText {
    /// Load the local model, or prepare the client for the chosen service.
    /// Call [`crate::config::keys::load_env_files`] first when the key may
    /// live in `./.env`.
    pub async fn from_config(config: &TranscriptionConfig) -> Result<Self> {
        if config.backend.is_cloud() {
            return CloudTranscriber::from_config(config).map(Self::Cloud);
        }
        cpu::ensure_supported()?;
        let whisper = WhisperTranscriber::new(&local_model_path(config), config.use_gpu)
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "{}\nDownload a model with: hush models download {}",
                    e,
                    config.model_size
                )
            })?;
        Ok(Self::Local(whisper.with_language(&config.language)))
    }

    pub fn is_cloud(&self) -> bool {
        matches!(self, Self::Cloud(_))
    }

    /// Where the audio goes, for logs and status lines.
    pub fn describe(&self) -> String {
        match self {
            Self::Local(whisper) => format!("Whisper on {}", whisper.get_device_info()),
            Self::Cloud(cloud) => format!("{}; audio leaves this machine", cloud.name()),
        }
    }

    pub async fn transcribe_with_prompt(
        &self,
        audio: &[f32],
        sample_rate: u32,
        prompt: Option<&str>,
    ) -> Result<TranscriptionResult> {
        match self {
            Self::Local(whisper) => {
                whisper
                    .transcribe_with_prompt(audio, sample_rate, prompt)
                    .await
            },
            Self::Cloud(cloud) => {
                cloud
                    .transcribe_with_prompt(audio, sample_rate, prompt)
                    .await
            },
        }
    }

    pub async fn transcribe_async(
        &self,
        audio: &[f32],
        sample_rate: u32,
    ) -> Result<TranscriptionResult> {
        self.transcribe_with_prompt(audio, sample_rate, None).await
    }
}

/// The configured model file, or the catalogue's copy of `model_size` when
/// that file is absent.
fn local_model_path(config: &TranscriptionConfig) -> PathBuf {
    let configured = config.model_path();
    if configured.exists() {
        return configured;
    }
    let size: ModelSize = config.model_size.parse().unwrap_or(ModelSize::Base);
    ModelManager::new(paths::models_dir())
        .ok()
        .and_then(|manager| manager.get_model_path(&size))
        .unwrap_or(configured)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::TranscriptionBackend;
    use crate::config::Config;

    #[tokio::test]
    async fn a_cloud_backend_needs_its_key_and_never_loads_the_model() {
        let mut config = Config::defaults().unwrap().transcription;
        config.backend = TranscriptionBackend::Groq;
        config.model_path = Some("/nonexistent/ggml-base.bin".into());
        match SpeechToText::from_config(&config).await {
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
    async fn the_local_backend_names_the_download_when_the_model_is_missing() {
        let mut config = Config::defaults().unwrap().transcription;
        config.model_path = Some("/nonexistent/ggml-base.bin".into());
        config.model_size = "large-v3".into();
        if local_model_path(&config).exists() {
            return;
        }
        let message = SpeechToText::from_config(&config)
            .await
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert!(
            message.contains("hush models download large-v3"),
            "{message}"
        );
    }
}
