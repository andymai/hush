//! `hush doctor`: everything that has to be true for dictation to work, each
//! with the one command that fixes it.

use crate::audio::wake;
use crate::config::server::{self, Sends};
use crate::config::settings::{LlmSetting, TranscriptionBackend, TranscriptionConfig};
use crate::config::{keys, paths, Config};
use crate::hotkey::keymap::ModifierKeys;
use crate::hotkey::{warnings, HotkeyBindings};
use crate::text::layout::Layout;
use crate::text::typing::{NoLeds, Resolver, Typing};
use crate::text::WindowProvider;
use crate::text_processing::hook::{self, Program};
use crate::transcription::device::{self, GpuAvailability, Modules};
use crate::transcription::engine::local_model_path;
use crate::transcription::gpu_fallback::GpuFailures;
use crate::transcription::models::{
    recommended_model, ModelManager, ModelSize, PARAKEET_V3_LANGUAGES,
};
use crate::transcription::Family;
use crate::AudioCapture;
use anyhow::Result;
use std::fmt;
use std::path::Path;

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
                format!("{}", paths::config_path().display()),
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

    let models_dir = paths::models_dir();
    let env_file = keys::env_file();
    checks.push(permissions_check());
    if config.transcription.backend.is_cloud() {
        let key_present = config
            .transcription
            .key_var()
            .is_none_or(|var| keys::api_key_in(&env_file, var).is_some());
        checks.push(cloud_check(&config.transcription, key_present));
    } else {
        let backends = backends_report(device::init_backends(), &device::loaded_modules());
        let backends_loaded = backends.level == Level::Ok;
        checks.push(backends);
        checks.push(model_check(&config, &models_dir));
        let manager = ModelManager::new(&models_dir).ok();
        checks.push(vad_check(
            &config.transcription,
            manager
                .as_ref()
                .and_then(ModelManager::vad_model_path)
                .is_some(),
        ));
        let parakeet = is_parakeet(
            &config.transcription,
            Family::of_file(&local_model_path(&config.transcription, &models_dir)),
        );
        checks.extend(parakeet_language_check(&config.transcription, parakeet));
        let limits_detection = !config.transcription.detect_languages.is_empty() && !parakeet;
        checks.extend(langid_check(
            &config.transcription,
            parakeet,
            limits_detection
                && manager
                    .as_ref()
                    .and_then(ModelManager::langid_model_path)
                    .is_some(),
        ));
        // Without the modules, ggml lists no devices, so a GPU line would blame the driver.
        if backends_loaded {
            checks.push(gpu_check(&config));
        }
        let failures = GpuFailures::path();
        checks.extend(gpu_failure_report(
            &GpuFailures::load_from(&failures),
            &failures,
        ));
    }
    checks.push(microphone_check(&config));
    let reply = crate::ipc::client::status_reply().await;
    checks.push(wake_check(&config, reply.as_ref()).await);
    if config.audio.pause_media {
        checks.push(media_report(crate::audio::media::probe().await));
    }
    checks.push(hotkey_check(&config));
    checks.extend(hotkeys_checks(
        &config,
        crate::text::layout::current().as_ref(),
        &ModifierKeys::session(),
    ));
    checks.push(typing_check(&Resolver::new().resolve(&mut NoLeds)));
    checks.push(window_check());
    checks.push(clipboard_check(crate::text::clipboard::support()));
    checks.push(llm_check(&config, &env_file).await);
    checks.push(hook_check(&config));
    checks.push(tray_check(&config));
    checks.push(updates_check(&config));
    checks.push(autostart_check());
    checks.push(daemon_line(reply.as_ref()));
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

/// The ggml modules that loaded: the CPU code that ggml picked for this CPU,
/// and the GPU backend.
fn backends_report(loaded: anyhow::Result<()>, modules: &Modules) -> Check {
    const NAME: &str = "Backends";
    match loaded {
        Ok(()) => Check::ok(NAME, modules.to_string()),
        Err(_) => Check::fail(
            NAME,
            format!(
                "no ggml backend module loaded from {}",
                device::module_folder(modules.dir.as_deref())
            ),
            "reinstall Hush, or run `make release` again in the source tree",
        ),
    }
}

