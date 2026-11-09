//! `hush doctor`: everything that has to be true for dictation to work, each
//! with the one command that fixes it.

use crate::config::settings::TranscriptionConfig;
use crate::config::{keys, Config};
use crate::hotkey::HotkeyBindings;
use crate::text::WindowProvider;
use crate::text_processing::hook::{self, Program};
use crate::transcription::cpu;
use crate::transcription::device::GpuAvailability;
use crate::transcription::models::{recommended_model, ModelManager, ModelSize};
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
    if config.transcription.backend.is_cloud() {
        let key = config
            .transcription
            .backend
            .key_var()
            .and_then(keys::api_key);
        checks.push(cloud_check(&config.transcription, key.is_some()));
    } else {
        let cpu = cpu_report(cpu::floor(), &cpu::missing());
        let cpu_supported = cpu.level == Level::Ok;
        checks.push(cpu);
        checks.push(model_check(&config));
        // Below the floor, ggml lists no devices, so a GPU line would blame the driver.
        if cpu_supported {
            checks.push(gpu_check(&config));
        }
    }
    checks.push(microphone_check(&config));
    checks.push(hotkey_check(&config));
    checks.push(window_check());
    checks.push(clipboard_check(crate::text::clipboard::support()));
    checks.push(llm_check(&config).await);
    checks.push(hook_check(&config));
    checks.push(tray_check(&config));
    checks.push(updates_check(&config));
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

fn cpu_report(floor: Option<&str>, missing: &[&str]) -> Check {
    match floor {
        None => Check::ok("CPU", "this build has no CPU floor"),
        Some(floor) if missing.is_empty() => {
            Check::ok("CPU", format!("has the {} features of this build", floor))
        },
        Some(floor) => Check::fail(
            "CPU",
            format!(
                "does not have {}, which this {} build needs",
                cpu::feature_list(missing),
                floor
            ),
            cpu::FIX,
        ),
    }
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
    let suggested = recommended_model();
    let fix = match size.parse::<ModelSize>() {
        Ok(_) => format!("hush models download {}", size),
        Err(_) => format!(
            "hush models download {} (unknown model name '{}')",
            crate::gui::size_key(suggested),
            size
        ),
    };
    Check::fail("Speech model", format!("{} is not downloaded", size), fix)
}

