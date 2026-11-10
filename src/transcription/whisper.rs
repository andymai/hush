//! Speech-to-text through transcribe.cpp, which runs Whisper on ggml.

use crate::audio::level;
use crate::audio::parts;
use crate::logging::{transcription as logging, RequestContext};
use crate::transcription::cpu;
use crate::transcription::device::{self, GpuAvailability};
use crate::transcription::gpu_fallback;
use crate::transcription::langid;
use crate::transcription::models;
use crate::Result;
use anyhow::Context;
use parking_lot::Mutex;
use rubato::{Fft, FixedSync, Indexing, Resampler, WindowFunction};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};
use transcribe_cpp::{
    Backend, Model, ModelOptions, RunExtension, RunOptions, Session, SessionOptions, Task,
    TimestampKind, Transcript, VadOptions, VadSession, VadSessionOptions, WhisperRunOptions,
};

/// Sample rate Whisper expects
pub const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// The most prompt tokens Whisper keeps: half of its 448-token text context,
/// less the token that starts the prompt. transcribe.cpp drops the rest from
/// the front.
pub const PROMPT_TOKEN_BUDGET: usize = 223;

/// Audio kept on each side of a stretch of speech, so that soft first and
/// last sounds of a word stay in.
const VAD_SPEECH_PAD_MS: i32 = 100;

/// Shorter speech is dropped as noise. Silero's default of 250 ms can drop a
/// dictated "no" or "OK".
const VAD_MIN_SPEECH_MS: i32 = 100;

/// Silence put between two stretches of speech, as whisper.cpp does.
const SPEECH_GAP: usize = WHISPER_SAMPLE_RATE as usize / 10;

/// Audio kept after each stretch of speech but the last, as whisper.cpp does.
const SPEECH_OVERLAP: usize = WHISPER_SAMPLE_RATE as usize / 10;

/// The longest input of one Parakeet run. The memory of its encoder grows
/// with the input, so a long take runs in pieces.
const PARAKEET_PIECE: usize = 30 * WHISPER_SAMPLE_RATE as usize;

/// The shortest piece that a cut without a stretch of speech leaves.
const MIN_PIECE: usize = WHISPER_SAMPLE_RATE as usize;

/// The end of a piece that a cut without a stretch of speech searches for
/// its quietest block, as a hard cut of a hands-free part does.
const CUT_SEARCH: usize = parts::HARD_CUT_SEARCH.as_secs() as usize * WHISPER_SAMPLE_RATE as usize;

/// Out-of-memory takes that a session runs again on a CPU copy of the model.
/// Each costs a failed GPU run and a model load, so the next one moves the
/// session to the CPU.
const OUT_OF_MEMORY_RETRIES: u32 = 1;

#[derive(Debug, Clone)]
pub struct TranscriptionResult {
    pub text: String,
    pub confidence: f32,
    pub processing_time: Duration,
    /// The language that limited detection chose, when it ran and chose one
    pub detected: Option<&'static str>,
}

/// What one run of Whisper heard.
struct Heard {
    text: String,
    detected: Option<&'static str>,
}

impl Heard {
    fn nothing() -> Self {
        Self {
            text: String::new(),
            detected: None,
        }
    }
}

/// The model families that Hush runs on transcribe.cpp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Whisper,
    /// NVIDIA Parakeet TDT: it takes no prompt and no language hint, and it
    /// tells its languages apart on its own. Its decoder runs on the CPU.
    Parakeet,
}

impl Family {
    fn from_arch(arch: &str) -> Option<Self> {
        match arch {
            "whisper" => Some(Family::Whisper),
            "parakeet" => Some(Family::Parakeet),
            _ => None,
        }
    }

    /// The family of the model in `path`, from its header, without a load.
    pub fn of_file(path: &Path) -> Option<Self> {
        models::file_architecture(path)
            .as_deref()
            .and_then(Self::from_arch)
    }

    pub fn name(self) -> &'static str {
        match self {
            Family::Whisper => "Whisper",
            Family::Parakeet => "Parakeet",
        }
    }

    /// Whether the model reads a prompt: the window title, the learned
    /// terms, and the text before a part.
    pub fn takes_prompt(self) -> bool {
        self == Family::Whisper
    }

    /// The CPU threads of a session. Whisper on a GPU needs one; the TDT
    /// decoder of Parakeet runs on the CPU, also when the encoder runs on a
    /// GPU, and gets about a third slower with one thread.
    fn threads(self, gpu: bool) -> i32 {
        match (self, gpu) {
            (Family::Whisper, true) => 1,
            _ => cpu_threads(),
        }
    }
}

/// A run that the model refuses on any device, so a retry on the CPU cannot
/// help.
#[derive(Debug, thiserror::Error)]
#[error("Transcription failed: {0}")]
struct Rejected(transcribe_cpp::Error);

