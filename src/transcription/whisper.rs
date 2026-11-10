//! Speech-to-text through whisper.cpp, via whisper-rs.

use crate::audio::level;
use crate::logging::{transcription as logging, RequestContext};
use crate::transcription::cpu;
use crate::transcription::device::GpuAvailability;
use crate::transcription::gpu_fallback;
use crate::transcription::language;
use crate::Result;
use anyhow::Context;
use parking_lot::Mutex;
use rubato::{Fft, FixedSync, Indexing, Resampler, WindowFunction};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Once};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
    WhisperVadContext, WhisperVadContextParams, WhisperVadParams,
};

/// Sample rate whisper.cpp expects
pub const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// Audio kept on each side of a stretch of speech, so that soft first and
/// last sounds of a word stay in.
const VAD_SPEECH_PAD_MS: i32 = 100;

/// Shorter speech is dropped as noise. whisper.cpp uses 250 ms, which can
/// drop a dictated "no" or "OK".
const VAD_MIN_SPEECH_MS: i32 = 100;

/// Silence put between two stretches of speech, as whisper.cpp does.
const SPEECH_GAP: usize = WHISPER_SAMPLE_RATE as usize / 10;

/// Audio kept after each stretch of speech but the last, as whisper.cpp does.
const SPEECH_OVERLAP: usize = WHISPER_SAMPLE_RATE as usize / 10;

#[derive(Debug, Clone)]
pub struct TranscriptionResult {
    pub text: String,
    pub confidence: f32,
    pub processing_time: Duration,
}

/// The context must outlive the state. The state is created once so that its
/// GPU buffers are reused across dictations instead of reallocated per call.
struct Engine {
    context: WhisperContext,
    state: Mutex<WhisperState>,
}

pub struct WhisperTranscriber {
    /// A CPU engine takes the place of a GPU engine that fails a take.
    engine: Mutex<Arc<Engine>>,
    /// whisper-rs runs `whisper_full_with_state`, which ignores the VAD
    /// fields of `FullParams`, so Hush runs the Silero model itself.
    vad: Option<Arc<Mutex<WhisperVadContext>>>,
    gpu: &'static GpuAvailability,
    using_gpu: AtomicBool,
    model_path: PathBuf,
    language: String,
    /// The languages that detection may pick; empty allows every language.
    detect_languages: Arc<[String]>,
}

