//! `hush tune`: time a model on the CPU and on the GPU, each in a worker
//! process of its own, and recommend the most accurate model that keeps up.
//! The audio is the take that the daemon kept for the tune, a file, or a
//! public-domain clip that comes with Hush.

use super::daemon::SessionOptions;
use crate::config::{paths, Config};
use crate::ipc::client;
use crate::ipc::protocol::{DaemonCommand, DaemonState};
use crate::transcription::gpu_fallback::{GpuFingerprint, GPU_BUILD};
use crate::transcription::models::{ModelManager, ModelSize};
use crate::transcription::tune::{
    self, DeviceKind, Measured, Pick, Preference, TuneResult, LADDER,
};
use crate::transcription::WHISPER_SAMPLE_RATE;
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// "And so my fellow Americans..." from whisper.cpp's samples, public domain.
const CLIP: &[u8] = include_bytes!("../../../tests/e2e/samples/jfk.wav");
const CLIP_WORDS: &str =
    "And so my fellow Americans, ask not what your country can do for you, ask what you can do for your country.";

/// How long one worker may take, the model load and two passes included.
const WORKER_TIMEOUT: Duration = Duration::from_secs(300);

/// A transcript that misses more than half of the words read means a
/// different sentence, not a worse model, so the tune shows no accuracy.
const MAX_SCORED_ERROR: f32 = 0.5;

pub struct TuneOptions {
    pub audio: Option<PathBuf>,
    /// The words that the audio holds, for the accuracy
    pub reference: Option<String>,
    pub prefer: Preference,
    /// The model to time; base when unset
    pub model: Option<String>,
    pub json: bool,
    /// Download and select the pick, time it, and step down once if it
    /// misses the budget
    pub apply: bool,
}

/// What a worker prints: one JSON line.
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkerReport {
    pub speed: Option<f32>,
    pub text: Option<String>,
    pub gpu: Option<GpuFingerprint>,
    pub memory_total: Option<u64>,
    pub error: Option<String>,
}

pub async fn handle_tune(options: TuneOptions) -> Result<()> {
    let config = Config::load()
        .or_else(|_| Config::defaults())
        .context("Failed to read the settings")?;
    let audio = TuneAudio::choose(options.audio.clone(), options.reference.clone())?;
    let language = if audio.bundled {
        "en".to_string()
    } else {
        config.transcription.language.clone()
    };
    let size: ModelSize = match &options.model {
        Some(name) => name.parse()?,
        None => ModelSize::Base,
    };
    let manager = ModelManager::new(paths::models_dir())?;
    let model = manager.get_model_path(&size).ok_or_else(|| {
        anyhow!(
            "The {} model is not downloaded. Download it with: hush models download {}",
            size,
            size
        )
    })?;
    if !options.json {
        println!("Timing {} on {} ...", size, audio.describe());
    }

    let unloaded = unload_daemon().await;
    let measured = measure_all(&model, size, &audio, &language).await;
    if unloaded {
        let _ = client::send(DaemonCommand::Reload).await;
    }
    let (runs, gpu) = measured?;
    let pick = tune::pick(&runs, options.prefer)
        .ok_or_else(|| anyhow!("No device finished the timing; run with -v to see why"))?;
    let mut result = TuneResult {
        gpu,
        preference: options.prefer,
        measured: runs,
        pick,
    };
    if options.apply {
        result.pick = apply(&result, &audio, &language).await?;
    }
    result
        .save_to(&TuneResult::path())
        .context("Failed to keep the tune's result")?;
    if options.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        print_result(&result, options.apply);
    }
    audio.finish();
    Ok(())
}

/// The audio of the tune and the words it holds, when known.
struct TuneAudio {
    path: PathBuf,
    reference: Option<String>,
    bundled: bool,
    /// The kept take, which the tune removes when it is done
    kept: bool,
}

impl TuneAudio {
    fn choose(file: Option<PathBuf>, reference: Option<String>) -> Result<Self> {
        if let Some(path) = file {
            return Ok(Self {
                path,
                reference,
                bundled: false,
                kept: false,
            });
        }
        let kept = crate::audio::store::tune_take_path();
        if kept.exists() {
            return Ok(Self {
                path: kept,
                reference,
                bundled: false,
                kept: true,
            });
        }
        let dir = crate::ipc::paths::ensure_runtime_dir()?;
        let path = dir.join("tune-clip.wav");
        std::fs::write(&path, CLIP).context("Failed to write the tune's clip")?;
        Ok(Self {
            path,
            reference: Some(CLIP_WORDS.to_string()),
            bundled: true,
            kept: false,
        })
    }