/// A run that did not fit in the memory of the device. The device works, so
/// only this take moves to the CPU.
#[derive(Debug, thiserror::Error)]
#[error("Transcription ran out of memory: {0}")]
struct OutOfMemory(transcribe_cpp::Error);

/// The session is created once so that its GPU buffers are reused across
/// dictations instead of reallocated per call.
struct Engine {
    model: Model,
    family: Family,
    session: Mutex<Session>,
    /// The description of the device that holds the weights
    device: String,
    /// The language codes the model accepts; empty when it lists none
    languages: Vec<String>,
    /// Whether the model knows more than one language, unlike an `.en` model
    multilingual: bool,
}

pub struct WhisperTranscriber {
    /// A CPU engine takes the place of a GPU engine that fails a take.
    engine: Mutex<Arc<Engine>>,
    vad: Option<Arc<Mutex<VadSession>>>,
    /// The models folder that holds the language-ID model, when limited
    /// detection uses it
    langid_dir: Option<PathBuf>,
    gpu: &'static GpuAvailability,
    using_gpu: AtomicBool,
    /// The takes that ran out of GPU memory in this session
    out_of_memory: AtomicU32,
    model_path: PathBuf,
    language: String,
    /// The languages that detection may pick; empty allows every language.
    detect_languages: Arc<[String]>,
}

impl WhisperTranscriber {
    /// Load a speech model: Whisper as a ggml `.bin` or a GGUF file, or
    /// Parakeet as a GGUF file. `use_gpu` is honored when a GPU backend
    /// exists, unless that GPU failed a take with the same driver before.
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

        device::route_engine_logs();

        gpu_fallback::wait_for_gpu(use_gpu).await;
        let gpu = GpuAvailability::detect();
        if use_gpu && !gpu.available {
            if gpu_fallback::GPU_BUILD {
                warn!(
                    "The speech model loads on the CPU, because ggml found no GPU. The graphics \
                     driver was probably not ready when Hush started, for example at login. \
                     Run `hush daemon restart` to use the GPU"
                );
            } else {
                warn!("GPU acceleration requested but no GPU backend is available; using CPU");
            }
        }
        let family = Family::of_file(model_path).unwrap_or(Family::Whisper);
        let using_gpu = use_gpu && gpu.available && !gpu_fallback::avoid_gpu(gpu, family);
        info!(
            request_id = %ctx.request_id,
            model_path = %model_path.display(),
            device = %gpu,
            using_gpu,
            "Loading the speech model"
        );