impl WhisperTranscriber {
    /// Load a ggml model. `use_gpu` is honored when a GPU backend exists,
    /// unless that GPU failed a take with the same driver before.
    pub async fn new(model_path: &Path, use_gpu: bool) -> Result<Self> {
        cpu::ensure_supported()?;
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

        gpu_fallback::wait_for_gpu(use_gpu).await;
        let gpu = GpuAvailability::detect();
        if use_gpu && !gpu.available {
            if gpu_fallback::GPU_BUILD {
                warn!(
                    "Whisper loads on the CPU, because ggml found no GPU. The graphics driver \
                     was probably not ready when Hush started, for example at login. \
                     Run `hush daemon restart` to use the GPU"
                );
            } else {
                warn!("GPU acceleration requested but no GPU backend is available; using CPU");
            }
        }
        let using_gpu = use_gpu && gpu.available && !gpu_fallback::avoid_gpu(gpu);
        info!(
            request_id = %ctx.request_id,
            model_path = %model_path.display(),
            device = %gpu,
            using_gpu,
            "Loading Whisper model"
        );

        let engine = load_engine(model_path, using_gpu).await?;
        let transcriber = Self {
            engine: Mutex::new(Arc::new(engine)),
            vad: None,
            gpu,
            using_gpu: AtomicBool::new(using_gpu),
            model_path: model_path.to_path_buf(),
            language: "en".to_string(),
            detect_languages: Arc::from([]),
        };
        info!("Whisper model loaded on {}", transcriber.get_device_info());
        Ok(transcriber)
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

    /// Limit detection to these languages: a take in `auto` gets the most
    /// likely of them. Empty allows every language.
    pub fn with_detect_languages(mut self, languages: &[String]) -> Self {
        self.detect_languages = languages
            .iter()
            .map(|code| code.trim().to_string())
            .filter(|code| !code.is_empty())
            .collect();
        self
    }

    /// Load the Silero voice activity detection model, so that Whisper gets
    /// only the speech in each take.
    pub fn load_vad(&mut self, model_path: &Path) -> Result<()> {
        let path = model_path.to_str().ok_or_else(|| {
            anyhow::anyhow!(
                "VAD model path contains invalid UTF-8: {}",
                model_path.display()
            )
        })?;
        let mut params = WhisperVadContextParams::new();
        params.set_n_threads(cpu_threads().min(4));
        let context = WhisperVadContext::new(path, params).map_err(|e| {
            anyhow::anyhow!(
                "Failed to load the VAD model {}: {}",
                model_path.display(),
                e
            )
        })?;
        self.vad = Some(Arc::new(Mutex::new(context)));
        info!(
            "Voice activity detection loaded from {}",
            model_path.display()
        );
        Ok(())
    }

    pub fn has_vad(&self) -> bool {
        self.vad.is_some()
    }

    /// How many tokens whisper.cpp makes of `prompt`, or `None` when it
    /// cannot tokenize it.
    pub fn prompt_tokens(&self, prompt: &str) -> Option<usize> {
        if prompt.contains('\0') {
            return None;
        }
        // whisper-rs reads a negative "too many tokens" answer as a length,
        // so the capacity must never run out: whisper.cpp makes at most one
        // token of each byte.
        self.engine()
            .context
            .tokenize(prompt, prompt.len() + 1)
            .ok()
            .map(|tokens| tokens.len())
    }

    /// The most prompt tokens whisper.cpp keeps: half of the text context,
    /// less the token that starts the prompt. It drops the rest from the
    /// front.
    pub fn prompt_token_budget(&self) -> usize {
        usize::try_from(self.engine().context.n_text_ctx() / 2 - 1).unwrap_or(0)
    }

    fn engine(&self) -> Arc<Engine> {
        Arc::clone(&self.engine.lock())
    }

    fn threads(&self) -> i32 {
        if self.is_using_gpu() {
            1
        } else {
            cpu_threads()
        }
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
        self.transcribe_in(audio_data, sample_rate, prompt, None)
            .await
    }

    /// [`Self::transcribe_with_prompt`] in `language`, a Whisper code or
    /// `auto`; `None` is the language this transcriber was made with.
    pub async fn transcribe_in(
        &self,
        audio_data: &[f32],
        sample_rate: u32,
        prompt: Option<&str>,
        language: Option<&str>,
    ) -> Result<TranscriptionResult> {
        let start = Instant::now();
        let language = language.unwrap_or(&self.language).to_string();
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

        let samples: Arc<[f32]> = if sample_rate == WHISPER_SAMPLE_RATE {
            Arc::from(audio_data)
        } else {
            resample(audio_data, sample_rate, WHISPER_SAMPLE_RATE)?.into()
        };
        let prompt = prompt
            .map(|p| p.replace('\0', ""))
            .filter(|p| !p.trim().is_empty());

        let text = match self
            .run_take(Arc::clone(&samples), prompt.clone(), language.clone())
            .await
        {
            Err(e) if self.is_using_gpu() => self.retry_on_cpu(e, samples, prompt, language).await,
            text => text,
        };
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

    async fn run_take(
        &self,
        samples: Arc<[f32]>,
        prompt: Option<String>,
        language: String,
    ) -> Result<String> {
        let engine = self.engine();
        let vad = self.vad.clone();
        let detect = Arc::clone(&self.detect_languages);
        let threads = self.threads();
        tokio::task::spawn_blocking(move || {
            let trimmed = vad.as_deref().and_then(|vad| match speech_in(vad, &samples) {
                Ok(speech) => Some(speech),
                Err(e) => {
                    warn!("Voice activity detection failed, so Whisper gets the whole take: {e:#}");
                    None
                },
            });
            let Some(samples) = whisper_input(&samples, trimmed.as_deref()) else {
                debug!("Voice activity detection found no speech in a quiet take");
                return Ok(String::new());
            };
            run_full(
                &engine,
                samples,
                &language,
                &detect,
                threads,
                prompt.as_deref(),
            )
        })
        .await
        .map_err(|e| anyhow::anyhow!("Transcription task failed: {}", e))?
    }

    /// Run the take again on the CPU after the GPU failed it, and keep the
    /// CPU for the rest of the session.
    async fn retry_on_cpu(
        &self,
        gpu_error: anyhow::Error,
        samples: Arc<[f32]>,
        prompt: Option<String>,
        language: String,
    ) -> Result<String> {
        warn!(
            "Whisper failed on the GPU, so the take runs again on the CPU, \
             and the CPU stays in use for this session: {:#}",
            gpu_error
        );
        gpu_fallback::remember(self.gpu);
        let engine = load_engine(&self.model_path, false)
            .await
            .with_context(|| {
                format!(
                    "The GPU failed the take ({:#}), and the model did not load on the CPU",
                    gpu_error
                )
            })?;
        let failed = std::mem::replace(&mut *self.engine.lock(), Arc::new(engine));
        self.using_gpu.store(false, Ordering::Release);
        // A failed GPU can be slow to free its memory.
        tokio::task::spawn_blocking(move || drop(failed));
        info!("Whisper model loaded on {}", self.get_device_info());
        self.run_take(samples, prompt, language).await
    }

    pub fn is_using_gpu(&self) -> bool {
        self.using_gpu.load(Ordering::Acquire)
    }

    pub fn get_device_info(&self) -> String {
        if self.is_using_gpu() {
            format!("GPU: {} ({})", self.gpu.device_name, self.gpu.gpu_type)
        } else if self.gpu.available {
            "CPU".to_string()
        } else {
            format!("CPU: {}", self.gpu.device_name)
        }
    }
}

/// Load a ggml model and make its state, on the GPU when `use_gpu` is true.
async fn load_engine(model_path: &Path, use_gpu: bool) -> Result<Engine> {
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
    params.use_gpu(use_gpu);

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
    Ok(Engine {
        context,
        state: Mutex::new(state),
    })
}

fn run_full(
    engine: &Engine,
    samples: &[f32],
    language: &str,
    detect: &[String],
    threads: i32,
    prompt: Option<&str>,
) -> Result<String> {
    let mut state = engine.state.lock();
    let language = if language == "auto" && !detect.is_empty() {
        limited_detection(&mut state, samples, threads, detect)
    } else {
        language.to_string()
    };

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some(&language));
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

    state
        .full(params, samples)
        .map_err(|e| anyhow::anyhow!("Transcription failed: {}", e))?;

    let mut text = String::new();
    for segment in state.as_iter() {
        let segment = strip_non_speech(&segment.to_string());
        if segment.is_empty() {
            continue;
        }
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(&segment);
    }
    Ok(text)
}

/// The most likely of the `allowed` languages in the first 30 seconds of
/// `samples`. When detection fails, or gives no allowed language, Whisper
/// detects the language on its own (`auto`).
fn limited_detection(
    state: &mut WhisperState,
    samples: &[f32],
    threads: i32,
    allowed: &[String],
) -> String {
    let probabilities = match language_probabilities(state, samples, threads) {
        Ok(probabilities) => probabilities,
        Err(e) => {
            warn!("Language detection failed, so Whisper picks from every language: {e:#}");
            return "auto".to_string();
        },
    };
    match language::most_likely(probabilities, allowed) {
        Some((code, probability)) => {
            info!(
                language = code,
                probability,
                "Detected {} among {}",
                code,
                allowed.join(", ")
            );
            code.to_string()
        },
        None => {
            warn!(
                "Whisper gave no probability for {}, so it picks from every language",
                allowed.join(", ")
            );
            "auto".to_string()
        },
    }
}

/// Whisper's probability for each language code, from the spectrogram of
/// `samples` at its start.
fn language_probabilities(
    state: &mut WhisperState,
    samples: &[f32],
    threads: i32,
) -> Result<Vec<(&'static str, f32)>> {
    let threads = usize::try_from(threads).unwrap_or(1).max(1);
    state
        .pcm_to_mel(samples, threads)
        .map_err(|e| anyhow::anyhow!("spectrogram: {}", e))?;
    let (_, probabilities) = state
        .lang_detect(0, threads)
        .map_err(|e| anyhow::anyhow!("detection: {}", e))?;
    Ok(probabilities
        .into_iter()
        .enumerate()
        .filter_map(|(id, probability)| {
            let code = whisper_rs::get_lang_str(i32::try_from(id).ok()?)?;
            Some((code, probability))
        })
        .collect())
}

/// The audio Whisper gets: the speech that voice activity detection found,
/// or the whole take when it did not run. When it found no speech in a take
/// that is loud enough to hold some, Whisper gets the whole take, because
/// the model can miss one short word. A quiet take gets nothing.
pub(crate) fn whisper_input<'a>(take: &'a [f32], speech: Option<&'a [f32]>) -> Option<&'a [f32]> {
    match speech {
        None => Some(take),
        Some(speech) if !speech.is_empty() => Some(speech),
        Some(_) if level::rms(take) >= level::QUIET_RMS => {
            debug!("Voice activity detection found no speech, so Whisper gets the whole take");
            Some(take)
        },
        Some(_) => None,
    }
}

/// The speech in `samples`, as the Silero model finds it.
fn speech_in(vad: &Mutex<WhisperVadContext>, samples: &[f32]) -> Result<Vec<f32>> {
    let mut params = WhisperVadParams::new();
    params.set_min_speech_duration(VAD_MIN_SPEECH_MS);
    params.set_speech_pad(VAD_SPEECH_PAD_MS);
    let segments = vad
        .lock()
        .segments_from_samples(params, samples)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    let spans: Vec<(f32, f32)> = segments.map(|s| (s.start, s.end)).collect();
    let speech = join_speech(samples, &spans);
    debug!(
        stretches = spans.len(),
        kept_samples = speech.len(),
        take_samples = samples.len(),
        "Voice activity detection"
    );
    Ok(speech)
}

/// The stretches of `samples` that `spans` mark as speech (start and end in
/// centiseconds), joined by a short silence. Each stretch but the last keeps
/// a little audio after its end, without reaching into the next one.
pub(crate) fn join_speech(samples: &[f32], spans: &[(f32, f32)]) -> Vec<f32> {
    let at = |centiseconds: f32| -> usize {
        let index = (centiseconds.max(0.0) / 100.0 * WHISPER_SAMPLE_RATE as f32).round();
        (index as usize).min(samples.len())
    };
    let mut speech = Vec::new();
    let mut cursor = 0;
    for (i, &(start, end)) in spans.iter().enumerate() {
        let start = at(start).max(cursor);
        let mut end = at(end);
        if let Some(&(next, _)) = spans.get(i + 1) {
            end = (end + SPEECH_OVERLAP).min(at(next)).max(end);
        }
        let end = end.min(samples.len());
        if end <= start {
            continue;
        }
        if !speech.is_empty() {
            speech.resize(speech.len() + SPEECH_GAP, 0.0);
        }
        speech.extend_from_slice(&samples[start..end]);
        cursor = end;
    }
    speech
}

/// Whisper writes non-speech as tags like `[BLANK_AUDIO]`, `[MUSIC]` or
/// `(upbeat music)`. Square brackets never hold dictated words, so they are
/// dropped anywhere. Parentheses are dropped only when they make up the whole
/// segment, because an inline aside can be real speech.
pub(crate) fn strip_non_speech(segment: &str) -> String {
    let segment = segment.trim();
    if let Some(inner) = segment.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
        if !inner.contains(['(', ')']) {
            return String::new();
        }
    }

