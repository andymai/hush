use super::utils::{print_line, show_config_status, show_device_status, show_system_info};
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
/// * `_config` - Reserved for future use to show only configuration details
/// * `_devices` - Reserved for future use to show only device details
/// * `full` - Follow with the `hush doctor` report
///
/// # Note
///
/// Currently, config and device status are always shown. The `_config` and `_devices`
/// parameters are reserved for future enhancement to allow selective display.
///
/// # Examples
///
/// ```no_run
/// # use hush::cli::commands::handle_status;
/// # async fn example() -> anyhow::Result<()> {
/// // Basic status
/// handle_status(false, false, false).await?;
///
/// // Followed by the doctor report
/// handle_status(false, false, true).await?;
/// # Ok(())
/// # }
/// ```
pub async fn handle_status(_config: bool, _devices: bool, full: bool) -> Result<()> {
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
    println!();

    // Always show basic status
    show_system_info().await?;
    println!();
    show_config_status().await?;
    println!();
    show_device_status().await?;

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
            .with_language(Some("de".to_string()));
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

    #[tokio::test]
    async fn test_status_command_runs() {
        crate::test_support::in_temp_home!(test_status_command_runs);
        // In an empty home no daemon runs, so only a panic fails the test.
        let _ = handle_status(false, false, false).await;
    }
}
