use crate::core::traits::{AudioBuffer, Transcriber, TranscriberInfo, TranscriptionResult};
/// Adapter for WhisperTranscriber to implement Transcriber trait
use crate::transcription::device::GpuAvailability;
use crate::transcription::WhisperTranscriber;
use crate::Result;
use async_trait::async_trait;
use std::time::Duration;
use tracing::info;

/// Adapter that wraps WhisperTranscriber to implement Transcriber trait
pub struct WhisperAdapter {
    inner: WhisperTranscriber,
    gpu_type: crate::transcription::device::GpuType,
}

impl WhisperAdapter {
    /// Create new adapter with automatic GPU detection
    pub async fn new(model_path: &std::path::Path) -> Result<Self> {
        // Detect GPU availability
        let gpu = GpuAvailability::detect();
        info!("Initializing WhisperAdapter with: {}", gpu);

        // Create transcriber with detected GPU
        let use_gpu = gpu.available;
        let inner = WhisperTranscriber::new(model_path, use_gpu).await?;

        Ok(Self {
            inner,
            gpu_type: gpu.gpu_type,
        })
    }

    /// Create adapter with explicit GPU preference (for testing/debugging)
    #[allow(dead_code)]
    pub async fn new_with_gpu_preference(
        model_path: &std::path::Path,
        use_gpu: bool,
    ) -> Result<Self> {
        let inner = WhisperTranscriber::new(model_path, use_gpu).await?;
        let gpu_type = if use_gpu {
            GpuAvailability::gpu_type()
        } else {
            crate::transcription::device::GpuType::Cpu
        };

        Ok(Self { inner, gpu_type })
    }

    /// Get reference to inner WhisperTranscriber (for migration period)
    pub fn inner(&self) -> &WhisperTranscriber {
        &self.inner
    }

    /// Get the GPU type being used
    pub fn gpu_type(&self) -> crate::transcription::device::GpuType {
        self.gpu_type
    }
}

#[async_trait]
impl Transcriber for WhisperAdapter {
    async fn transcribe(&self, audio: &AudioBuffer) -> Result<TranscriptionResult> {
        let result = self
            .inner
            .transcribe_async(&audio.samples, audio.sample_rate)
            .await?;

        Ok(TranscriptionResult {
            text: result.text,
            confidence: result.confidence,
            language: None,
            processing_time: result.processing_time,
        })
    }

    fn info(&self) -> TranscriberInfo {
        use crate::transcription::device::GpuType;

        let hardware_accelerated = !matches!(self.gpu_type, GpuType::Cpu);

        TranscriberInfo {
            name: match self.gpu_type {
                GpuType::Cuda => "Whisper (CUDA)".to_string(),
                GpuType::Cpu => "Whisper (CPU)".to_string(),
            },
            version: "1.0.0".to_string(),
            supports_languages: vec!["en".to_string(), "es".to_string(), "fr".to_string()],
            max_audio_duration: Some(Duration::from_secs(30)),
            requires_network: false,
            hardware_accelerated,
        }
    }

    async fn is_ready(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whisper_adapter_compiles() {
        fn _type_check(adapter: WhisperAdapter) -> Box<dyn Transcriber> {
            Box::new(adapter)
        }
    }
}
