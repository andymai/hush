//! Speech-to-text through whisper.cpp, via whisper-rs.

use crate::logging::{transcription as logging, RequestContext};
use crate::transcription::device::GpuAvailability;
use crate::Result;
use parking_lot::Mutex;
use rubato::{Fft, FixedSync, Indexing, Resampler};
use std::path::Path;
use std::sync::{Arc, Once};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

/// Sample rate whisper.cpp expects
pub const WHISPER_SAMPLE_RATE: u32 = 16_000;

#[derive(Debug, Clone)]
pub struct TranscriptionResult {
    pub text: String,
    pub confidence: f32,
    pub processing_time: Duration,
}

/// The context must outlive the state. The state is created once so that its
/// GPU buffers are reused across dictations instead of reallocated per call.
struct Engine {
    #[allow(dead_code)]
    context: WhisperContext,
    state: Mutex<WhisperState>,
}

pub struct WhisperTranscriber {
    engine: Arc<Engine>,
    gpu: &'static GpuAvailability,
    using_gpu: bool,
    language: String,
    threads: i32,
}

impl WhisperTranscriber {
    /// Load a ggml model. `use_gpu` is honored when a GPU backend exists.
    pub async fn new(model_path: &Path, use_gpu: bool) -> Result<Self> {
        let ctx = RequestContext::new("whisper_transcriber_init")
            .with_metadata("model_path", &model_path.display().to_string())
            .with_metadata("gpu_requested", &use_gpu.to_string());
        logging::log_model_loading(&model_path.display().to_string(), use_gpu);

        if !model_path.is_file() {
            return Err(anyhow::anyhow!(
                "Model file not found: {}",
                model_path.display()
            ));
        }

        // Route whisper.cpp and ggml log lines through tracing instead of stderr.
        static LOGGING_HOOKS: Once = Once::new();
        LOGGING_HOOKS.call_once(whisper_rs::install_logging_hooks);

        let gpu = GpuAvailability::detect();
        let using_gpu = use_gpu && gpu.available;
        if use_gpu && !gpu.available {
            warn!("GPU acceleration requested but no GPU backend is available; using CPU");
        }
        info!(
            request_id = %ctx.request_id,
            model_path = %model_path.display(),
            device = %gpu,
            using_gpu,
            "Loading Whisper model"
        );

        let path = model_path
            .to_str()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Model path contains invalid UTF-8: {}",
                    model_path.display()
                )
            })?
            .to_owned();
        let mut params = WhisperContextParameters::default();
        params.use_gpu(using_gpu);

        // Loading a large model takes seconds, so keep it off the async runtime.
        let context =
            tokio::task::spawn_blocking(move || WhisperContext::new_with_params(&path, params))
                .await
                .map_err(|e| anyhow::anyhow!("Model loading task failed: {}", e))?
                .map_err(|e| {
                    anyhow::anyhow!(
                        "Failed to load Whisper model {}: {}",
                        model_path.display(),
                        e
                    )
                })?;
        let state = context
            .create_state()
            .map_err(|e| anyhow::anyhow!("Failed to create whisper state: {}", e))?;

        info!("Whisper model loaded on {}", gpu);

        Ok(Self {
            engine: Arc::new(Engine {
                context,
                state: Mutex::new(state),
            }),
            gpu,
            using_gpu,
            language: "en".to_string(),
            threads: if using_gpu { 1 } else { cpu_threads() },
        })
    }

    /// Set the spoken language as a Whisper language code. `auto` or an empty
    /// value lets the model detect it.
    pub fn with_language(mut self, language: &str) -> Self {
        let language = language.trim();
        self.language = if language.is_empty() {
            "auto".to_string()
        } else {
            language.to_string()
        };
        self
    }

    pub fn language(&self) -> &str {
        &self.language
    }

    pub async fn transcribe_async(
        &self,
        audio_data: &[f32],
        sample_rate: u32,
    ) -> Result<TranscriptionResult> {
        self.transcribe_with_prompt(audio_data, sample_rate, None)
            .await
    }

    /// Transcribe with an initial prompt that primes Whisper with names and
    /// spellings (the window title, learned terms).
    pub async fn transcribe_with_prompt(
        &self,
        audio_data: &[f32],
        sample_rate: u32,
        prompt: Option<&str>,
    ) -> Result<TranscriptionResult> {
        let start = Instant::now();
        let ctx = RequestContext::new("whisper_transcription")
            .with_metadata("samples", &audio_data.len().to_string())
            .with_metadata("sample_rate", &sample_rate.to_string());

        if audio_data.is_empty() {
            warn!(request_id = %ctx.request_id, "Empty audio data provided for transcription");
            return Ok(TranscriptionResult {
                text: String::new(),
                confidence: 0.0,
                processing_time: start.elapsed(),
            });
        }

        let duration = audio_data.len() as f32 / sample_rate as f32;
        logging::log_transcription_started(&ctx, duration, sample_rate);
        debug!(
            request_id = %ctx.request_id,
            samples = audio_data.len(),
            sample_rate,
            duration_sec = duration,
            "Processing audio for transcription"
        );

        let samples = if sample_rate == WHISPER_SAMPLE_RATE {
            audio_data.to_vec()
        } else {
            resample(audio_data, sample_rate, WHISPER_SAMPLE_RATE)?
        };

        let engine = Arc::clone(&self.engine);
        let language = self.language.clone();
        let threads = self.threads;
        let prompt = prompt
            .map(|p| p.replace('\0', ""))
            .filter(|p| !p.trim().is_empty());
        let text = tokio::task::spawn_blocking(move || {
            run_full(&engine, &samples, &language, threads, prompt.as_deref())
        })
        .await
        .map_err(|e| anyhow::anyhow!("Transcription task failed: {}", e))?;

        let text = match text {
            Ok(text) => text,
            Err(e) => {
                logging::log_transcription_error(&ctx, &e);
                return Err(e);
            },
        };
        let confidence = if text.is_empty() { 0.0 } else { 0.9 };
        let processing_time = start.elapsed();

        logging::log_transcription_completed(&ctx, &text, confidence);
        info!(
            request_id = %ctx.request_id,
            text_length = text.len(),
            processing_time_ms = processing_time.as_millis(),
            "Transcription completed"
        );

        Ok(TranscriptionResult {
            text,
            confidence,
            processing_time,
        })
    }

    pub fn is_using_gpu(&self) -> bool {
        self.using_gpu
    }

    pub fn get_device_info(&self) -> String {
        if self.using_gpu {
            format!("GPU: {} ({})", self.gpu.device_name, self.gpu.gpu_type)
        } else {
            format!("CPU: {}", self.gpu.device_name)
        }
    }
}