fn cloud_check(config: &TranscriptionConfig, key_present: bool) -> Check {
    let service = config.backend.name();
    let var = config.backend.key_var().unwrap_or_default();
    if !key_present {
        return Check::fail(
            "Speech model",
            format!("{} needs {}", service, var),
            format!(
                "put {} in ~/.config/hush/.env, or set backend = \"local\" under [transcription]",
                var
            ),
        );
    }
    Check::ok(
        "Speech model",
        format!(
            "{} {}; audio leaves this machine",
            service,
            config.cloud_model().unwrap_or_default()
        ),
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

fn clipboard_check(support: crate::text::clipboard::Support) -> Check {
    use crate::text::clipboard::Support;
    match support {
        Support::DataControl => Check::ok(
            "Clipboard",
            "a paste puts back every format (Wayland data-control)",
        ),
        Support::NoDataControl => Check::warn(
            "Clipboard",
            "a paste puts back only text: the compositor has no data-control \
             protocol (GNOME has none)",
            "images and rich text on the clipboard are lost when Hush pastes",
        ),
        Support::X11 => Check::ok("Clipboard", "a paste puts back the text (X11)"),
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

fn hook_check(config: &Config) -> Check {
    let command = config.hooks.transcript.trim();
    let home = dirs::home_dir();
    let program = hook::resolve_program(
        command,
        std::env::var_os("PATH").as_deref(),
        home.as_deref(),
    );
    hook_report(command, program)
}

fn hook_report(command: &str, program: Program) -> Check {
    const NAME: &str = "Transcript hook";
    if command.is_empty() {
        return Check::ok(NAME, "off");
    }
    let shown: String = if command.chars().count() > 60 {
        command.chars().take(59).chain(['…']).collect()
    } else {
        command.to_string()
    };
    let until = "until then, Hush types the original text";
    match program {
        Program::Absolute(_) => Check::ok(NAME, shown),
        Program::OnPath(path) => Check::warn(
            NAME,
            format!("{} ({} is on this PATH)", shown, path.display()),
            format!(
                "write {} in hooks.transcript: the daemon runs with the PATH of your login session, so an absolute path is safer",
                path.display()
            ),
        ),
        Program::NotExecutable(path) => Check::warn(
            NAME,
            format!("{} is not executable", path.display()),
            format!("chmod +x {}; {}", path.display(), until),
        ),
        Program::Missing(word) => Check::warn(
            NAME,
            format!("{} is not an absolute path or a name on PATH", word),
            format!("write the absolute path in hooks.transcript; {}", until),
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

fn updates_check(config: &Config) -> Check {
    use crate::updates::{InstallKind, UpdateState};
    updates_report(
        config.updates.check,
        InstallKind::current(),
        &UpdateState::load_from(&UpdateState::path()),
    )
}

fn updates_report(
    on: bool,
    kind: crate::updates::InstallKind,
    state: &crate::updates::UpdateState,
) -> Check {
    use crate::updates::CURRENT_VERSION;
    if !kind.checks() {
        return Check::ok("Updates", "your package manager handles updates");
    }
    if !on {
        return Check::ok("Updates", format!("check off ({})", kind.describe()));
    }
    if let Some((version, url)) = state.newer_than(CURRENT_VERSION) {
        return Check::warn(
            "Updates",
            format!("{} is available, you use {}", version, CURRENT_VERSION),
            format!("get it from {}", url),
        );
    }
    let when = |time: chrono::DateTime<chrono::Utc>| {
        time.with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M")
            .to_string()
    };
    match (state.last_checked, state.latest_version.is_some()) {
        (Some(checked), true) => Check::ok(
            "Updates",
            format!(
                "on, {} is the newest (checked {})",
                CURRENT_VERSION,
                when(checked)
            ),
        ),
        (Some(checked), false) => Check::ok(
            "Updates",
            format!("on, the last check got no answer ({})", when(checked)),
        ),
        (None, _) => Check::ok("Updates", "on, the daemon checks soon after it starts"),
    }
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
        let fix = check.fix.unwrap();
        assert!(
            fix.starts_with("hush models download "),
            "an unknown name falls back to the suggested model: {fix}"
        );
        assert!(
            fix.contains("definitely-not-a-model"),
            "and says what was wrong: {fix}"
        );

        config.transcription.model_size = "large-v3".to_string();
        config.transcription.model_path = Some("/nowhere/ggml-large-v3.bin".into());
        let check = model_check(&config);
        assert_eq!(check.level, Level::Fail);
        assert_eq!(
            check.fix.as_deref(),
            Some("hush models download large-v3"),
            "a known name is downloaded as asked"
        );
    }

    #[test]
    fn a_cloud_backend_is_checked_for_its_key() {
        let mut config = Config::defaults().unwrap();
        config.transcription.backend = crate::config::settings::TranscriptionBackend::Groq;
        let check = cloud_check(&config.transcription, false);
        assert_eq!(check.level, Level::Fail);
        assert!(check.detail.contains("GROQ_API_KEY"));
        assert!(check.fix.unwrap().contains("backend = \"local\""));

        let check = cloud_check(&config.transcription, true);
        assert_eq!(check.level, Level::Ok);
        assert!(check.detail.contains("whisper-large-v3-turbo"));
        assert!(check.detail.contains("audio leaves this machine"));
    }

    #[test]
    fn the_cpu_line_fails_only_below_the_floor() {
        assert_eq!(cpu_report(None, &[]).level, Level::Ok);
        assert_eq!(cpu_report(Some(cpu::X86_64_V3), &[]).level, Level::Ok);

        let check = cpu_report(Some(cpu::X86_64_V3), &["avx2", "bmi2"]);
        assert_eq!(check.level, Level::Fail);
        assert!(check.detail.contains("AVX2, BMI2"), "{}", check.detail);
        assert!(check.detail.contains("x86-64-v3"), "{}", check.detail);
        let fix = check.fix.unwrap();
        assert!(fix.contains("make release"), "{fix}");
        assert!(fix.contains("backend = \"groq\""), "{fix}");
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
    fn a_compositor_without_data_control_is_a_warning() {
        use crate::text::clipboard::Support;
        assert_eq!(clipboard_check(Support::DataControl).level, Level::Ok);
        assert_eq!(clipboard_check(Support::X11).level, Level::Ok);
        let gnome = clipboard_check(Support::NoDataControl);
        assert_eq!(gnome.level, Level::Warn);
        assert!(gnome.detail.contains("GNOME"));
    }

    #[test]
    fn the_hook_line_says_whether_the_command_resolves() {
        let off = hook_report("", Program::Missing(String::new()));
        assert_eq!(off.level, Level::Ok);
        assert_eq!(off.detail, "off");

        let absolute = hook_report(
            "/usr/bin/tr -d '!'",
            Program::Absolute("/usr/bin/tr".into()),
        );
        assert_eq!(absolute.level, Level::Ok);
        assert_eq!(absolute.detail, "/usr/bin/tr -d '!'");

        let on_path = hook_report("tr -d '!'", Program::OnPath("/usr/bin/tr".into()));
        assert_eq!(on_path.level, Level::Warn);
        assert!(on_path.detail.contains("/usr/bin/tr"), "{}", on_path.detail);
        let fix = on_path.fix.unwrap();
        assert!(fix.contains("write /usr/bin/tr"), "{fix}");
        assert!(fix.contains("PATH of your login session"), "{fix}");

        let missing = hook_report("fixit", Program::Missing("fixit".into()));
        assert_eq!(missing.level, Level::Warn);
        assert!(missing.detail.starts_with("fixit is not"));
        assert!(missing.fix.unwrap().contains("original text"));

        let plain = hook_report(
            "/home/me/fix",
            Program::NotExecutable("/home/me/fix".into()),
        );
        assert_eq!(plain.level, Level::Warn);
        assert!(plain.fix.unwrap().starts_with("chmod +x /home/me/fix"));

        let long = "x".repeat(80);
        let shown = hook_report(&long, Program::Absolute("/x".into())).detail;
        assert_eq!(shown.chars().count(), 60);
        assert!(shown.ends_with('…'));
    }

    #[test]
    fn a_switched_off_tray_is_not_a_problem() {
        let mut config = Config::defaults().unwrap();
        config.tray.enabled = false;
        assert_eq!(tray_check(&config).level, Level::Ok);
    }

    #[test]
    fn the_update_line_follows_the_install_and_the_last_check() {
        use crate::updates::{InstallKind, Release, UpdateState};
        let empty = UpdateState::default();
        let managed = updates_report(true, InstallKind::PackageManager, &empty);
        assert_eq!(managed.level, Level::Ok);
        assert!(managed.detail.contains("package manager"));
        assert!(updates_report(false, InstallKind::SelfManaged, &empty)
            .detail
            .starts_with("check off"));

        let mut state = UpdateState::default();
        state.record(
            chrono::Utc::now(),
            Some(Release {
                version: "999.0.0".to_string(),
                url: "https://github.com/andymai/hush/releases/tag/v999.0.0".to_string(),
            }),
        );
        let newer = updates_report(true, InstallKind::AppImage, &state);
        assert_eq!(newer.level, Level::Warn);
        assert!(newer.detail.starts_with("999.0.0 is available"));
        assert!(newer.fix.unwrap().ends_with("v999.0.0"));
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
