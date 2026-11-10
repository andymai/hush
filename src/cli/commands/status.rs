use super::utils::{config_section, device_section, print_line, show_system_info};
use crate::audio::take::StartDelay;
use crate::cli::exit::{Exit, ExitStatus};
use crate::config::settings::TranscriptionBackend;
use crate::ipc::{DaemonResponse, DaemonState, ModelState};
use crate::transcription::GpuAvailability;
use crate::{AudioCapture, Config};
/// Status command implementation
///
/// Displays system status including configuration, devices, and component health.
use anyhow::Result;
use serde::Serialize;
use std::path::Path;

/// Handle the status command
///
/// # Arguments
///
/// * `config` - Print only the Configuration section
/// * `devices` - Print only the Audio Devices section
/// * `full` - Follow with the `hush doctor` report
///
/// With `config` or `devices`, the report has only those sections, without
/// the daemon state, and the exit code does not depend on the daemon.
///
/// # Examples
///
/// ```no_run
/// # use hush::cli::commands::handle_status;
/// # async fn example() -> anyhow::Result<()> {
/// // Basic status
/// handle_status(false, false, false).await?;
///
/// // Only the input devices
/// handle_status(false, true, false).await?;
///
/// // Followed by the doctor report
/// handle_status(false, false, true).await?;
/// # Ok(())
/// # }
/// ```
pub async fn handle_status(config: bool, devices: bool, full: bool) -> Result<()> {
    if config || devices {
        print!(
            "{}",
            only_sections(
                config.then(Config::load),
                devices.then(AudioCapture::list_devices),
            )
        );
        if full {
            println!();
            super::doctor::handle_doctor().await?;
        }
        return Ok(());
    }

    println!("🤫 Hush System Status");
    println!();

    print!("Daemon: ");
    let reply = crate::ipc::client::status_reply().await;
    let running = match reply.as_ref().and_then(|reply| reply.state) {
        Some(state) => {
            println!("✅ running ({})", state);
            true
        },
        None => {
            println!("not running (start with `hush daemon start`)");
            false
        },
    };
    if let Some(model) = reply.as_ref().and_then(|reply| reply.model_state) {
        println!("Model: {}", model);
    }
    if let Some(reason) = reply.as_ref().and_then(|reply| reply.safe_mode.as_deref()) {
        println!("Safe mode: {}", safe_mode_line(reason));
    }
    if let Some(delay) = reply.as_ref().and_then(|reply| reply.start_delay) {
        println!("Microphone start: {}", start_delay_line(&delay));
    }
    println!();

    show_system_info().await?;
    println!();
    print!("{}", config_section(Config::load()));
    println!();
    print!("{}", device_section(AudioCapture::list_devices()));

    if full {
        println!();
        super::doctor::handle_doctor().await?;
    }

    if running {
        Ok(())
    } else {
        Exit::quiet_result(ExitStatus::NoDaemon)
    }
}

/// The sections that `--config` and `--devices` ask for, in the order of
/// the full report, with a blank line between them.
fn only_sections(config: Option<Result<Config>>, devices: Option<Result<Vec<String>>>) -> String {
    [config.map(config_section), devices.map(device_section)]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n")
}

/// What `hush status --json` prints. Every key is present; unknown values
/// are null. The README lists the keys.
#[derive(Debug, PartialEq, Serialize)]
pub struct StatusReport {
    pub state: &'static str,
    pub running: bool,
    pub elapsed_ms: Option<u64>,
    pub model_state: Option<ModelState>,
    pub backend: Option<TranscriptionBackend>,
    pub model: Option<String>,
    pub model_path: Option<String>,
    pub use_gpu: Option<bool>,
    pub gpu: Option<String>,
    pub gpu_type: String,
    pub sample_rate: Option<u32>,
    pub hotkey: Option<String>,
    /// The language of the take in progress or of the next take; without a
    /// daemon, `transcription.language`
    pub language: Option<String>,
    pub devices: Vec<String>,
    pub os: &'static str,
    pub arch: &'static str,
    pub config_error: Option<String>,
    /// Why the daemon runs in safe mode, else `None`
    pub safe_mode: Option<String>,
    /// How long recent takes waited for the first audio, else `None`
    pub start_delay: Option<StartDelay>,
}

/// The start delay of recent takes, for `hush status`.
pub fn start_delay_line(delay: &StartDelay) -> String {
    if delay.takes == 1 {
        return format!("{} ms in the last take", delay.last_ms);
    }
    format!(
        "{} ms in the last take, {} ms at most in the last {} takes",
        delay.last_ms, delay.worst_ms, delay.takes
    )
}