fn run_full(
    engine: &Engine,
    samples: &[f32],
    language: &str,
    threads: i32,
    prompt: Option<&str>,
) -> Result<String> {
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some(language));
    if let Some(prompt) = prompt {
        params.set_initial_prompt(prompt);
    }
    params.set_translate(false);
    params.set_no_context(true);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_n_threads(threads);

    let mut state = engine.state.lock();
    state
        .full(params, samples)
        .map_err(|e| anyhow::anyhow!("Transcription failed: {}", e))?;

    let mut text = String::new();
    for segment in state.as_iter() {
        let segment = segment.to_string();
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(segment);
    }
    Ok(text)
}

fn cpu_threads() -> i32 {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(8) as i32
}

/// Sinc resampling through rubato, mono only.
pub fn resample(audio: &[f32], from_rate: u32, to_rate: u32) -> Result<Vec<f32>> {
    if from_rate == to_rate {
        return Ok(audio.to_vec());
    }

    use audioadapter_buffers::direct::SequentialSlice;

    let chunk_size = 1024;
    let mut resampler = Fft::<f32>::new(
        from_rate as usize,
        to_rate as usize,
        chunk_size,
        1,
        1,
        FixedSync::Both,
    )
    .map_err(|e| anyhow::anyhow!("Failed to create resampler: {}", e))?;

    let input_frames_next = resampler.input_frames_next();
    let mut padded_input = audio.to_vec();
    let remainder = padded_input.len() % input_frames_next;
    if remainder != 0 {
        padded_input.extend(vec![0.0f32; input_frames_next - remainder]);
    }

    let expected_len = (audio.len() as f64 * to_rate as f64 / from_rate as f64) as usize;
    let output_chunk_max = resampler.output_frames_max();
    let mut output = Vec::with_capacity(expected_len + output_chunk_max);

    for chunk in padded_input.chunks(input_frames_next) {
        let input_adapter = SequentialSlice::new(chunk, 1, chunk.len())
            .map_err(|e| anyhow::anyhow!("Failed to create input adapter: {}", e))?;
        let mut output_buf = vec![0.0f32; output_chunk_max];
        let mut output_adapter = SequentialSlice::new_mut(&mut output_buf, 1, output_chunk_max)
            .map_err(|e| anyhow::anyhow!("Failed to create output adapter: {}", e))?;
        let indexing = Indexing {
            input_offset: 0,
            output_offset: 0,
            active_channels_mask: None,
            partial_len: None,
        };
        let (_read, written) = resampler
            .process_into_buffer(&input_adapter, &mut output_adapter, Some(&indexing))
            .map_err(|e| anyhow::anyhow!("Resampling failed: {}", e))?;
        output.extend_from_slice(&output_buf[..written]);
    }

    output.truncate(expected_len);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn missing_model_is_an_error() {
        let err = WhisperTranscriber::new(Path::new("/nonexistent/ggml-base.bin"), false)
            .await
            .err()
            .expect("missing model must fail");
        assert!(err.to_string().contains("not found"), "{err}");
    }

    #[test]
    fn resample_scales_the_sample_count() {
        let input: Vec<f32> = (0..48_000).map(|i| (i as f32 * 0.01).sin()).collect();
        let output = resample(&input, 48_000, 16_000).unwrap();
        assert_eq!(output.len(), 16_000);
        assert!(output.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn resample_is_identity_for_equal_rates() {
        let input = vec![0.5f32; 100];
        assert_eq!(resample(&input, 16_000, 16_000).unwrap(), input);
    }
}