    fn describe(&self) -> String {
        match (self.bundled, self.kept) {
            (true, _) => "a public-domain clip".to_string(),
            (false, true) => "your kept take".to_string(),
            (false, false) => self.path.display().to_string(),
        }
    }

    fn finish(self) {
        if self.bundled || self.kept {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Free the daemon's model while the workers run. False when no daemon
/// runs, when it is busy, or when it is older than the `unload` command.
async fn unload_daemon() -> bool {
    match client::send(DaemonCommand::Unload).await {
        Ok(reply) if reply.ok => true,
        Ok(reply) => {
            tracing::info!(
                "The daemon keeps its model during the tune: {}",
                reply.error.unwrap_or_default()
            );
            false
        },
        Err(_) => false,
    }
}

async fn measure_all(
    model: &Path,
    size: ModelSize,
    audio: &TuneAudio,
    language: &str,
) -> Result<(Vec<Measured>, Option<GpuFingerprint>)> {
    let mut runs = Vec::new();
    let mut gpu = None;
    let devices: &[DeviceKind] = if GPU_BUILD {
        &[DeviceKind::Gpu, DeviceKind::Cpu]
    } else {
        &[DeviceKind::Cpu]
    };
    for device in devices {
        match run_worker(*device, model, &audio.path, language).await {
            Ok(report) => {
                if let Some(error) = report.error {
                    tracing::info!(?device, "No timing: {}", error);
                    continue;
                }
                let Some(speed) = report.speed else {
                    continue;
                };
                if *device == DeviceKind::Gpu {
                    gpu = report.gpu;
                }
                let word_error_rate = audio
                    .reference
                    .as_deref()
                    .zip(report.text.as_deref())
                    .map(|(reference, text)| tune::word_error_rate(reference, text))
                    .filter(|rate| *rate <= MAX_SCORED_ERROR);
                runs.push(Measured {
                    device: *device,
                    model: size,
                    speed,
                    memory_total: report.memory_total,
                    word_error_rate,
                });
            },
            Err(e) => tracing::warn!(?device, "The timing failed: {:#}", e),
        }
    }
    if runs.is_empty() {
        bail!("Neither the CPU nor the GPU finished the timing");
    }
    Ok((runs, gpu))
}

/// Time `model` on `device` in a worker process. A take that starts in the
/// daemon meanwhile stops the worker, so dictation never waits for the tune.
async fn run_worker(
    device: DeviceKind,
    model: &Path,
    audio: &Path,
    language: &str,
) -> Result<WorkerReport> {
    let exe = std::env::current_exe().context("Failed to find the hush binary")?;
    let mut command = tokio::process::Command::new(exe);
    command
        .arg("tune-worker")
        .arg("--model")
        .arg(model)
        .arg("--audio")
        .arg(audio)
        .arg("--language")
        .arg(language)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    match device {
        DeviceKind::Gpu => {
            command.arg("--gpu");
        },
        // The CPU worker never loads a GPU module, so a GPU fault cannot
        // spoil the CPU's timing.
        DeviceKind::Cpu => {
            command.env("TRANSCRIBE_BACKENDS", "cpu");
        },
    }
    let child = command.spawn().context("Failed to start the tune worker")?;
    let output = child.wait_with_output();
    tokio::pin!(output);
    let deadline = tokio::time::sleep(WORKER_TIMEOUT);
    tokio::pin!(deadline);
    let mut watch = tokio::time::interval(Duration::from_millis(250));
    loop {
        tokio::select! {
            finished = &mut output => {
                let output = finished.context("The tune worker failed")?;
                let line = String::from_utf8_lossy(&output.stdout);
                let line = line.lines().last().unwrap_or_default();
                return serde_json::from_str(line)
                    .with_context(|| format!("The tune worker printed no report: {}", line));
            },
            _ = &mut deadline => bail!("The tune worker took more than {} s", WORKER_TIMEOUT.as_secs()),
            _ = watch.tick() => {
                if matches!(client::status().await, Some(state) if state != DaemonState::Idle) {
                    bail!("A take started, so the tune stopped; run it again when Hush is idle");
                }
            },
        }
    }
}

/// `hush tune-worker`: load the model on the devices that the environment
/// allows, transcribe the audio once to warm up, time a second pass, and
/// print one JSON line.
pub async fn handle_tune_worker(
    model: PathBuf,
    audio: PathBuf,
    language: String,
    gpu: bool,
) -> Result<()> {
    let report = match time_model(&model, &audio, &language, gpu).await {
        Ok(report) => report,
        Err(e) => WorkerReport {
            speed: None,
            text: None,
            gpu: None,
            memory_total: None,
            error: Some(format!("{:#}", e)),
        },
    };
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}

async fn time_model(model: &Path, audio: &Path, language: &str, gpu: bool) -> Result<WorkerReport> {
    use crate::transcription::device::GpuAvailability;
    use crate::transcription::whisper::WhisperTranscriber;
    let samples = crate::audio::file::read_for_whisper(audio)?;
    let seconds = samples.len() as f32 / WHISPER_SAMPLE_RATE as f32;
    let (fingerprint, memory_total) = if gpu {
        crate::transcription::device::load_backends(true).await?;
        let availability = GpuAvailability::detect();
        if !availability.available {
            bail!("ggml found no GPU");
        }
        let memory = transcribe_cpp::devices()
            .into_iter()
            .filter(|device| device.kind != "cpu")
            .map(|device| device.memory_total)
            .max();
        (Some(GpuFingerprint::current(availability)), memory)
    } else {
        (None, None)
    };
    let whisper = WhisperTranscriber::new(model, gpu)
        .await?
        .with_language(language);
    whisper
        .transcribe_async(&samples, WHISPER_SAMPLE_RATE)
        .await?;
    let started = Instant::now();
    let result = whisper
        .transcribe_async(&samples, WHISPER_SAMPLE_RATE)
        .await?;
    let took = started.elapsed().as_secs_f32().max(1e-3);
    Ok(WorkerReport {
        speed: Some(seconds / took),
        text: Some(result.text),
        gpu: fingerprint,
        memory_total,
        error: None,
    })
}

/// Select the pick, time it on its device, and step down once when it
/// misses the budget. Returns the model that stays selected.
async fn apply(result: &TuneResult, audio: &TuneAudio, language: &str) -> Result<Pick> {
    let mut pick = result.pick;
    let rank = |model: ModelSize| LADDER.iter().position(|step| *step == model).unwrap_or(0);
    super::models::set_model(&pick.model.to_string()).await?;
    let manager = ModelManager::new(paths::models_dir())?;
    if let Some(path) = manager.get_model_path(&pick.model) {
        if let Ok(report) = run_worker(pick.device, &path, &audio.path, language).await {
            if let Some(speed) = report.speed {
                pick.speed = speed;
                let rank = rank(pick.model);
                if speed < result.preference.min_speed() && rank > 0 {
                    let lower = LADDER[rank - 1];
                    println!(
                        "{} runs at {:.1}x real time, below the budget, so Hush takes {}",
                        pick.model, speed, lower
                    );
                    super::models::set_model(&lower.to_string()).await?;
                    pick.model = lower;
                }
            }
        }
    }
    if client::status().await.is_some() {
        super::daemon::handle_daemon_restart(SessionOptions {
            editing_mode: "light".to_string(),
            no_processing: false,
            no_button: false,
        })
        .await?;
    }
    Ok(pick)
}

fn print_result(result: &TuneResult, applied: bool) {
    println!();
    for run in &result.measured {
        let device = match run.device {
            DeviceKind::Gpu => result
                .gpu
                .as_ref()
                .map_or("GPU".to_string(), |gpu| gpu.gpu.clone()),
            DeviceKind::Cpu => "CPU".to_string(),
        };
        let accuracy = run
            .word_error_rate
            .map(|rate| format!(", {:.0}% of the words right", (1.0 - rate) * 100.0))
            .unwrap_or_default();
        println!(
            "  {} on {}: {:.1}x real time{}",
            run.model, device, run.speed, accuracy
        );
    }
    let device = match result.pick.device {
        DeviceKind::Gpu => "the GPU",
        DeviceKind::Cpu => "the CPU",
    };
    println!();
    if applied {
        println!(
            "Hush now uses {} on {} (about {:.0}x real time).",
            result.pick.model, device, result.pick.speed
        );
    } else {
        println!(
            "Recommended: {} on {} (about {:.0}x real time, estimated).",
            result.pick.model, device, result.pick.speed
        );
        println!("Run `hush tune --apply` to download it, time it, and use it.");
    }
}
