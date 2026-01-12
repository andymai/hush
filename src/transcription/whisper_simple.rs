use crate::transcription::cuda::CudaAvailability;
/// Simple working Whisper implementation using whisper-rs
/// This provides real speech-to-text without the complexity of PyTorch model conversion
use crate::Result;
use parking_lot::Mutex;
use std::path::Path;
use tracing::{info, warn};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

#[derive(Debug, Clone)]
pub struct SimpleTranscriptionResult {
    pub text: String,
    pub confidence: f32,
    pub processing_time: std::time::Duration,
}

/// Holds both the WhisperContext and WhisperState together.
/// The context is kept alive to support the state. The state is cached and reused
/// across transcriptions to avoid GPU memory allocation per call.
struct WhisperContextWithState {
    /// The whisper context - kept alive to support state operations.
    #[allow(dead_code)]
    context: WhisperContext,
    /// Cached state - reused across transcriptions to avoid GPU memory allocation.
    /// Wrapped in Mutex because WhisperState::full() requires &mut self.
    state: Mutex<WhisperState>,
}

impl WhisperContextWithState {
    fn new(context: WhisperContext) -> Result<Self> {
        // Create state from the context - this pre-allocates GPU buffers
        let state = context
            .create_state()
            .map_err(|e| anyhow::anyhow!("Failed to create whisper state: {}", e))?;

        Ok(Self {
            context,
            state: Mutex::new(state),
        })
    }
}

pub struct SimpleWhisperTranscriber {
    /// Combined context and cached state for GPU memory efficiency.
    context_with_state: Option<WhisperContextWithState>,
    model_path: std::path::PathBuf,
    ready: bool,
}

impl SimpleWhisperTranscriber {
    /// Create a new simple Whisper transcriber
    pub async fn new(model_path: &Path) -> Result<Self> {
        info!("🎤 Initializing Simple Whisper transcriber");
        info!("Model path: {:?}", model_path);

        let mut transcriber = SimpleWhisperTranscriber {
            context_with_state: None,
            model_path: model_path.to_path_buf(),
            ready: false,
        };

        // Try to load the model
        match transcriber.load_model().await {
            Ok(()) => {
                info!("✅ Simple Whisper transcriber ready");
                transcriber.ready = true;
            },
            Err(e) => {
                warn!("⚠️ Failed to load Whisper model: {}", e);
                warn!("Will fall back to simulation mode");
                transcriber.ready = false;
            },
        }

        Ok(transcriber)
    }

    /// Load the Whisper model and pre-create the state for GPU memory efficiency
    async fn load_model(&mut self) -> Result<()> {
        // Check if model file exists
        if !self.model_path.exists() {
            return Err(anyhow::anyhow!(
                "Model file not found at {:?}. Please download a compatible GGML model.",
                self.model_path
            ));
        }

        info!("Loading Whisper model from {:?}", self.model_path);

        // Create whisper context with GPU acceleration if available
        let cuda = CudaAvailability::detect();
        let mut ctx_params = WhisperContextParameters::default();
        ctx_params.use_gpu(cuda.available);
        if cuda.available {
            info!("GPU acceleration enabled for Whisper context");
        } else {
            info!("Using CPU for Whisper context");
        }

        let context =
            WhisperContext::new_with_params(self.model_path.to_string_lossy().as_ref(), ctx_params)
                .map_err(|e| anyhow::anyhow!("Failed to create Whisper context: {}", e))?;

        // Create combined context and state - this pre-allocates GPU buffers once
        let context_with_state = WhisperContextWithState::new(context)?;
        self.context_with_state = Some(context_with_state);

        info!("✅ Whisper model and state loaded successfully (GPU buffers pre-allocated)");

        Ok(())
    }

    /// Check if the transcriber is ready for use
    pub fn is_ready(&self) -> bool {
        self.ready
    }

    /// Transcribe audio samples (16kHz, mono, f32)
    ///
    /// Uses a cached WhisperState to avoid GPU memory allocation per transcription.
    /// The state is reused across calls, significantly reducing GPU memory pressure.
    pub async fn transcribe(&self, audio_data: &[f32]) -> Result<SimpleTranscriptionResult> {
        let start_time = std::time::Instant::now();

        if !self.ready {
            return Err(anyhow::anyhow!("Transcriber not ready"));
        }

        let ctx_with_state = self
            .context_with_state
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Whisper context not initialized"))?;

        info!(
            "🎤 Transcribing {} samples of audio (using cached state)",
            audio_data.len()
        );

        // Create parameters for transcription - using Greedy sampling for speed
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 0 });
        params.set_language(Some("en"));
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_n_threads(1); // Use single thread for now

        // Reuse the cached state - this avoids GPU memory allocation per transcription
        let mut state = ctx_with_state.state.lock();

        // Run the transcription
        state
            .full(params, audio_data)
            .map_err(|e| anyhow::anyhow!("Transcription failed: {}", e))?;

        // Extract the results using the iterator API
        let mut full_text = String::new();
        let mut segment_count = 0;

        // Use the new iterator API to get segments
        for segment in state.as_iter() {
            let segment_text = segment.to_string();
            if !segment_text.trim().is_empty() {
                if !full_text.is_empty() {
                    full_text.push(' ');
                }
                full_text.push_str(segment_text.trim());
                segment_count += 1;
            }
        }

        // Drop the lock before logging to minimize lock hold time
        drop(state);

        // Simple confidence estimation based on whether we got meaningful text
        let confidence = if full_text.trim().is_empty() {
            0.0
        } else if segment_count > 0 {
            0.9 // High confidence when we get segments
        } else {
            0.3 // Lower confidence for minimal output
        };

        let result = SimpleTranscriptionResult {
            text: full_text.trim().to_string(),
            confidence,
            processing_time: start_time.elapsed(),
        };

        info!(
            "✅ Transcription completed: '{}' (confidence: {:.2})",
            result.text, result.confidence
        );

        Ok(result)
    }

    pub fn get_device_info(&self) -> String {
        "whisper.cpp (GPU-accelerated)".to_string()
    }
}
