//! `hush doctor`: everything that has to be true for dictation to work, each
//! with the one command that fixes it.

use crate::config::Config;
use crate::hotkey::HotkeyBindings;
use crate::text::WindowProvider;
use crate::transcription::device::GpuAvailability;
use crate::transcription::models::{ModelManager, ModelSize};
use crate::AudioCapture;
use anyhow::Result;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Working.
    Ok,
    /// Works, but something is unavailable or degraded.
    Warn,
    /// Dictation will not work until this is fixed.
    Fail,
}

impl Level {
    fn mark(self) -> &'static str {
        match self {
            Level::Ok => "✅",
            Level::Warn => "⚠️ ",
            Level::Fail => "❌",
        }
    }
}

pub struct Check {
    pub name: &'static str,
    pub level: Level,
    pub detail: String,
    /// The command that fixes it.
    pub fix: Option<String>,
}

impl Check {
    fn ok(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            level: Level::Ok,
            detail: detail.into(),
            fix: None,
        }
    }

    fn warn(name: &'static str, detail: impl Into<String>, fix: impl Into<String>) -> Self {
        Self {
            name,
            level: Level::Warn,
            detail: detail.into(),
            fix: Some(fix.into()),
        }
    }

    fn fail(name: &'static str, detail: impl Into<String>, fix: impl Into<String>) -> Self {
        Self {
            name,
            level: Level::Fail,
            detail: detail.into(),
            fix: Some(fix.into()),
        }
    }
}

impl fmt::Display for Check {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {:<18} {}", self.level.mark(), self.name, self.detail)
    }
}

/// Everything `hush doctor` looks at, in the order it matters.
pub async fn checks() -> Vec<Check> {
    let mut checks = Vec::new();
    let config = match Config::load() {
        Ok(config) => {
            checks.push(Check::ok(
                "Configuration",
                format!("{}", crate::config::paths::config_path().display()),
            ));
            config
        },
        Err(e) => {
            checks.push(Check::fail(
                "Configuration",
                e.to_string(),
                "hush setup init --force",
            ));
            match Config::defaults() {
                Ok(defaults) => defaults,
                Err(_) => return checks,
            }
        },
    };

    checks.push(permissions_check());
    checks.push(model_check(&config));
    checks.push(gpu_check(&config));
    checks.push(microphone_check(&config));
    checks.push(hotkey_check(&config));
    checks.push(window_check());
    checks.push(llm_check(&config).await);
    checks.push(tray_check(&config));
    checks.push(autostart_check());
    checks.push(daemon_check().await);
    checks
}

fn permissions_check() -> Check {
    let status = crate::permissions::status();
    if status.ready() {
        return Check::ok(
            "Keyboard access",
            format!(
                "{} of {} devices readable, uinput writable",
                status.readable_event_nodes, status.event_nodes
            ),
        );
    }
    let detail = if status.in_container {
        "running in a container; the rule belongs on the host".to_string()
    } else if !status.uinput_writable {
        "cannot write /dev/uinput, so Hush cannot type".to_string()
    } else {
        format!("no readable keyboard ({} devices)", status.event_nodes)
    };
    Check::fail("Keyboard access", detail, "hush setup permissions")
}

fn model_check(config: &Config) -> Check {
    let size = config.transcription.model_size.clone();
    let path = config.transcription.model_path();
    if path.exists() {
        let megabytes = std::fs::metadata(&path)
            .map(|meta| meta.len() / 1_000_000)
            .unwrap_or(0);
        return Check::ok("Speech model", format!("{} ({} MB)", size, megabytes));
    }
    if let Ok(manager) = ModelManager::new(crate::config::paths::models_dir()) {
        if let Ok(parsed) = size.parse::<ModelSize>() {
            if manager.get_model_path(&parsed).is_some() {
                return Check::ok("Speech model", format!("{} installed", size));
            }
        }
    }
    Check::fail(
        "Speech model",
        format!("{} is not downloaded", size),
        format!("hush models download {}", size),
    )
}

fn gpu_check(config: &Config) -> Check {
    let gpu = GpuAvailability::detect();
    let built_with = if cfg!(feature = "cuda") {
        "CUDA"
    } else if cfg!(feature = "vulkan") {
        "Vulkan"
    } else {
        "none"
    };
    if built_with == "none" {
        return Check::warn(
            "GPU",
            "this build is CPU only",
            "install the release build, or build with `make release`",
        );
    }
    if !config.transcription.use_gpu {
        return Check::warn(
            "GPU",
            format!("{} available but switched off", built_with),
            "set use_gpu = true under [transcription]",
        );
    }
    if gpu.available {
        Check::ok("GPU", format!("{} on {}", built_with, gpu.device_name))
    } else {
        Check::warn(
            "GPU",
            format!("{} build, but no device answered", built_with),
            "check your graphics driver; Hush falls back to the CPU",
        )
    }
}

fn microphone_check(config: &Config) -> Check {
    match AudioCapture::new(config.audio.device.as_deref()) {
        Ok(capture) => Check::ok("Microphone", capture.get_device_name()),
        Err(e) => Check::fail(
            "Microphone",
            e.to_string(),
            "hush status --devices lists what is available",
        ),
    }
}