fn model_check(config: &Config, models_dir: &Path) -> Check {
    let size = config.transcription.model_size.clone();
    let path = config.transcription.model_path_in(models_dir);
    if path.exists() {
        let megabytes = std::fs::metadata(&path)
            .map(|meta| meta.len() / 1_000_000)
            .unwrap_or(0);
        return Check::ok("Speech model", format!("{} ({} MB)", size, megabytes));
    }
    if let Ok(manager) = ModelManager::new(models_dir) {
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

fn vad_check(config: &TranscriptionConfig, model_present: bool) -> Check {
    const NAME: &str = "Voice detection";
    if !config.vad {
        return Check::ok(NAME, "switched off; Whisper gets the whole take");
    }
    if model_present {
        return Check::ok(NAME, "Silero VAD trims silence before Whisper");
    }
    Check::warn(
        NAME,
        "the Silero VAD model is missing or damaged, so Whisper gets silence too",
        download_command(config),
    )
}

/// `hush models download` for the configured model, which also fetches the
/// VAD and language-ID models that are missing.
fn download_command(config: &TranscriptionConfig) -> String {
    let size = if config.model_size.parse::<ModelSize>().is_ok() {
        config.model_size.clone()
    } else {
        crate::gui::size_key(recommended_model()).to_string()
    };
    format!("hush models download {}", size)
}

/// Whether the model that loads is Parakeet: the `family` of its file, or
/// `model_size` when the file is missing or unreadable. A `model_path` set by
/// hand can name a file of another family than `model_size`.
fn is_parakeet(config: &TranscriptionConfig, family: Option<Family>) -> bool {
    match family {
        Some(family) => family == Family::Parakeet,
        None => config
            .model_size
            .parse::<ModelSize>()
            .is_ok_and(|size| size.is_parakeet()),
    }
}

/// A line only when Parakeet is the model and `language` names a language
/// that Parakeet does not know: Parakeet ignores the setting and picks among
/// its own languages.
fn parakeet_language_check(config: &TranscriptionConfig, parakeet: bool) -> Option<Check> {
    let language = config.language.trim();
    if !parakeet || language == "auto" || PARAKEET_V3_LANGUAGES.contains(&language) {
        return None;
    }
    Some(Check::warn(
        "Language",
        format!(
            "Parakeet does not know {}; it picks among its 25 European languages",
            language
        ),
        "choose a Whisper model for this language: hush models set base",
    ))
}

/// A line only when `detect_languages` limits detection, which needs the
/// language-ID model.
fn langid_check(
    config: &TranscriptionConfig,
    parakeet: bool,
    model_present: bool,
) -> Option<Check> {
    const NAME: &str = "Language detection";
    if config.detect_languages.is_empty() {
        return None;
    }
    if parakeet {
        return Some(Check::ok(
            NAME,
            "Parakeet tells its own languages apart, so detect_languages is not used",
        ));
    }
    let languages = config.detect_languages.join(", ");
    if model_present {
        return Some(Check::ok(
            NAME,
            format!("VoxLingua107 picks among {}", languages),
        ));
    }
    Some(Check::warn(
        NAME,
        format!(
            "the language-ID model is missing or damaged, so Whisper picks among {} on its own",
            languages
        ),
        download_command(config),
    ))
}

fn cloud_check(config: &TranscriptionConfig, key_present: bool) -> Check {
    if config.backend == TranscriptionBackend::OpenAiCompatible {
        return server_check(config, key_present);
    }
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

fn server_check(config: &TranscriptionConfig, key_present: bool) -> Check {
    if let Err(e) = config.validate_server() {
        return Check::fail(
            "Speech model",
            e.to_string(),
            "set server_url and server_model under [transcription]",
        );
    }
    if let (Some(var), false) = (config.key_var(), key_present) {
        return Check::fail(
            "Speech model",
            format!("the server needs {}", var),
            format!(
                "put {} in ~/.config/hush/.env, or set server_key_env = \"\" under [transcription]",
                var
            ),
        );
    }
    Check::ok("Speech model", config.describe_server())
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

/// A line for each model family that failed a take on a GPU and that Hush
/// keeps on the CPU.
fn gpu_failure_report(records: &GpuFailures, path: &Path) -> Vec<Check> {
    records
        .iter()
        .map(|record| {
            Check::warn(
                "GPU failure",
                format!(
                    "{} via {} failed a {} take with driver {}, so {} uses the CPU until the \
                     driver changes",
                    record.gpu, record.backend, record.family, record.driver, record.family
                ),
                format!(
                    "rm {} && hush daemon restart (tries the GPU again)",
                    path.display()
                ),
            )
        })
        .collect()
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

/// Whether WirePlumber suspends the microphone between takes, with the start
/// delay that the daemon measured.
async fn wake_check(config: &Config, reply: Option<&crate::ipc::DaemonResponse>) -> Check {
    if wake::sandboxed() {
        return sandbox_wake_report();
    }
    let probe = wake::look(config.audio.device.as_deref()).await;
    let hush_rule = match (&probe, wake::config_dir()) {
        (wake::Probe::Found(source), Ok(dir)) => wake::rule_names(&dir, source),
        _ => false,
    };
    wake_report(wake::finding(
        &probe,
        hush_rule,
        reply.and_then(|reply| reply.start_delay),
    ))
}

/// Inside Flatpak, one sentence and no fix: `hush setup mic` cannot work there.
fn sandbox_wake_report() -> Check {
    Check::ok("Microphone wake", wake::SANDBOX_NOTE)
}

fn wake_report(finding: wake::Finding) -> Check {
    const NAME: &str = "Microphone wake";
    match finding.fix {
        Some(fix) if finding.warn => Check::warn(NAME, finding.detail, fix),
        _ => Check::ok(NAME, finding.detail),
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

/// Chords that two actions share, a key that types AltGr, and keys that type
/// a character, language chords included: one line for each kind of
/// problem.
fn hotkeys_checks(config: &Config, layout: Option<&Layout>, keys: &ModifierKeys) -> Vec<Check> {
    const NAME: &str = "Hotkeys";
    let chords = config.hotkey.bound();
    let mut checks = Vec::new();

    let conflicts = warnings::conflicts(chords.clone());
    if !conflicts.is_empty() {
        let each: Vec<String> = conflicts.iter().map(ToString::to_string).collect();
        checks.push(Check::fail(
            NAME,
            each.join("; "),
            "give each action its own chord in hush settings, or under [hotkey]",
        ));
    }
    if let Some(altgr) = warnings::altgr(chords.clone(), layout, keys) {
        checks.push(Check::warn(
            NAME,
            altgr.to_string(),
            format!("{} in hush settings, or under [hotkey]", altgr.advice()),
        ));
    }
    let printable = warnings::printable(chords, config.hotkey.hides_keys());
    if !printable.is_empty() {
        let each: Vec<String> = printable
            .iter()
            .map(|(role, chord)| format!("{} ({})", chord, role.label()))
            .collect();
        let verb = if each.len() == 1 { "reaches" } else { "reach" };
        checks.push(Check::warn(
            NAME,
            format!("{} also {} the focused app", warnings::join(&each), verb),
            "set exclusive = true under [hotkey], or pick a key that types no character",
        ));
    }
    if checks.is_empty() {
        checks.push(Check::ok(NAME, "each action has its own chord"));
    }
    checks
}

fn typing_check(typing: &Typing) -> Check {
    match typing {
        Typing::Paste { fix, .. } => Check::warn("Typing", typing.to_string(), *fix),
        _ => Check::ok("Typing", typing.to_string()),
    }
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

async fn llm_check(config: &Config, env_file: &Path) -> Check {
    use crate::text_processing::LlmProvider;
    let provider = crate::text_processing::llm::resolve_provider(&config.llm, env_file).await;
    match provider {
        LlmProvider::Ollama { model, .. } => Check::ok("Polishing", format!("Ollama {}", model)),
        LlmProvider::Anthropic { model, .. } => {
            Check::ok("Polishing", format!("Anthropic {}", model))
        },
        LlmProvider::OpenAiCompatible { url, model, .. } => Check::ok(
            "Polishing",
            format!(
                "OpenAI-compatible {}",
                server::describe(&model, &url, Sends::Text)
            ),
        ),
        LlmProvider::None if config.llm.provider == LlmSetting::OpenAiCompatible => {
            if let Err(e) = config.llm.validate_server() {
                return Check::warn(
                    "Polishing",
                    e.to_string(),
                    "set server_url and server_model under [llm]",
                );
            }
            let var = config.llm.server_key_var().unwrap_or_default();
            Check::warn(
                "Polishing",
                format!("the server needs {}; Command Mode is off", var),
                format!(
                    "put {} in ~/.config/hush/.env, or set server_key_env = \"\" under [llm]",
                    var
                ),
            )
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

fn media_report(players: Result<usize>) -> Check {
    const NAME: &str = "Media pause";
    match players {
        Ok(1) => Check::ok(NAME, "on, 1 player on the session bus now"),
        Ok(count) => Check::ok(
            NAME,
            format!("on, {} players on the session bus now", count),
        ),
        Err(e) => Check::warn(
            NAME,
            format!("the session bus does not answer ({:#})", e),
            "run Hush inside your desktop session, or set audio.pause_media = false",
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

/// The doctor line for the daemon's status reply, `None` when none runs.
fn daemon_line(reply: Option<&crate::ipc::DaemonResponse>) -> Check {
    let state = reply.and_then(|reply| reply.state);
    match (state, reply.and_then(|reply| reply.safe_mode.as_deref())) {
        (Some(state), Some(reason)) => Check::warn(
            "Hush",
            format!("running in safe mode, {}", state),
            super::status::safe_mode_line(reason),
        ),
        (Some(state), None) => Check::ok("Hush", format!("running, {}", state)),
        (None, _) => Check::warn("Hush", "not running", "hush daemon start"),
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
    fn a_daemon_in_safe_mode_is_a_warning_with_its_reason() {
        use crate::ipc::{DaemonResponse, DaemonState};
        let normal = DaemonResponse::ok(DaemonState::Idle);
        assert_eq!(daemon_line(Some(&normal)).level, Level::Ok);
        let safe = DaemonResponse::ok(DaemonState::Idle)
            .with_safe_mode(Some("the configuration does not load: bad toml".into()));
        let line = daemon_line(Some(&safe));
        assert_eq!(line.level, Level::Warn);
        assert!(line.detail.contains("safe mode"), "{}", line.detail);
        let fix = line.fix.unwrap();
        assert!(
            fix.contains("bad toml") && fix.contains("hush daemon restart"),
            "{fix}"
        );
        assert_eq!(daemon_line(None).level, Level::Warn);
    }

    #[test]
    fn the_typing_line_says_types_or_pastes_and_why() {
        use crate::text::keymap::KeyTable;
        let keys = Typing::Keys {
            table: std::sync::Arc::new(KeyTable::us().clone()),
            layout: "de".into(),
        };
        let check = typing_check(&keys);
        assert_eq!(check.level, Level::Ok);
        assert_eq!(check.detail, "types key by key on de");

        let paste = Typing::Paste {
            why: "the session has 2 layouts (us,de), and Hush cannot tell which one is active"
                .into(),
            fix: "keep one layout",
        };
        let check = typing_check(&paste);
        assert_eq!(check.level, Level::Warn);
        assert!(check
            .detail
            .starts_with("pastes every take: the session has 2 layouts"));
        assert_eq!(check.fix.as_deref(), Some("keep one layout"));
    }

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
        let models = tempfile::tempdir().unwrap();
        let mut config = Config::defaults().unwrap();
        config.transcription.model_size = "definitely-not-a-model".to_string();
        config.transcription.model_path = Some("/nowhere/ggml-definitely-not-a-model.bin".into());
        let check = model_check(&config, models.path());
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
        let check = model_check(&config, models.path());
        assert_eq!(check.level, Level::Fail);
        assert_eq!(
            check.fix.as_deref(),
            Some("hush models download large-v3"),
            "a known name is downloaded as asked"
        );
    }

    #[test]
    fn the_vad_line_names_the_download_when_the_model_is_missing() {
        let mut config = Config::defaults().unwrap().transcription;
        config.model_size = "small".to_string();
        let check = vad_check(&config, false);
        assert_eq!(check.level, Level::Warn);
        assert_eq!(check.name, "Voice detection");
        assert_eq!(check.fix.as_deref(), Some("hush models download small"));

        assert_eq!(vad_check(&config, true).level, Level::Ok);
        config.vad = false;
        let off = vad_check(&config, false);
        assert_eq!(off.level, Level::Ok);
        assert!(off.detail.contains("switched off"));
    }

    #[test]
    fn the_language_line_shows_only_when_detection_is_limited() {
        let mut config = Config::defaults().unwrap().transcription;
        assert!(langid_check(&config, false, false).is_none());
        config.detect_languages = vec!["en".to_string(), "de".to_string()];
        let missing = langid_check(&config, false, false).unwrap();
        assert_eq!(missing.level, Level::Warn);
        assert_eq!(missing.fix.as_deref(), Some("hush models download base"));
        assert!(missing.detail.contains("en, de"), "{}", missing.detail);
        let present = langid_check(&config, false, true).unwrap();
        assert_eq!(present.level, Level::Ok);
        assert!(present.detail.contains("en, de"), "{}", present.detail);
    }

    #[test]
    fn parakeet_warns_about_a_language_it_lacks_and_skips_language_id() {
        let mut config = Config::defaults().unwrap().transcription;
        config.language = "de".to_string();
        assert!(
            parakeet_language_check(&config, true).is_none(),
            "German is a Parakeet language"
        );
        config.language = "auto".to_string();
        assert!(parakeet_language_check(&config, true).is_none());
        config.language = "ja".to_string();
        let check = parakeet_language_check(&config, true).unwrap();
        assert_eq!(check.level, Level::Warn);
        assert!(
            check.detail.contains("does not know ja"),
            "{}",
            check.detail
        );
        assert!(
            parakeet_language_check(&config, false).is_none(),
            "Whisper knows Japanese"
        );

        config.detect_languages = vec!["en".to_string(), "de".to_string()];
        let line = langid_check(&config, true, false).unwrap();
        assert_eq!(line.level, Level::Ok, "no language-ID model is needed");
        assert!(line.detail.contains("not used"), "{}", line.detail);
    }

    #[test]
    fn the_model_file_decides_parakeet_over_the_model_name() {
        let mut config = Config::defaults().unwrap().transcription;
        assert!(!is_parakeet(&config, None));
        assert!(
            is_parakeet(&config, Some(Family::Parakeet)),
            "a Parakeet file set by hand under the name base"
        );
        config.model_size = "parakeet-v3".to_string();
        assert!(is_parakeet(&config, None), "the file is missing");
        assert!(
            !is_parakeet(&config, Some(Family::Whisper)),
            "a Whisper file set by hand under the name parakeet-v3"
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
    fn a_server_backend_says_local_server_or_names_the_receiver() {
        let mut config = Config::defaults().unwrap().transcription;
        config.backend = TranscriptionBackend::OpenAiCompatible;
        config.server_url = "http://192.168.1.5:8080/v1".into();
        config.server_model = "whisper-large-v3".into();
        let check = cloud_check(&config, true);
        assert_eq!(check.level, Level::Ok);
        assert_eq!(
            check.detail,
            "whisper-large-v3 on the local server 192.168.1.5:8080"
        );

        config.server_url = "https://stt.example.com".into();
        let check = cloud_check(&config, true);
        assert_eq!(check.level, Level::Ok);
        assert!(
            check.detail.ends_with("a service that receives the audio"),
            "{}",
            check.detail
        );

        config.server_key_env = "STT_KEY".into();
        let check = cloud_check(&config, false);
        assert_eq!(check.level, Level::Fail);
        assert!(check.detail.contains("STT_KEY"));
        assert!(check.fix.unwrap().contains("server_key_env = \"\""));

        config.server_url.clear();
        let check = cloud_check(&config, true);
        assert_eq!(check.level, Level::Fail);
        assert!(check.detail.contains("transcription.server_url"));
    }

    #[tokio::test]
    async fn a_server_llm_without_its_key_names_the_variable() {
        let dir = tempfile::tempdir().unwrap();
        let env_file = dir.path().join(".env");
        let mut config = Config::defaults().unwrap();
        config.llm.provider = LlmSetting::OpenAiCompatible;
        config.llm.server_url = "http://localhost:8000".into();
        config.llm.server_model = "qwen3".into();
        let check = llm_check(&config, &env_file).await;
        assert_eq!(check.level, Level::Ok);
        assert_eq!(
            check.detail,
            "OpenAI-compatible qwen3 on the local server localhost:8000"
        );

        config.llm.server_key_env = "HUSH_TEST_UNSET_LLM_KEY".into();
        let check = llm_check(&config, &env_file).await;
        assert_eq!(check.level, Level::Warn);
        assert!(check.detail.contains("HUSH_TEST_UNSET_LLM_KEY"));
        assert!(check.fix.unwrap().contains("under [llm]"));
    }

    #[test]
    fn the_backends_line_names_the_modules_or_fails_without_them() {
        let modules = Modules {
            dir: Some("/usr/lib/hush".into()),
            cpu: Some("haswell".to_string()),
            gpu: vec!["Vulkan".to_string()],
        };
        let check = backends_report(Ok(()), &modules);
        assert_eq!(check.level, Level::Ok);
        assert_eq!(
            check.detail,
            "CPU code haswell, GPU backend Vulkan (/usr/lib/hush)"
        );

        let missing = Modules {
            dir: Some("/usr/lib/hush".into()),
            ..Modules::default()
        };
        let check = backends_report(Err(anyhow::anyhow!("status 8")), &missing);
        assert_eq!(check.level, Level::Fail);
        assert_eq!(
            check.detail,
            "no ggml backend module loaded from /usr/lib/hush"
        );
        assert!(check.fix.unwrap().contains("reinstall Hush"));
    }

    #[test]
    fn a_remembered_gpu_failure_names_the_file_to_delete() {
        let path = Path::new("/home/me/.local/state/hush/gpu-failure.json");
        let mut records = GpuFailures::default();
        assert!(gpu_failure_report(&records, path).is_empty());

        let record = crate::transcription::gpu_fallback::GpuFailure {
            gpu: "NVIDIA GeForce RTX 4080".to_string(),
            backend: "Vulkan".to_string(),
            driver: "NVIDIA 580.95.05".to_string(),
            family: "Parakeet".to_string(),
        };
        records.insert(record.clone());
        records.insert(crate::transcription::gpu_fallback::GpuFailure {
            family: "Whisper".to_string(),
            ..record
        });
        let lines = gpu_failure_report(&records, path);
        assert_eq!(lines.len(), 2, "a line for each family");
        let check = lines
            .into_iter()
            .find(|line| line.detail.contains("Parakeet"))
            .unwrap();
        assert_eq!(check.level, Level::Warn);
        assert!(
            check.detail.starts_with(
                "NVIDIA GeForce RTX 4080 via Vulkan failed a Parakeet take with driver \
                 NVIDIA 580.95.05"
            ),
            "{}",
            check.detail
        );
        assert!(
            check.detail.contains("so Parakeet uses the CPU"),
            "{}",
            check.detail
        );
        assert_eq!(
            check.fix.as_deref(),
            Some(
                "rm /home/me/.local/state/hush/gpu-failure.json && hush daemon restart (tries the GPU again)"
            )
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
    fn the_hotkeys_line_names_conflicts_altgr_and_typing_keys() {
        let physical = ModifierKeys::physical();
        let us = Layout::Xkb {
            layout: "us".into(),
            variant: String::new(),
        };
        let german = Layout::Xkb {
            layout: "de".into(),
            variant: String::new(),
        };
        let mut config = Config::defaults().unwrap();
        let clean = hotkeys_checks(&config, Some(&us), &physical);
        assert_eq!(clean.len(), 1);
        assert_eq!(clean[0].level, Level::Ok);
        assert_eq!(clean[0].name, "Hotkeys");

        let altgr = hotkeys_checks(&config, Some(&german), &physical);
        assert_eq!(altgr.len(), 1);
        assert_eq!(altgr[0].level, Level::Warn);
        assert!(altgr[0].detail.contains("AltGr"), "{}", altgr[0].detail);
        assert!(altgr[0].fix.as_ref().unwrap().contains("RightCtrl"));
        let moved = ModifierKeys::from_classes(&crate::text::xkb::KeyClasses {
            control: vec![input_linux::Key::LeftCtrl, input_linux::Key::RightMeta],
            super_keys: vec![input_linux::Key::RightCtrl, input_linux::Key::LeftMeta],
            altgr: vec![input_linux::Key::RightAlt],
            ..Default::default()
        });
        let altgr = hotkeys_checks(&config, Some(&german), &moved);
        assert_eq!(
            altgr[0].fix.as_deref(),
            Some(
                "use RightSuper, which types Ctrl on this layout, in place of RightAlt in hush \
                 settings, or under [hotkey]"
            ),
            "Right Ctrl types Super with ctrl:swap_rwin_rctl"
        );

        config.hotkey.combination = "F12".into();
        config.hotkey.paste_last = "f12".into();
        config.hotkey.learn = "ctrl+rightalt".into();
        config.hotkey.cancel = "Space".into();
        let checks = hotkeys_checks(&config, Some(&us), &physical);
        assert_eq!(checks.len(), 2);
        assert_eq!(checks[0].level, Level::Fail);
        assert_eq!(
            checks[0].detail,
            "Dictate and Type last again use F12; Command and Learn selection use Ctrl+RightAlt"
        );
        assert!(checks[0].fix.as_ref().unwrap().contains("own chord"));
        assert_eq!(checks[1].level, Level::Warn);
        assert_eq!(
            checks[1].detail,
            "Space (Cancel) also reaches the focused app"
        );
        assert!(checks[1].fix.as_ref().unwrap().contains("exclusive = true"));

        let mut config = Config::defaults().unwrap();
        config
            .hotkey
            .languages
            .insert("ctrl+rightalt".into(), "de".into());
        config
            .hotkey
            .languages
            .insert("Shift+Q".into(), "fr".into());
        let checks = hotkeys_checks(&config, Some(&us), &physical);
        assert_eq!(checks.len(), 2);
        assert_eq!(
            checks[0].detail,
            "Command and Dictate in German use Ctrl+RightAlt"
        );
        assert_eq!(
            checks[1].detail,
            "Shift+Q (Dictate in French) also reaches the focused app"
        );

        config.hotkey.exclusive = true;
        assert_eq!(hotkeys_checks(&config, Some(&us), &physical).len(), 1);
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
    fn inside_flatpak_the_wake_line_has_no_fix() {
        let check = sandbox_wake_report();
        assert_eq!(check.level, Level::Ok);
        assert!(check.fix.is_none());
        assert!(check.detail.contains("inside Flatpak"), "{}", check.detail);
        assert!(check.detail.contains("does not apply"), "{}", check.detail);
    }

    #[test]
    fn the_wake_line_warns_with_the_setup_command() {
        use crate::audio::take::StartDelay;
        let source = wake::Source {
            name: "alsa_input.usb-Blue_Microphones_Yeti_Stereo_Microphone_REV8-00.iec958-stereo"
                .to_string(),
            label: "Yeti Stereo Microphone".to_string(),
            profile: Some("iec958-stereo".to_string()),
            serial: Some(1),
            kind: wake::Kind::Alsa,
            sleep: wake::Sleep::After(5),
            split: None,
            twin: false,
        };
        let slow = StartDelay {
            last_ms: 30,
            worst_ms: 650,
            takes: 4,
        };
        let line = wake_report(wake::finding(
            &wake::Probe::Found(source.clone()),
            false,
            Some(slow),
        ));
        assert_eq!(line.level, Level::Warn);
        assert_eq!(line.name, "Microphone wake");
        assert_eq!(line.fix.as_deref(), Some("hush setup mic"));
        assert!(line.detail.contains("650 ms"), "{}", line.detail);

        let awake = wake::Source {
            sleep: wake::Sleep::Never,
            ..source
        };
        let line = wake_report(wake::finding(&wake::Probe::Found(awake), true, Some(slow)));
        assert_eq!(line.level, Level::Ok);
        assert!(line.fix.is_none());

        let skipped = wake_report(wake::finding(&wake::Probe::NoPipeWire, false, None));
        assert_eq!(skipped.level, Level::Ok);
        assert!(skipped.detail.contains("PulseAudio"), "{}", skipped.detail);
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

    #[test]
    fn the_media_line_says_whether_the_session_bus_answers() {
        let on = media_report(Ok(2));
        assert_eq!(on.level, Level::Ok);
        assert_eq!(on.detail, "on, 2 players on the session bus now");
        assert_eq!(
            media_report(Ok(1)).detail,
            "on, 1 player on the session bus now"
        );

        let off = media_report(Err(anyhow::anyhow!("no answer in 500 ms")));
        assert_eq!(off.level, Level::Warn);
        assert!(off.detail.contains("no answer in 500 ms"));
        assert!(off.fix.unwrap().contains("audio.pause_media = false"));
    }

    #[tokio::test]
    async fn the_checks_run_without_panicking() {
        crate::test_support::in_temp_home!(the_checks_run_without_panicking);
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