    let mut kept = String::with_capacity(segment.len());
    let mut rest = segment;
    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open..].find(']') else {
            break;
        };
        kept.push_str(&rest[..open]);
        kept.push(' ');
        rest = &rest[open + close + 1..];
    }
    kept.push_str(rest);
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
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

    use rubato::audioadapter_buffers::direct::SequentialSlice;

    let chunk_size = 1024;
    let mut resampler = Fft::<f32>::new_custom(
        from_rate as usize,
        to_rate as usize,
        chunk_size,
        1,
        1,
        WindowFunction::BlackmanHarris2,
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
    fn non_speech_tags_are_dropped() {
        assert_eq!(strip_non_speech("[BLANK_AUDIO]"), "");
        assert_eq!(strip_non_speech(" [ Silence ] "), "");
        assert_eq!(strip_non_speech("(upbeat music)"), "");
        assert_eq!(
            strip_non_speech("Hello [BLANK_AUDIO] world."),
            "Hello world."
        );
        assert_eq!(strip_non_speech("[MUSIC] Okay, ship it."), "Okay, ship it.");
    }

    #[test]
    fn speech_with_brackets_or_parentheses_is_kept() {
        assert_eq!(
            strip_non_speech("Use the API (the public one) here."),
            "Use the API (the public one) here."
        );
        assert_eq!(strip_non_speech("(one) and (two)"), "(one) and (two)");
        assert_eq!(strip_non_speech("an open [ bracket"), "an open [ bracket");
    }

    #[test]
    fn speech_stretches_are_joined_by_a_short_silence() {
        let samples: Vec<f32> = (0..48_000).map(|i| 1.0 + i as f32).collect();
        let speech = join_speech(&samples, &[(50.0, 100.0), (200.0, 250.0)]);
        let first = 8_000..16_000 + SPEECH_OVERLAP;
        let second = 32_000..40_000;
        assert_eq!(speech.len(), first.len() + SPEECH_GAP + second.len());
        assert_eq!(speech[0], samples[8_000]);
        assert_eq!(speech[first.len() - 1], samples[first.end - 1]);
        assert!(speech[first.len()..first.len() + SPEECH_GAP]
            .iter()
            .all(|s| *s == 0.0));
        assert_eq!(speech[speech.len() - 1], samples[39_999]);
    }

    #[test]
    fn speech_overlap_never_repeats_audio() {
        let samples: Vec<f32> = (0..32_000).map(|i| i as f32).collect();
        let speech = join_speech(&samples, &[(0.0, 100.0), (101.0, 150.0)]);
        let kept: Vec<f32> = speech.into_iter().filter(|s| *s != 0.0).collect();
        assert!(
            kept.windows(2).all(|pair| pair[0] < pair[1]),
            "no sample is copied twice"
        );
        assert_eq!(kept.last(), Some(&23_999.0));
    }

    #[test]
    fn whisper_gets_the_speech_or_the_whole_take() {
        let loud: Vec<f32> = (0..16_000).map(|i| (i as f32 * 0.05).sin() * 0.2).collect();
        let speech = &loud[4_000..8_000];
        assert_eq!(whisper_input(&loud, Some(speech)), Some(speech));
        assert_eq!(
            whisper_input(&loud, None),
            Some(&loud[..]),
            "without VAD, the whole take"
        );
        assert_eq!(
            whisper_input(&loud, Some(&[])),
            Some(&loud[..]),
            "VAD found nothing in a loud take: the whole take"
        );

        let quiet = vec![0.002f32; 16_000];
        assert!(level::rms(&quiet) < level::QUIET_RMS);
        assert_eq!(
            whisper_input(&quiet, Some(&[])),
            None,
            "VAD found nothing in a quiet take: nothing"
        );
        assert_eq!(whisper_input(&quiet, None), Some(&quiet[..]));
    }

    #[test]
    fn no_speech_and_bad_spans_give_nothing() {
        let samples = vec![0.5f32; 16_000];
        assert!(join_speech(&samples, &[]).is_empty());
        assert!(join_speech(&samples, &[(80.0, 40.0)]).is_empty());
        assert_eq!(join_speech(&samples, &[(90.0, 500.0)]).len(), 1_600);
        assert!(join_speech(&[], &[(0.0, 100.0)]).is_empty());
    }

    #[test]
    fn resample_is_identity_for_equal_rates() {
        let input = vec![0.5f32; 100];
        assert_eq!(resample(&input, 16_000, 16_000).unwrap(), input);
    }
}