        let (engine, using_gpu) = match load_engine(model_path, using_gpu).await {
            Ok(engine) => (engine, using_gpu),
            Err(e) if using_gpu => {
                warn!(
                    "The speech model did not load on the GPU, so it loads on the CPU: {:#}",
                    e
                );
                (load_engine(model_path, false).await?, false)
            },
            Err(e) => return Err(e),
        };
        let transcriber = Self {
            engine: Mutex::new(Arc::new(engine)),
            vad: None,
            langid_dir: None,
            gpu,
            using_gpu: AtomicBool::new(using_gpu),
            out_of_memory: AtomicU32::new(0),
            model_path: model_path.to_path_buf(),
            language: "en".to_string(),
            detect_languages: Arc::from([]),
        };
        info!(
            "{} model loaded on {}",
            transcriber.family().name(),
            transcriber.get_device_info()
        );
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
        let session = Model::load_with(model_path, &cpu_options())
            .and_then(|model| {
                model.vad_session_with(&VadSessionOptions {
                    n_threads: cpu_threads().min(4),
                })
            })
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to load the VAD model {}: {}",
                    model_path.display(),
                    e
                )
            })?;
        self.vad = Some(Arc::new(Mutex::new(session)));
        info!(
            "Voice activity detection loaded from {}",
            model_path.display()
        );
        Ok(())
    }

    pub fn has_vad(&self) -> bool {
        self.vad.is_some()
    }

    /// Pick the language of a take in `auto` among `detect_languages` with
    /// the language-ID model in `models_dir`. Takes before the model is there
    /// let Whisper detect the language within the set.
    pub fn with_langid(mut self, models_dir: &Path) -> Self {
        self.langid_dir = Some(models_dir.to_path_buf());
        self
    }

    /// The languages that limited detection may pick.
    pub fn detect_languages(&self) -> &[String] {
        &self.detect_languages
    }

    /// Whether the model knows more than one language, so that detection can
    /// choose. An `.en` model knows only English.
    pub fn is_multilingual(&self) -> bool {
        self.engine().multilingual
    }

    pub fn family(&self) -> Family {
        self.engine().family
    }

    /// How many tokens Whisper makes of `prompt`, or `None` when it cannot
    /// tokenize it.
    pub fn prompt_tokens(&self, prompt: &str) -> Option<usize> {
        if prompt.contains('\0') {
            return None;
        }
        self.engine()
            .model
            .tokenize(&format!(" {}", whisper_prompt(prompt)))
            .ok()
            .map(|tokens| tokens.len())
    }

    /// The most prompt tokens Whisper keeps.
    pub fn prompt_token_budget(&self) -> usize {
        PROMPT_TOKEN_BUDGET
    }

    fn engine(&self) -> Arc<Engine> {
        Arc::clone(&self.engine.lock())
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
                detected: None,
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

        let heard = match self
            .run_take(
                self.engine(),
                Arc::clone(&samples),
                prompt.clone(),
                language.clone(),
            )
            .await
        {
            Err(e) if self.is_using_gpu() && e.downcast_ref::<OutOfMemory>().is_some() => {
                let earlier = self.out_of_memory.fetch_add(1, Ordering::AcqRel);
                if retries_on_a_copy(earlier) {
                    self.retry_once_on_cpu(e, samples, prompt, language).await
                } else {
                    self.stay_on_cpu(e, samples, prompt, language).await
                }
            },
            Err(e) if self.is_using_gpu() && e.downcast_ref::<Rejected>().is_none() => {
                self.retry_on_cpu(e, samples, prompt, language).await
            },
            heard => heard,
        };
        let Heard { text, detected } = match heard {
            Ok(heard) => heard,
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
            detected,
        })
    }

    async fn run_take(
        &self,
        engine: Arc<Engine>,
        samples: Arc<[f32]>,
        prompt: Option<String>,
        language: String,
    ) -> Result<Heard> {
        let vad = self.vad.clone();
        let langid_dir = self.langid_dir.clone();
        let detect = Arc::clone(&self.detect_languages);
        tokio::task::spawn_blocking(move || {
            let trimmed = vad.as_deref().and_then(|vad| match speech_in(vad, &samples) {
                Ok(speech) => Some(speech),
                Err(e) => {
                    warn!("Voice activity detection failed, so Whisper gets the whole take: {e:#}");
                    None
                },
            });
            let starts = trimmed
                .as_ref()
                .map_or(&[][..], |(_, starts)| starts.as_slice());
            let Some(speech) = whisper_input(
                &samples,
                trimmed.as_ref().map(|(speech, _)| speech.as_slice()),
            ) else {
                debug!("The take is too quiet to hold speech");
                return Ok(Heard::nothing());
            };
            if engine.family == Family::Parakeet {
                return Ok(Heard {
                    text: run_pieces(&engine, speech, starts)?,
                    detected: None,
                });
            }
            let prompt = prompt.as_deref().filter(|_| engine.family.takes_prompt());
            if language != "auto" || detect.is_empty() || !engine.multilingual {
                let (text, _) = run_full(&engine, speech, &language, prompt)?;
                return Ok(Heard {
                    text,
                    detected: None,
                });
            }
            if let Some(code) = langid_dir
                .as_deref()
                .and_then(|dir| langid::pick(dir, &samples, speech, &detect))
            {
                let (text, _) = run_full(&engine, speech, code, prompt)?;
                return Ok(Heard {
                    text,
                    detected: Some(code),
                });
            }
            detect_within(&engine, speech, prompt, &detect)
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
    ) -> Result<Heard> {
        warn!(
            "The speech model failed on the GPU, so the take runs again on the CPU, \
             and the CPU stays in use for this session: {:#}",
            gpu_error
        );
        gpu_fallback::remember(self.gpu, self.family());
        self.move_to_cpu(&gpu_error).await?;
        self.run_take(self.engine(), samples, prompt, language)
            .await
    }

    /// Run the take again on the CPU after the GPU ran out of memory again,
    /// and keep the CPU for the rest of the session. The GPU works, so no
    /// failure is written for later starts.
    async fn stay_on_cpu(
        &self,
        gpu_error: anyhow::Error,
        samples: Arc<[f32]>,
        prompt: Option<String>,
        language: String,
    ) -> Result<Heard> {
        warn!(
            "The GPU ran out of memory again, so the take runs again on the CPU, and the CPU \
             stays in use for this session: {:#}",
            gpu_error
        );
        gpu_fallback::keep_on_cpu(self.family());
        self.move_to_cpu(&gpu_error).await?;
        self.run_take(self.engine(), samples, prompt, language)
            .await
    }

    /// Put a CPU engine in place of the GPU engine for the rest of the session.
    async fn move_to_cpu(&self, gpu_error: &anyhow::Error) -> Result<()> {
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
        info!(
            "{} model loaded on {}",
            self.family().name(),
            self.get_device_info()
        );
        Ok(())
    }

    /// Run the take again on a CPU copy of the model after the GPU ran out
    /// of memory for it. The session keeps the GPU for the next take.
    async fn retry_once_on_cpu(
        &self,
        gpu_error: anyhow::Error,
        samples: Arc<[f32]>,
        prompt: Option<String>,
        language: String,
    ) -> Result<Heard> {
        warn!(
            "The GPU ran out of memory for this take, so it runs again on the CPU. \
             The next take uses the GPU again: {:#}",
            gpu_error
        );
        let engine = load_engine(&self.model_path, false)
            .await
            .with_context(|| {
                format!(
                    "The GPU ran out of memory for the take ({:#}), and the model did not \
                     load on the CPU",
                    gpu_error
                )
            })?;
        self.run_take(Arc::new(engine), samples, prompt, language)
            .await
    }

    pub fn is_using_gpu(&self) -> bool {
        self.using_gpu.load(Ordering::Acquire)
    }

    pub fn get_device_info(&self) -> String {
        let device = self.engine().device.clone();
        if self.is_using_gpu() {
            format!("GPU: {} ({})", device, self.gpu.gpu_type)
        } else if self.gpu.available {
            "CPU".to_string()
        } else {
            format!("CPU: {}", device)
        }
    }
}

pub(crate) fn cpu_options() -> ModelOptions {
    ModelOptions {
        backend: Backend::Cpu,
        ..Default::default()
    }
}

/// Load a speech model and make its session, on the GPU when `use_gpu` is
/// true.
async fn load_engine(model_path: &Path, use_gpu: bool) -> Result<Engine> {
    let path = model_path.to_path_buf();
    // Loading a large model takes seconds, so keep it off the async runtime.
    tokio::task::spawn_blocking(move || {
        let options = if use_gpu {
            let (backend, gpu) = device::gpu_device()
                .ok_or_else(|| anyhow::anyhow!("ggml lists no GPU to load the model on"))?;
            ModelOptions {
                backend,
                device: Some(gpu),
                ..Default::default()
            }
        } else {
            cpu_options()
        };
        let model = Model::load_with(&path, &options).map_err(|e| {
            anyhow::anyhow!("Failed to load the speech model {}: {}", path.display(), e)
        })?;
        let family = Family::from_arch(&model.arch()).ok_or_else(|| {
            anyhow::anyhow!(
                "{} is a {} model, and Hush runs only Whisper and Parakeet models",
                path.display(),
                model.arch()
            )
        })?;
        let threads = family.threads(use_gpu);
        let session = model
            .session_with(&SessionOptions {
                n_threads: threads,
                ..Default::default()
            })
            .map_err(|e| {
                anyhow::anyhow!("Failed to create the {} session: {}", family.name(), e)
            })?;
        let device = model
            .device()
            .map(|device| device.description)
            .unwrap_or_else(|_| model.backend());
        let (languages, multilingual) = model
            .capabilities()
            .map(|caps| (caps.languages, caps.supports_language_detect))
            .unwrap_or_default();
        debug!(
            backend = %model.backend(),
            threads,
            multilingual,
            "{} session ready",
            family.name()
        );
        Ok(Engine {
            model,
            family,
            session: Mutex::new(session),
            device,
            languages,
            multilingual,
        })
    })
    .await
    .map_err(|e| anyhow::anyhow!("Model loading task failed: {}", e))?
}

/// The prompt as Whisper gets it. transcribe.cpp refuses a prompt that holds
/// a special token such as `<|en|>`, so no `<|` stays in the text.
fn whisper_prompt(prompt: &str) -> String {
    let mut text = prompt.trim().to_string();
    while text.contains("<|") {
        text = text.replace("<|", "<");
    }
    text
}

/// The language hint for a run: `None` lets Whisper detect it. A model that
/// does not list the language gets no hint, so an `.en` model transcribes
/// English as whisper.cpp did, instead of refusing the take.
fn take_language<'a>(language: &'a str, known: &[String]) -> Option<&'a str> {
    if language == "auto" {
        return None;
    }
    if known.is_empty() || known.iter().any(|code| code == language) {
        return Some(language);
    }
    debug!(
        language,
        "The model does not list this language, so it gets no hint"
    );
    None
}