/// What safe mode means, with its reason, for `hush status` and `hush doctor`.
pub fn safe_mode_line(reason: &str) -> String {
    format!(
        "the daemon writes and deletes nothing in the history and the kept audio, because {}. \
         Fix the file, then run hush daemon restart",
        reason
    )
}

impl StatusReport {
    /// `reply` is the daemon's status reply, `None` when no daemon runs.
    /// A model without an explicit path is looked for in `models_dir`.
    pub fn new(
        reply: Option<&DaemonResponse>,
        config: Result<Config>,
        models_dir: &Path,
        gpu: &GpuAvailability,
        devices: Vec<String>,
    ) -> Self {
        let (config, config_error) = match config {
            Ok(config) => (Some(config), None),
            Err(e) => (None, Some(format!("{:#}", e))),
        };
        let transcription = config.as_ref().map(|config| &config.transcription);
        let state = reply.and_then(|reply| reply.state);
        Self {
            state: state.as_ref().map_or("stopped", DaemonState::name),
            running: state.is_some(),
            elapsed_ms: match state {
                Some(DaemonState::Recording { elapsed_ms }) => Some(elapsed_ms),
                _ => None,
            },
            model_state: reply.and_then(|reply| reply.model_state),
            backend: transcription.map(|t| t.backend),
            model: transcription.map(|t| match t.cloud_model() {
                Some(model) => model.to_string(),
                None => t.model_size.clone(),
            }),
            model_path: transcription
                .filter(|t| !t.backend.is_cloud())
                .map(|t| t.model_path_in(models_dir).display().to_string()),
            use_gpu: transcription.map(|t| t.use_gpu),
            gpu: gpu.available.then(|| gpu.device_name.clone()),
            gpu_type: gpu.gpu_type.to_string().to_lowercase(),
            sample_rate: config.as_ref().map(|config| config.audio.sample_rate),
            hotkey: config
                .as_ref()
                .map(|config| config.hotkey.combination.clone()),
            language: reply
                .and_then(|reply| reply.language.clone())
                .or_else(|| transcription.map(|t| t.language.clone())),
            devices,
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            config_error,
            safe_mode: reply.and_then(|reply| reply.safe_mode.clone()),
            start_delay: reply.and_then(|reply| reply.start_delay),
        }
    }
}

