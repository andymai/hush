//! `hush report`: one text file for a bug report. It holds the version, the
//! doctor's checks, `hush status --full`, the settings without prompts, hook
//! commands, or server addresses, the recent log lines at INFO and above, and
//! the last crash. Home paths, the host name, the user name, and API keys are
//! replaced. Hush writes the file and uploads nothing.

use crate::config::{paths, transfer, Config};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The log lines at the end of the report.
const LOG_LINES: usize = 500;
/// The lines of the last crash's summary.
const CRASH_LINES: usize = 60;

pub async fn handle_report(output: Option<PathBuf>) -> Result<()> {
    let output = output.unwrap_or_else(|| {
        PathBuf::from(format!(
            "hush-report-{}.txt",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        ))
    });
    let config = Config::load().ok();
    let secrets = Secrets::current(config.as_ref());
    let mut report = String::new();
    section(&mut report, "Hush", &about());
    section(&mut report, "hush doctor", &run_hush(&["doctor"]));
    section(
        &mut report,
        "hush status --full",
        &run_hush(&["status", "--full"]),
    );
    section(&mut report, "Settings", &settings(&paths::config_path()));
    section(
        &mut report,
        "Log",
        &recent_log(&paths::data_dir().join("logs")),
    );
    section(&mut report, "Last crash", &last_crash());
    std::fs::write(&output, secrets.redact(&report))
        .with_context(|| format!("Failed to write {}", output.display()))?;
    println!("Wrote {}.", output.display());
    println!(
        "Read it before you attach it to an issue: Hush removed personal details, not every name."
    );
    Ok(())
}

fn section(report: &mut String, title: &str, body: &str) {
    report.push_str(&format!("## {}\n\n{}\n\n", title, body.trim_end()));
}

fn about() -> String {
    let os = std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|text| {
            text.lines()
                .find_map(|line| line.strip_prefix("PRETTY_NAME="))
                .map(|name| name.trim_matches('"').to_string())
        })
        .unwrap_or_else(|| std::env::consts::OS.to_string());
    let var = |name: &str| std::env::var(name).unwrap_or_else(|_| "-".to_string());
    format!(
        "Version: {}\nInstall: {:?}\nSystem: {} ({})\nDesktop: {}, session {}",
        env!("CARGO_PKG_VERSION"),
        crate::updates::InstallKind::current(),
        os,
        std::env::consts::ARCH,
        var("XDG_CURRENT_DESKTOP"),
        var("XDG_SESSION_TYPE"),
    )
}

/// The output of this binary with `args`, so the report holds what the user
/// would see.
fn run_hush(args: &[&str]) -> String {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => return format!("(could not find the hush binary: {})", e),
    };
    let mut command = Command::new(exe);
    match crate::config::keys::without_env_files(&mut command)
        .arg("--no-notifications")
        .args(args)
        .output()
    {
        Ok(output) => format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
        Err(e) => format!("(could not run hush {}: {})", args.join(" "), e),
    }
}

/// The settings file without the text that a user writes into it: style
/// prompts, hook commands, and the server addresses, which can carry a token.
fn settings(path: &Path) -> String {
    let Ok(text) = std::fs::read_to_string(path) else {
        return "(no config.toml: Hush runs on its defaults)".to_string();
    };
    let Ok(mut table) = text.parse::<toml::Table>() else {
        return "(config.toml does not parse, so the report leaves it out)".to_string();
    };
    strip_settings(&mut table);
    toml::to_string(&table).unwrap_or_default()
}

fn strip_settings(table: &mut toml::Table) {
    const REMOVED: &str = "<removed>";
    if let Some(toml::Value::Table(styles)) = table.get_mut("styles") {
        for (_, style) in styles.iter_mut() {
            if let toml::Value::Table(style) = style {
                if style.contains_key("prompt") {
                    style.insert("prompt".into(), REMOVED.into());
                }
            }
        }
    }
    if let Some(toml::Value::Table(hooks)) = table.get_mut("hooks") {
        for (_, value) in hooks.iter_mut() {
            if value.as_str().is_some_and(|command| !command.is_empty()) {
                *value = REMOVED.into();
            }
        }
    }
    for section in ["transcription", "llm"] {
        if let Some(toml::Value::Table(section)) = table.get_mut(section) {
            for key in ["server_url", "ollama_url"] {
                if let Some(toml::Value::String(url)) = section.get_mut(key) {
                    *url = origin(url);
                }
            }
        }
    }
}

/// `https://host:port` from a URL, without the user, the path, or the query.
fn origin(url: &str) -> String {
    let (scheme, rest) = url.split_once("://").unwrap_or(("", url));
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = host.rsplit('@').next().unwrap_or(host);
    if scheme.is_empty() {
        host.to_string()
    } else {
        format!("{}://{}", scheme, host)
    }
}

/// The last lines at INFO, WARN, and ERROR of the two newest log files.
/// Hush logs dictated text only at DEBUG and below, so these lines hold none.
fn recent_log(dir: &Path) -> String {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|path| path.extension().is_some_and(|ext| ext == "log"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    let text: String = files
        .iter()
        .rev()
        .take(2)
        .rev()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .collect::<Vec<_>>()
        .join("\n");
    let lines = levelled_lines(&text);
    if lines.is_empty() {
        return "(no log lines)".to_string();
    }
    lines[lines.len().saturating_sub(LOG_LINES)..].join("\n")
}

/// Lines such as `2026-11-26 10:44:20.047  WARN ...` at INFO and above.
fn levelled_lines(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| {
            matches!(
                line.split_whitespace().nth(2),
                Some("INFO" | "WARN" | "ERROR")
            )
        })
        .collect()
}