/// Within one take, each 30-second window after the first is conditioned on
/// the prompt and the text before it, which slide out as the text grows, as
/// whisper.cpp does. Nothing carries from one take to the next.
fn run_options(language: Option<&str>, prompt: Option<&str>) -> RunOptions {
    RunOptions {
        task: Task::Transcribe,
        timestamps: TimestampKind::Segment,
        language: language.map(str::to_string),
        family: Some(RunExtension::Whisper(WhisperRunOptions {
            initial_prompt: prompt.map(whisper_prompt),
            condition_on_prev_tokens: Some(true),
            ..Default::default()
        })),
        ..Default::default()
    }
}

/// A Parakeet run: no language hint, no prompt, and no timestamps, which Hush
/// does not use.
fn parakeet_options() -> RunOptions {
    RunOptions {
        task: Task::Transcribe,
        timestamps: TimestampKind::None,
        ..Default::default()
    }
}

/// The text of a run, and the language Whisper detected when it got no hint.
fn run_full(
    engine: &Engine,
    samples: &[f32],
    language: &str,
    prompt: Option<&str>,
) -> Result<(String, Option<String>)> {
    use transcribe_cpp::Error;

    let options = match engine.family {
        Family::Whisper => run_options(take_language(language, &engine.languages), prompt),
        Family::Parakeet => parakeet_options(),
    };
    let transcript = match engine.session.lock().run(samples, &options) {
        Ok(transcript) => transcript,
        Err(
            Error::OutputTruncated {
                message,
                partial: Some(partial),
            }
            | Error::OutputRepetition {
                message,
                partial: Some(partial),
            },
        ) => {
            warn!(
                "The model stopped early, so the text ends there: {}",
                message
            );
            *partial
        },
        Err(e @ (Error::InvalidArgument(_) | Error::Unsupported(_))) => {
            return Err(Rejected(e).into());
        },
        Err(e @ Error::OutOfMemory(_)) => return Err(OutOfMemory(e).into()),
        Err(e) => return Err(anyhow::anyhow!("Transcription failed: {}", e)),
    };
    Ok((transcript_text(&transcript), transcript.language))
}

