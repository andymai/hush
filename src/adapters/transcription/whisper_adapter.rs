/// Adapter for WhisperTranscriber to implement Transcriber trait

use crate::transcription::WhisperTranscriber;
use crate::core::traits::{Transcriber, TranscriptionResult, TranscriberInfo, AudioBuffer};
use crate::Result;
use async_trait::async_trait;
use std::time::Duration;

/// Adapter that wraps WhisperTranscriber to implement Transcriber trait
pub struct WhisperAdapter {
    inner: WhisperTranscriber,
}

impl WhisperAdapter {
    /// Create new adapter wrapping a WhisperTranscriber instance
    pub async fn new(model_path: &std::path::Path, use_cuda: bool) -> Result<Self> {
        let inner = WhisperTranscriber::new(model_path, use_cuda).await?;
        Ok(Self { inner })
    }

    /// Get reference to inner WhisperTranscriber (for migration period)
    pub fn inner(&self) -> &WhisperTranscriber {
        &self.inner
    }
}

#[async_trait]
impl Transcriber for WhisperAdapter {
    async fn transcribe(&self, audio: &AudioBuffer) -> Result<TranscriptionResult> {
        // Call the inner transcriber with the audio samples
        let result = self.inner
            .transcribe_async(&audio.samples, audio.sample_rate)
            .await?;

        // Convert legacy TranscriptionResult to new one
        Ok(TranscriptionResult {
            text: result.text,
            confidence: result.confidence,
            language: None, // Legacy result doesn't have language
            processing_time: result.processing_time,
        })
    }

    fn info(&self) -> TranscriberInfo {
        TranscriberInfo {
            name: "Whisper".to_string(),
            version: "1.0.0".to_string(),
            supports_languages: vec!["en".to_string(), "es".to_string(), "fr".to_string()],
            max_audio_duration: Some(Duration::from_secs(30)),
            requires_network: false,
            hardware_accelerated: self.inner.is_using_cuda(),
        }
    }

    async fn is_ready(&self) -> bool {
        // Whisper is ready if it was successfully initialized
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whisper_adapter_compiles() {
        // This test verifies the adapter compiles and implements the trait
        // Actual functionality is tested via integration tests with real models

        // Type check: ensure we can Box the trait object
        fn _type_check(adapter: WhisperAdapter) -> Box<dyn Transcriber> {
            Box::new(adapter)
        }
    }
}