/// The summary of the last crash of hush. A sandbox cannot read the
/// journal, and Hush asks for no permission to.
fn last_crash() -> String {
    #[cfg(target_os = "linux")]
    if crate::portal::sandboxed() {
        return "(not available in the Flatpak)".to_string();
    }
    match Command::new("coredumpctl")
        .args(["--no-pager", "-1", "info", "hush"])
        .output()
    {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .lines()
            .take(CRASH_LINES)
            .collect::<Vec<_>>()
            .join("\n"),
        _ => "(none recorded)".to_string(),
    }
}

/// What the report must not show.
struct Secrets {
    keys: Vec<String>,
    homes: Vec<String>,
    host: Option<String>,
    user: Option<String>,
}

impl Secrets {
    fn current(config: Option<&Config>) -> Self {
        let user = std::env::var("USER").ok().or_else(|| {
            dirs::home_dir().and_then(|home| Some(home.file_name()?.to_string_lossy().into_owned()))
        });
        let mut homes: Vec<String> = dirs::home_dir()
            .map(|home| home.display().to_string())
            .into_iter()
            .collect();
        if let Some(user) = &user {
            homes.push(format!("/var/home/{}", user));
            homes.push(format!("/home/{}", user));
        }
        Self {
            keys: config
                .map(transfer::api_keys)
                .unwrap_or_default()
                .into_iter()
                .map(|(_, key)| key)
                .collect(),
            homes,
            host: std::fs::read_to_string("/proc/sys/kernel/hostname")
                .ok()
                .map(|host| host.trim().to_string()),
            user,
        }
    }

    fn redact(&self, text: &str) -> String {
        let mut text = text.to_string();
        for key in &self.keys {
            let key = key.trim();
            if key.len() >= transfer::MIN_KEY_LEN {
                text = text.replace(key, "<api key>");
            }
        }
        // The longest first: /var/home/me holds /home/me.
        let mut homes = self.homes.clone();
        homes.sort_by_key(|home| std::cmp::Reverse(home.len()));
        homes.dedup();
        for home in homes.iter().filter(|home| home.len() > 1) {
            text = text.replace(home.as_str(), "~");
        }
        if let Some(host) = self.host.as_deref().filter(|host| host.len() >= 2) {
            text = replace_word(&text, host, "<host>");
        }
        if let Some(user) = self.user.as_deref().filter(|user| user.len() >= 3) {
            text = replace_word(&text, user, "<user>");
        }
        text
    }
}

/// Replace `word` where no letter, digit, or underscore touches it.
fn replace_word(text: &str, word: &str, with: &str) -> String {
    let touches = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    let mut out = String::with_capacity(text.len());
    let mut kept = 0;
    for (at, _) in text.match_indices(word) {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        if !touches(before) && !touches(after) {
            out.push_str(&text[kept..at]);
            out.push_str(with);
            kept = at + word.len();
        }
    }
    out.push_str(&text[kept..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secrets() -> Secrets {
        Secrets {
            keys: vec!["gsk_0123456789abcdef".into(), "short".into()],
            homes: vec!["/var/home/andy".into(), "/home/andy".into()],
            host: Some("aurora".into()),
            user: Some("andy".into()),
        }
    }

    #[test]
    fn personal_details_and_keys_leave_the_report() {
        let text = "log_dir=/var/home/andy/.local/share/hush/logs\n\
                    model=/home/andy/models/base.bin\n\
                    Hostname: aurora\n\
                    Owner UID: 1000 (andy)\n\
                    key=gsk_0123456789abcdef short\n\
                    sandy and aurorae stay\n";
        let redacted = secrets().redact(text);
        assert_eq!(
            redacted,
            "log_dir=~/.local/share/hush/logs\n\
             model=~/models/base.bin\n\
             Hostname: <host>\n\
             Owner UID: 1000 (<user>)\n\
             key=<api key> short\n\
             sandy and aurorae stay\n"
        );
    }

    #[test]
    fn settings_lose_prompts_hooks_and_url_secrets() {
        let mut table: toml::Table = r#"
            [transcription]
            model_size = "base"
            server_url = "https://user:pass@asr.example:8443/v1/audio?token=x"
            [llm]
            ollama_url = "http://localhost:11434/api"
            [hooks]
            transcript = "/usr/bin/tr -d '!'"
            [styles.casual]
            prompt = "Write like my friend Ada"
            polish = true
        "#
        .parse()
        .unwrap();
        strip_settings(&mut table);
        let text = toml::to_string(&table).unwrap();
        assert!(text.contains("model_size = \"base\""));
        assert!(
            text.contains("server_url = \"https://asr.example:8443\""),
            "{text}"
        );
        assert!(
            text.contains("ollama_url = \"http://localhost:11434\""),
            "{text}"
        );
        assert!(!text.contains("tr -d") && !text.contains("Ada") && !text.contains("pass"));
        assert!(text.contains("polish = true"));
    }

    #[test]
    fn only_info_and_above_lines_stay() {
        let text = "2026-11-26 10:44:20.047  WARN ThreadId(01) a: no GPU\n\
                    2026-11-26 10:44:20.048 DEBUG ThreadId(01) b: transcript \"secret words\"\n\
                    2026-11-26 10:44:20.049  INFO ThreadId(01) c: ready\n\
                    a continuation line\n";
        assert_eq!(
            levelled_lines(text),
            [
                "2026-11-26 10:44:20.047  WARN ThreadId(01) a: no GPU",
                "2026-11-26 10:44:20.049  INFO ThreadId(01) c: ready",
            ]
        );
    }

    #[test]
    fn an_origin_keeps_the_scheme_host_and_port() {
        assert_eq!(origin("https://u:p@h.example:9/x?y"), "https://h.example:9");
        assert_eq!(origin("localhost:11434/api"), "localhost:11434");
    }
}