/// Whether an out-of-memory take runs again on a CPU copy of the model,
/// after `earlier` such takes in the session, and the session keeps the GPU.
fn retries_on_a_copy(earlier: u32) -> bool {
    earlier < OUT_OF_MEMORY_RETRIES
}

/// A Parakeet take in [`pieces`], with the text of each piece one space
/// apart.
fn run_pieces(engine: &Engine, speech: &[f32], starts: &[usize]) -> Result<String> {
    let mut text = String::new();
    for piece in pieces(speech, starts, PARAKEET_PIECE) {
        let (heard, _) = run_full(engine, &speech[piece], "auto", None)?;
        if heard.is_empty() {
            continue;
        }
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(&heard);
    }
    Ok(text)
}

/// `speech` in pieces of at most `max` samples. A cut falls in the middle of
/// the silence before the last stretch of speech that starts within the
/// limit. `starts` holds where each stretch starts, as [`join_speech`] gives
/// them. Without such a stretch, the cut falls in the quietest block of the
/// last [`CUT_SEARCH`] before the limit, and the last piece keeps at least
/// [`MIN_PIECE`].
fn pieces(speech: &[f32], starts: &[usize], max: usize) -> Vec<Range<usize>> {
    let len = speech.len();
    let max = max.max(MIN_PIECE);
    let mut pieces = Vec::new();
    let mut from = 0;
    while len - from > max {
        let limit = from + max;
        let cut = starts
            .iter()
            .rev()
            .map(|&start| start.saturating_sub(SPEECH_GAP / 2))
            .find(|&cut| cut > from && cut <= limit)
            .unwrap_or_else(|| {
                let end = limit.min(len - MIN_PIECE);
                parts::quietest_in(speech, end.saturating_sub(CUT_SEARCH).max(from)..end)
                    .unwrap_or(end)
            });
        pieces.push(from..cut);
        from = cut;
    }
    pieces.push(from..len);
    pieces
}

/// Limited detection without the language-ID model: Whisper detects the
/// language, and a take that it hears in a language outside `allowed` runs
/// again in an allowed language (see [`langid::fallback`]), so the choice
/// stays in the set.
fn detect_within(
    engine: &Engine,
    speech: &[f32],
    prompt: Option<&str>,
    allowed: &[String],
) -> Result<Heard> {
    let (text, heard) = run_full(engine, speech, "auto", prompt)?;
    if let Some(code) = heard
        .as_deref()
        .and_then(|code| langid::allowed_code(code, allowed))
    {
        langid::confirm(code);
        info!(
            language = code,
            "Whisper detected {} among {}",
            code,
            allowed.join(", ")
        );
        return Ok(Heard {
            text,
            detected: Some(code),
        });
    }
    let Some(first) = langid::fallback(allowed, &engine.languages) else {
        return Ok(Heard {
            text,
            detected: None,
        });
    };
    info!(
        "Whisper heard {}, which is not among {}, so the take runs again in {}",
        heard.as_deref().unwrap_or("no language"),
        allowed.join(", "),
        first
    );
    let (text, _) = run_full(engine, speech, first, prompt)?;
    Ok(Heard {
        text,
        detected: Some(first),
    })
}