/// `hush status --json`
pub async fn handle_status_json() -> Result<()> {
    let reply = crate::ipc::client::status_reply().await;
    let report = StatusReport::new(
        reply.as_ref(),
        Config::load(),
        &crate::config::paths::models_dir(),
        GpuAvailability::detect(),
        AudioCapture::list_devices().unwrap_or_default(),
    );
    print_line(&serde_json::to_string(&report)?)?;
    if report.running {
        Ok(())
    } else {
        Exit::quiet_result(ExitStatus::NoDaemon)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcription::GpuType;

    fn cpu() -> GpuAvailability {
        GpuAvailability {
            gpu_type: GpuType::Cpu,
            device_name: "CPU".to_string(),
            available: false,
        }
    }

    #[test]
    fn json_status_has_stable_keys() {
        let reply = DaemonResponse::ok(DaemonState::Recording { elapsed_ms: 1500 })
            .with_model_state(Some(ModelState::Loading))
            .with_language(Some("de".to_string()))
            .with_start_delay(Some(StartDelay {
                last_ms: 30,
                worst_ms: 700,
                takes: 4,
            }));
        let report = StatusReport::new(
            Some(&reply),
            Config::defaults(),
            Path::new("/models"),
            &cpu(),
            vec!["Built-in Microphone".to_string()],
        );
        let value = serde_json::to_value(&report).unwrap();
        let mut keys: Vec<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "arch",
                "backend",
                "config_error",
                "devices",
                "elapsed_ms",
                "gpu",
                "gpu_type",
                "hotkey",
                "language",
                "model",
                "model_path",
                "model_state",
                "os",
                "running",
                "safe_mode",
                "sample_rate",
                "start_delay",
                "state",
                "use_gpu"
            ]
        );
        assert_eq!(value["state"], "recording");
        assert_eq!(value["running"], true);
        assert_eq!(value["elapsed_ms"], 1500);
        assert_eq!(value["backend"], "local");
        assert_eq!(value["model"], "base");
        assert_eq!(value["model_state"], "loading");
        assert_eq!(value["gpu"], serde_json::Value::Null);
        assert_eq!(value["gpu_type"], "cpu");
        assert_eq!(value["hotkey"], "RightAlt");
        assert_eq!(value["language"], "de", "the daemon's language");
        assert_eq!(value["model_path"], "/models/ggml-base.bin");
        assert_eq!(
            value["start_delay"],
            serde_json::json!({"last_ms": 30, "worst_ms": 700, "takes": 4})
        );

        let older = DaemonResponse::ok(DaemonState::Idle);
        let report = StatusReport::new(
            Some(&older),
            Config::defaults(),
            Path::new("/models"),
            &cpu(),
            vec![],
        );
        assert_eq!(
            report.language.as_deref(),
            Some("en"),
            "without one from the daemon, the config language"
        );
        assert_eq!(
            serde_json::to_value(&report).unwrap()["start_delay"],
            serde_json::Value::Null,
            "an older daemon or no take yet"
        );
    }

    #[test]
    fn the_start_delay_line_names_the_last_and_the_slowest_take() {
        let one = StartDelay {
            last_ms: 640,
            worst_ms: 640,
            takes: 1,
        };
        assert_eq!(start_delay_line(&one), "640 ms in the last take");
        let many = StartDelay {
            last_ms: 12,
            worst_ms: 640,
            takes: 10,
        };
        assert_eq!(
            start_delay_line(&many),
            "12 ms in the last take, 640 ms at most in the last 10 takes"
        );
    }

    #[test]
    fn json_status_without_a_daemon_or_config() {
        let report = StatusReport::new(
            None,
            Err(anyhow::anyhow!("bad toml")),
            Path::new("/models"),
            &cpu(),
            vec![],
        );
        assert_eq!(report.state, "stopped");
        assert!(!report.running);
        assert_eq!(report.model_state, None);
        assert_eq!(report.backend, None);
        assert_eq!(report.language, None);
        assert_eq!(report.config_error.as_deref(), Some("bad toml"));
    }

    #[test]
    fn devices_prints_only_the_input_devices() {
        let devices = vec!["Built-in Microphone".to_string(), "USB Mic".to_string()];
        let text = only_sections(None, Some(Ok(devices)));
        assert_eq!(
            text,
            "🎤 Audio Devices:\n  1. Built-in Microphone\n  2. USB Mic\n"
        );

        let text = only_sections(None, Some(Ok(vec![])));
        assert_eq!(text, "🎤 Audio Devices:\n  ⚠️ No audio devices found\n");
    }

    #[test]
    fn config_prints_only_the_settings() {
        let text = only_sections(Some(Config::defaults()), None);
        assert!(text.starts_with("📋 Configuration:\n"), "{text}");
        assert!(text.contains("  Hotkey: RightAlt\n"), "{text}");
        assert!(
            text.contains("  LLM: auto (Ollama, else Anthropic)\n"),
            "{text}"
        );
        for other in [
            "Audio Devices",
            "System Information",
            "Daemon",
            "Hush System Status",
        ] {
            assert!(!text.contains(other), "{other} in:\n{text}");
        }

        let text = only_sections(Some(Err(anyhow::anyhow!("bad toml"))), None);
        assert_eq!(
            text,
            "📋 Configuration:\n  ❌ Failed to load config: bad toml\n"
        );
    }

    #[test]
    fn config_and_devices_print_both_in_report_order() {
        let text = only_sections(
            Some(Config::defaults()),
            Some(Ok(vec!["Built-in Microphone".to_string()])),
        );
        let settings = text.find("📋 Configuration:").expect("the settings");
        let devices = text
            .find("\n\n🎤 Audio Devices:")
            .expect("a blank line, then the devices");
        assert!(settings < devices, "{text}");
        assert!(!text.contains("System Information"), "{text}");
    }

    #[test]
    fn json_takes_neither_config_nor_devices() {
        use clap::Parser;
        for args in [
            &["hush", "status", "--json", "--config"][..],
            &["hush", "status", "--json", "--devices"],
            &["hush", "status", "--json", "--follow", "--devices"],
        ] {
            let error = match crate::cli::Cli::try_parse_from(args) {
                Ok(_) => panic!("{args:?} must fail"),
                Err(error) => error,
            };
            assert_eq!(error.exit_code(), 2, "{args:?} is a usage error");
        }
        for args in [
            &["hush", "status", "--config", "--devices"][..],
            &["hush", "status", "--devices", "--full"],
        ] {
            assert!(crate::cli::Cli::try_parse_from(args).is_ok(), "{args:?}");
        }
    }

    #[tokio::test]
    async fn test_status_command_runs() {
        crate::test_support::in_temp_home!(test_status_command_runs);
        // In an empty home no daemon runs, so only a panic fails the test.
        let _ = handle_status(false, false, false).await;
    }
}