fn hotkey_check(config: &Config) -> Check {
    let bindings =
        match HotkeyBindings::parse(&config.hotkey.combination, Some(&config.hotkey.cancel))
            .and_then(|bindings| bindings.with_command(&config.hotkey.command))
        {
            Ok(bindings) => bindings,
            Err(e) => {
                return Check::fail(
                    "Hotkey",
                    e.to_string(),
                    "fix combination under [hotkey], or run hush settings",
                )
            },
        };
    let devices = crate::hotkey::evdev::devices_for(&bindings).len();
    if devices == 0 {
        return Check::fail(
            "Hotkey",
            format!("no device carries {}", config.hotkey.combination),
            "hush setup permissions, or pick another key in hush settings",
        );
    }
    Check::ok(
        "Hotkey",
        format!(
            "{} on {} device(s){}",
            config.hotkey.combination,
            devices,
            if config.hotkey.exclusive {
                ", exclusive"
            } else {
                ""
            }
        ),
    )
}

fn window_check() -> Check {
    let provider = WindowProvider::detect();
    match provider.name() {
        "none" => Check::warn(
            "Focused window",
            "no compositor backend answered",
            "tone by app and the spelling prompt are off; everything else works",
        ),
        name => match provider.focused() {
            Some(info) => Check::ok("Focused window", format!("{} ({})", name, info.class)),
            None => Check::warn(
                "Focused window",
                format!("{} reported no window", name),
                "tone by app falls back to plain prose",
            ),
        },
    }
}

async fn llm_check(config: &Config) -> Check {
    use crate::text_processing::LlmProvider;
    let provider = crate::text_processing::llm::resolve_provider(&config.llm).await;
    match provider {
        LlmProvider::Ollama { model, .. } => Check::ok("Polishing", format!("Ollama {}", model)),
        LlmProvider::Anthropic { model, .. } => {
            Check::ok("Polishing", format!("Anthropic {}", model))
        },
        LlmProvider::None => Check::warn(
            "Polishing",
            "rule-based only; Command Mode is off",
            "run Ollama, or put ANTHROPIC_API_KEY in ~/.config/hush/.env",
        ),
    }
}

fn tray_check(config: &Config) -> Check {
    if !config.tray.enabled {
        return Check::ok("Panel icon", "switched off");
    }
    if std::env::var_os("WAYLAND_DISPLAY").is_none() && std::env::var_os("DISPLAY").is_none() {
        return Check::warn(
            "Panel icon",
            "no desktop session",
            "the icon appears when you run Hush inside your session",
        );
    }
    Check::ok("Panel icon", "on; the panel decides whether to show it")
}

fn autostart_check() -> Check {
    if crate::desktop::autostart_installed() {
        Check::ok("Starts at login", "yes")
    } else {
        Check::warn("Starts at login", "no", "hush install --autostart")
    }
}

async fn daemon_check() -> Check {
    match crate::ipc::client::status().await {
        Some(state) => Check::ok("Hush", format!("running, {}", state)),
        None => Check::warn("Hush", "not running", "hush daemon start"),
    }
}

pub async fn handle_doctor() -> Result<()> {
    println!("🩺 Hush");
    println!();
    let checks = checks().await;
    for check in &checks {
        println!("{}", check);
    }

    let problems: Vec<&Check> = checks
        .iter()
        .filter(|check| check.level != Level::Ok)
        .collect();
    println!();
    if problems.is_empty() {
        println!("Everything is ready.");
        return Ok(());
    }
    println!("To fix:");
    for check in &problems {
        if let Some(fix) = &check.fix {
            println!("  {:<18} {}", check.name, fix);
        }
    }
    if checks.iter().any(|check| check.level == Level::Fail) {
        println!();
        println!("Dictation will not work until the ❌ lines are fixed.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_check_prints_its_mark_and_name() {
        let check = Check::fail(
            "Speech model",
            "base is not downloaded",
            "hush models download base",
        );
        let line = check.to_string();
        assert!(line.starts_with("❌"));
        assert!(line.contains("Speech model"));
        assert!(line.contains("not downloaded"));
        assert_eq!(check.fix.as_deref(), Some("hush models download base"));
        assert!(Check::ok("Hotkey", "RightAlt").fix.is_none());
    }

    #[test]
    fn the_model_check_names_the_command_that_fixes_it() {
        let mut config = Config::defaults().unwrap();
        config.transcription.model_size = "definitely-not-a-model".to_string();
        let check = model_check(&config);
        assert_eq!(check.level, Level::Fail);
        assert_eq!(
            check.fix.as_deref(),
            Some("hush models download definitely-not-a-model")
        );
    }

    #[test]
    fn an_unparseable_hotkey_is_a_failure() {
        let mut config = Config::defaults().unwrap();
        config.hotkey.combination = "Ctrl+Nope".to_string();
        let check = hotkey_check(&config);
        assert_eq!(check.level, Level::Fail);
        assert!(check.detail.contains("Nope"));
    }

    #[test]
    fn a_switched_off_tray_is_not_a_problem() {
        let mut config = Config::defaults().unwrap();
        config.tray.enabled = false;
        assert_eq!(tray_check(&config).level, Level::Ok);
    }

    #[tokio::test]
    async fn the_checks_run_without_panicking() {
        let checks = checks().await;
        assert!(checks.len() >= 8, "every area is reported");
        assert!(checks.iter().any(|check| check.name == "Keyboard access"));
        for check in &checks {
            if check.level == Level::Ok {
                assert!(check.fix.is_none(), "{} needs no fix", check.name);
            } else {
                assert!(check.fix.is_some(), "{} suggests a fix", check.name);
            }
        }
    }
}