/// The text of every segment without non-speech tags, one space apart.
fn transcript_text(transcript: &Transcript) -> String {
    let segments: Vec<&str> = if transcript.segments.is_empty() {
        vec![transcript.text.as_str()]
    } else {
        transcript
            .segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect()
    };
    let mut text = String::new();
    for segment in segments {
        let segment = strip_non_speech(segment);
        if segment.is_empty() {
            continue;
        }
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(&segment);
    }
    text
}

/// The audio Whisper gets: the speech that voice activity detection found,
/// or the whole take when it found none or did not run. A quiet take gets
/// nothing, because Whisper invents words such as "you" for silence.
pub(crate) fn whisper_input<'a>(take: &'a [f32], speech: Option<&'a [f32]>) -> Option<&'a [f32]> {
    match speech {
        Some(speech) if !speech.is_empty() => Some(speech),
        _ if level::rms(take) < level::QUIET_RMS => None,
        None => Some(take),
        Some(_) => {
            debug!("Voice activity detection found no speech, so Whisper gets the whole take");
            Some(take)
        },
    }
}

/// The speech in `samples`, as the Silero model finds it, and where each
/// stretch of it starts (see [`join_speech`]).
fn speech_in(vad: &Mutex<VadSession>, samples: &[f32]) -> Result<(Vec<f32>, Vec<usize>)> {
    let options = VadOptions {
        min_speech_ms: VAD_MIN_SPEECH_MS,
        speech_pad_ms: VAD_SPEECH_PAD_MS,
        ..Default::default()
    };
    let found = vad
        .lock()
        .run(samples, &options)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    let index = |sample: i64| usize::try_from(sample.max(0)).unwrap_or(usize::MAX);
    let spans = merge_touching(
        found
            .segments
            .iter()
            .map(|segment| (index(segment.start_sample), index(segment.end_sample))),
    );
    let (speech, starts) = join_speech(samples, &spans);
    debug!(
        stretches = spans.len(),
        kept_samples = speech.len(),
        take_samples = samples.len(),
        "Voice activity detection"
    );
    Ok((speech, starts))
}

/// Spans in time order, with spans that touch or overlap made into one.
/// Padding can make two stretches touch, and joining them as two would put
/// silence into the middle of the speech.
pub(crate) fn merge_touching(
    spans: impl IntoIterator<Item = (usize, usize)>,
) -> Vec<(usize, usize)> {
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in spans {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged
}

/// The stretches of `samples` that `spans` mark as speech (start and end
/// sample), joined by a short silence, and where each stretch starts in the
/// result. Each stretch but the last keeps a little audio after its end,
/// without reaching into the next one.
pub(crate) fn join_speech(samples: &[f32], spans: &[(usize, usize)]) -> (Vec<f32>, Vec<usize>) {
    let mut speech = Vec::new();
    let mut starts = Vec::new();
    let mut cursor = 0;
    for (i, &(start, end)) in spans.iter().enumerate() {
        let start = start.min(samples.len()).max(cursor);
        let mut end = end.min(samples.len());
        if let Some(&(next, _)) = spans.get(i + 1) {
            end = (end + SPEECH_OVERLAP).min(next.min(samples.len())).max(end);
        }
        if end <= start {
            continue;
        }
        if !speech.is_empty() {
            speech.resize(speech.len() + SPEECH_GAP, 0.0);
        }
        starts.push(speech.len());
        speech.extend_from_slice(&samples[start..end]);
        cursor = end;
    }
    (speech, starts)
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

pub(crate) fn cpu_threads() -> i32 {
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

    #[tokio::test]
    async fn a_file_that_is_no_model_fails_to_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ggml-base.bin");
        std::fs::write(&path, b"not a model").unwrap();
        let err = WhisperTranscriber::new(&path, false)
            .await
            .err()
            .expect("a file that is no model must fail");
        assert!(err.to_string().contains("Failed to load"), "{err}");
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
    fn the_text_joins_segments_without_tags() {
        let segment = |text: &str| transcribe_cpp::Segment {
            text: text.to_string(),
            ..Default::default()
        };
        let transcript = Transcript {
            text: "ignored".to_string(),
            segments: vec![
                segment(" Ask not"),
                segment("[BLANK_AUDIO]"),
                segment("(wind blowing)"),
                segment(" what you can do."),
            ],
            ..Default::default()
        };
        assert_eq!(transcript_text(&transcript), "Ask not what you can do.");
        let plain = Transcript {
            text: " [MUSIC] Hello.".to_string(),
            ..Default::default()
        };
        assert_eq!(transcript_text(&plain), "Hello.");
    }

    #[test]
    fn a_prompt_never_holds_a_special_token() {
        assert_eq!(
            whisper_prompt("  Kubernetes, PostgreSQL "),
            "Kubernetes, PostgreSQL"
        );
        assert_eq!(whisper_prompt("a <|en|> b"), "a <en|> b");
        assert!(!whisper_prompt("<<||endoftext|>").contains("<|"));
    }

    #[test]
    fn a_run_transcribes_in_the_take_language_with_the_prompt() {
        let options = run_options(Some("de"), Some("Gregor Samsa"));
        assert_eq!(options.task, Task::Transcribe);
        assert_eq!(options.language.as_deref(), Some("de"));
        match options.family {
            Some(RunExtension::Whisper(whisper)) => {
                assert_eq!(whisper.initial_prompt.as_deref(), Some("Gregor Samsa"));
                assert_eq!(
                    whisper.condition_on_prev_tokens,
                    Some(true),
                    "later windows of a long take keep the prompt and the text before them"
                );
            },
            other => panic!("expected the Whisper extension, got {other:?}"),
        }
        assert_eq!(run_options(None, None).language, None);
    }

    #[test]
    fn parakeet_gets_no_hint_no_prompt_and_its_own_threads() {
        let options = parakeet_options();
        assert_eq!(options.task, Task::Transcribe);
        assert_eq!(options.language, None);
        assert_eq!(options.prompt, None);
        assert_eq!(options.family, None, "no Whisper extension");
        assert_eq!(options.timestamps, TimestampKind::None);
        assert!(!Family::Parakeet.takes_prompt());
        assert!(Family::Whisper.takes_prompt());
        assert_eq!(Family::Whisper.threads(true), 1);
        assert_eq!(Family::Parakeet.threads(true), cpu_threads());
        assert_eq!(Family::from_arch("parakeet"), Some(Family::Parakeet));
        assert_eq!(Family::from_arch("whisper"), Some(Family::Whisper));
        assert_eq!(Family::from_arch("canary"), None);
    }

    #[test]
    fn a_language_the_model_does_not_list_gets_no_hint() {
        let multilingual: Vec<String> = ["en", "de"].map(String::from).to_vec();
        let english_only = vec!["en".to_string()];
        assert_eq!(take_language("de", &multilingual), Some("de"));
        assert_eq!(take_language("auto", &multilingual), None, "auto detects");
        assert_eq!(
            take_language("de", &english_only),
            None,
            "an .en model gets no hint"
        );
        assert_eq!(take_language("en", &english_only), Some("en"));
        assert_eq!(
            take_language("de", &[]),
            Some("de"),
            "a model without a list"
        );
    }

    #[test]
    fn speech_stretches_are_joined_by_a_short_silence() {
        let samples: Vec<f32> = (0..48_000).map(|i| 1.0 + i as f32).collect();
        let (speech, starts) = join_speech(&samples, &[(8_000, 16_000), (32_000, 40_000)]);
        let first = 8_000..16_000 + SPEECH_OVERLAP;
        let second = 32_000..40_000;
        assert_eq!(speech.len(), first.len() + SPEECH_GAP + second.len());
        assert_eq!(starts, [0, first.len() + SPEECH_GAP]);
        assert_eq!(speech[starts[1]], samples[32_000]);
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
        let (speech, _) = join_speech(&samples, &[(0, 16_000), (16_160, 24_000)]);
        let kept: Vec<f32> = speech.into_iter().filter(|s| *s != 0.0).collect();
        assert!(
            kept.windows(2).all(|pair| pair[0] < pair[1]),
            "no sample is copied twice"
        );
        assert_eq!(kept.last(), Some(&23_999.0));
    }

    #[test]
    fn touching_stretches_become_one_without_silence_inside() {
        assert_eq!(
            merge_touching([(2_500, 77_000), (77_000, 149_500), (160_000, 170_000)]),
            [(2_500, 149_500), (160_000, 170_000)]
        );
        assert_eq!(merge_touching([(0, 100), (50, 80)]), [(0, 100)]);
        assert!(merge_touching([]).is_empty());

        let samples: Vec<f32> = (0..160_000).map(|i| 1.0 + i as f32).collect();
        let spans = merge_touching([(2_500, 77_000), (77_000, 149_500)]);
        let (speech, starts) = join_speech(&samples, &spans);
        assert_eq!(speech.len(), 147_000);
        assert_eq!(starts, [0]);
        assert!(speech.iter().all(|s| *s != 0.0), "no silence mid-speech");
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
    }

    #[test]
    fn silence_reaches_whisper_neither_with_nor_without_vad() {
        let quiet = vec![0.002f32; 16_000];
        assert!(level::rms(&quiet) < level::QUIET_RMS);
        assert_eq!(whisper_input(&quiet, Some(&[])), None, "VAD found nothing");
        assert_eq!(whisper_input(&quiet, None), None, "VAD is off");
        let silence = vec![0.0f32; 48_000];
        assert_eq!(whisper_input(&silence, None), None, "digital silence");
    }

    #[test]
    fn no_speech_and_bad_spans_give_nothing() {
        let samples = vec![0.5f32; 16_000];
        assert_eq!(join_speech(&samples, &[]), (vec![], vec![]));
        assert_eq!(join_speech(&samples, &[(12_800, 6_400)]), (vec![], vec![]));
        assert_eq!(join_speech(&samples, &[(14_400, 80_000)]).0.len(), 1_600);
        assert_eq!(join_speech(&[], &[(0, 16_000)]), (vec![], vec![]));
    }

    #[test]
    fn the_second_out_of_memory_take_moves_the_session_to_the_cpu() {
        assert!(retries_on_a_copy(0), "the first runs on a CPU copy");
        assert!(!retries_on_a_copy(1));
        assert!(!retries_on_a_copy(7));
    }

    const SECOND: usize = WHISPER_SAMPLE_RATE as usize;

    /// Speech at one level from end to end.
    fn loud(len: usize) -> Vec<f32> {
        (0..len)
            .map(|i| if i % 2 == 0 { 0.3 } else { -0.3 })
            .collect()
    }

    /// The pieces follow each other from 0 to `len`, and none is empty or
    /// longer than `max`.
    fn assert_tiles(pieces: &[Range<usize>], len: usize, max: usize) {
        let mut at = 0;
        for piece in pieces {
            assert_eq!(piece.start, at, "{pieces:?}");
            assert!(!piece.is_empty() && piece.len() <= max, "{pieces:?}");
            at = piece.end;
        }
        assert_eq!(at, len, "{pieces:?}");
    }

    #[test]
    fn a_long_parakeet_take_is_cut_in_the_silence_between_stretches() {
        let starts = [0, 20 * SECOND, 45 * SECOND, 60 * SECOND];
        let half_gap = SPEECH_GAP / 2;
        assert_eq!(
            pieces(&loud(70 * SECOND), &starts, PARAKEET_PIECE),
            [
                0..20 * SECOND - half_gap,
                20 * SECOND - half_gap..45 * SECOND - half_gap,
                45 * SECOND - half_gap..70 * SECOND,
            ]
        );
    }

    #[test]
    fn without_a_stretch_the_cut_falls_in_the_quietest_block_before_the_limit() {
        let mut take = loud(70 * SECOND);
        take[27 * SECOND..27 * SECOND + SECOND / 5].fill(0.0);
        take[55 * SECOND..55 * SECOND + SECOND / 5].fill(0.0);
        let cuts = pieces(&take, &[], PARAKEET_PIECE);
        assert_tiles(&cuts, take.len(), PARAKEET_PIECE);
        assert_eq!(cuts.len(), 3, "{cuts:?}");
        let quiet = |at: usize| at..at + SECOND / 5;
        assert!(quiet(27 * SECOND).contains(&cuts[0].end), "{cuts:?}");
        assert!(quiet(55 * SECOND).contains(&cuts[1].end), "{cuts:?}");

        let after_a_stretch = pieces(&take, &[0, 40 * SECOND], PARAKEET_PIECE);
        assert!(
            quiet(27 * SECOND).contains(&after_a_stretch[0].end),
            "no stretch starts within the first 30 s: {after_a_stretch:?}"
        );
    }

    #[test]
    fn a_cut_without_a_quiet_block_stays_within_the_last_seconds() {
        let take = loud(70 * SECOND);
        let cuts = pieces(&take, &[], PARAKEET_PIECE);
        assert_tiles(&cuts, take.len(), PARAKEET_PIECE);
        assert!(cuts[0].end >= PARAKEET_PIECE - CUT_SEARCH, "{cuts:?}");

        let take = loud(30 * SECOND + SECOND / 2);
        let cuts = pieces(&take, &[], PARAKEET_PIECE);
        assert_tiles(&cuts, take.len(), PARAKEET_PIECE);
        assert_eq!(cuts.len(), 2, "{cuts:?}");
        assert!(cuts[1].len() >= MIN_PIECE, "the last piece keeps a second");
    }

    #[test]
    fn a_short_take_is_one_piece() {
        let whole = |len: usize| vec![Range { start: 0, end: len }];
        assert_eq!(
            pieces(&loud(30 * SECOND), &[0, 10 * SECOND], PARAKEET_PIECE),
            whole(30 * SECOND)
        );
        assert_eq!(pieces(&loud(SECOND), &[], PARAKEET_PIECE), whole(SECOND));
        let take = loud(3 * SECOND);
        assert_tiles(&pieces(&take, &[], 0), take.len(), MIN_PIECE);
    }

    #[test]
    fn resample_is_identity_for_equal_rates() {
        let input = vec![0.5f32; 100];
        assert_eq!(resample(&input, 16_000, 16_000).unwrap(), input);
    }
}
