//! The transcript hook: a user command that runs after polishing and before
//! insertion. It reads the transcript on stdin, and Hush types what it prints.

use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::{ChildStdin, Command};
use tracing::{info, warn};

pub const TIMEOUT: Duration = Duration::from_secs(5);

/// The exit code that tells Hush to type nothing.
pub const SKIP_EXIT_CODE: i32 = 77;

const MAX_OUTPUT_BYTES: usize = 1 << 20;
const MAX_STDERR_BYTES: u64 = 64 << 10;
const STDERR_LINE_CHARS: usize = 200;

/// What started a take. The hook gets it as `HUSH_TRIGGER`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// The dictation hotkey
    Hotkey,
    /// The control socket: `hush toggle`, `hush start`, or a keybind that runs them
    Ipc,
    /// The panel icon
    Tray,
    /// The Command Mode chord
    Command,
    /// A rewrite key or `hush rewrite`: a style rewrite of the selection
    Rewrite,
}

impl Trigger {
    pub fn as_str(self) -> &'static str {
        match self {
            Trigger::Hotkey => "hotkey",
            Trigger::Ipc => "ipc",
            Trigger::Tray => "tray",
            Trigger::Command => "command",
            Trigger::Rewrite => "rewrite",
        }
    }
}

/// The environment variables the command gets.
#[derive(Debug, Clone, Copy)]
pub struct HookEnv<'a> {
    /// `HUSH_APP`: the window class, empty when unknown
    pub app: &'a str,
    /// `HUSH_PROFILE`: the tone profile name
    pub profile: &'a str,
    /// `HUSH_RAW_TEXT`: the text as Whisper returned it
    pub raw_text: &'a str,
    /// `HUSH_TRIGGER`
    pub trigger: Trigger,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Type this text
    Replace(String),
    /// The command exited with [`SKIP_EXIT_CODE`]: type nothing
    Skip,
    /// Type the original text
    Fallback {
        reason: String,
        stderr: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct TranscriptHook {
    command: String,
    timeout: Duration,
}

impl TranscriptHook {
    /// `None` when the command is blank.
    pub fn new(command: &str) -> Option<Self> {
        let command = command.trim();
        (!command.is_empty()).then(|| Self {
            command: command.to_string(),
            timeout: TIMEOUT,
        })
    }

    pub fn with_timeout(self, timeout: Duration) -> Self {
        Self { timeout, ..self }
    }

    pub fn command(&self) -> &str {
        &self.command
    }

    /// The text to type after the hook, or `None` to type nothing.
    pub async fn apply(&self, text: String, env: &HookEnv<'_>) -> Option<String> {
        if text.trim().is_empty() {
            return Some(text);
        }
        match self.run(&text, env).await {
            Outcome::Replace(replaced) => Some(replaced),
            Outcome::Skip => {
                info!(
                    "The transcript hook exited with {}, so Hush types nothing",
                    SKIP_EXIT_CODE
                );
                None
            },
            Outcome::Fallback { reason, stderr } => {
                match stderr {
                    Some(line) => warn!(
                        "The transcript hook {}, so Hush types the original text. stderr: {}",
                        reason, line
                    ),
                    None => warn!(
                        "The transcript hook {}, so Hush types the original text",
                        reason
                    ),
                }
                Some(text)
            },
        }
    }

    pub async fn run(&self, text: &str, env: &HookEnv<'_>) -> Outcome {
        let mut command = Command::new("sh");
        for name in crate::config::keys::loaded_from_env_files() {
            command.env_remove(name);
        }
        let spawned = command
            .arg("-c")
            .arg(&self.command)
            .env("HUSH_APP", env.app)
            .env("HUSH_PROFILE", env.profile)
            .env("HUSH_RAW_TEXT", env.raw_text)
            .env("HUSH_TRIGGER", env.trigger.as_str())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .kill_on_drop(true)
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(e) => {
                return Outcome::Fallback {
                    reason: format!("could not start ({})", e),
                    stderr: None,
                }
            },
        };
        let group = child.id();
        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let finished = tokio::time::timeout(self.timeout, async {
            let ((), out, err, status) = tokio::join!(
                feed(stdin, text),
                read_up_to(stdout, MAX_OUTPUT_BYTES as u64 + 1),
                read_up_to(stderr, MAX_STDERR_BYTES),
                child.wait()
            );
            (status, out, err)
        })
        .await;
        match finished {
            Ok((Ok(status), out, err)) => interpret(status.code(), out, &err),
            Ok((Err(e), _, _)) => Outcome::Fallback {
                reason: format!("could not be watched ({})", e),
                stderr: None,
            },
            Err(_) => {
                if let Some(group) = group {
                    kill_group(group);
                }
                let _ = child.wait().await;
                Outcome::Fallback {
                    reason: format!("ran longer than {:?}", self.timeout),
                    stderr: None,
                }
            },
        }
    }
}

async fn feed(stdin: Option<ChildStdin>, text: &str) {
    // When the command does not read stdin, this write fails with EPIPE. That is not an error.
    if let Some(mut stdin) = stdin {
        let _ = stdin.write_all(text.as_bytes()).await;
    }
}

async fn read_up_to<R: AsyncRead + Unpin>(pipe: Option<R>, limit: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    if let Some(pipe) = pipe {
        let _ = pipe.take(limit).read_to_end(&mut bytes).await;
    }
    bytes
}

fn kill_group(group: u32) {
    // SAFETY: killpg only sends a signal. The group is the hook's own, made by
    // process_group(0), and it outlives sh while any member holds a pipe.
    unsafe {
        libc::killpg(group as libc::pid_t, libc::SIGKILL);
    }
}

fn interpret(code: Option<i32>, stdout: Vec<u8>, stderr: &[u8]) -> Outcome {
    let fallback = |reason: &str| Outcome::Fallback {
        reason: reason.to_string(),
        stderr: first_line(stderr),
    };
    match code {
        Some(SKIP_EXIT_CODE) => return Outcome::Skip,
        Some(0) => {},
        Some(code) => return fallback(&format!("exited with code {}", code)),
        None => return fallback("was stopped by a signal"),
    }
    if stdout.len() > MAX_OUTPUT_BYTES {
        return fallback("printed more than 1 MiB");
    }
    let Ok(mut text) = String::from_utf8(stdout) else {
        return fallback("printed text that is not UTF-8");
    };
    if text.ends_with('\n') {
        text.pop();
    }
    if text.is_empty() {
        return fallback("printed nothing");
    }
    Outcome::Replace(text)
}

fn first_line(stderr: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(stderr);
    let line = text.lines().map(str::trim).find(|line| !line.is_empty())?;
    Some(line.chars().take(STDERR_LINE_CHARS).collect())
}

/// Where the first word of a hook command points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Program {
    /// An absolute path to an executable file
    Absolute(PathBuf),
    /// A bare name, found at this path on `PATH`
    OnPath(PathBuf),
    /// A file that exists but is not executable
    NotExecutable(PathBuf),
    /// No file at that path, or no such name on `PATH`
    Missing(String),
}

/// Resolve the first word of `command` the way `sh` finds it, with `path`
/// as `PATH` and `home` for a leading `~/`.
pub fn resolve_program(command: &str, path: Option<&OsStr>, home: Option<&Path>) -> Program {
    let word = first_word(command);
    let expanded = match (word.strip_prefix("~/"), home) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(word),
    };
    if word.contains('/') {
        if !expanded.is_absolute() || !expanded.exists() {
            return Program::Missing(word.to_string());
        }
        return if is_executable(&expanded) {
            Program::Absolute(expanded)
        } else {
            Program::NotExecutable(expanded)
        };
    }
    path.into_iter()
        .flat_map(std::env::split_paths)
        .map(|dir| dir.join(word))
        .find(|candidate| is_executable(candidate))
        .map(Program::OnPath)
        .unwrap_or_else(|| Program::Missing(word.to_string()))
}

fn first_word(command: &str) -> &str {
    let command = command.trim_start();
    for quote in ['\'', '"'] {
        if let Some(rest) = command.strip_prefix(quote) {
            return rest.split(quote).next().unwrap_or(rest);
        }
    }
    command
        .split(|c: char| c.is_whitespace() || ";|&<>()".contains(c))
        .next()
        .unwrap_or(command)
}

fn is_executable(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    const ENV: HookEnv<'static> = HookEnv {
        app: "org.kde.konsole",
        profile: "terminal",
        raw_text: "um hello world",
        trigger: Trigger::Hotkey,
    };

    async fn run(command: &str, text: &str) -> Outcome {
        TranscriptHook::new(command).unwrap().run(text, &ENV).await
    }

    #[test]
    fn a_blank_command_is_off() {
        assert!(TranscriptHook::new("").is_none());
        assert!(TranscriptHook::new("  ").is_none());
        let hook = TranscriptHook::new(" tr a-z A-Z ").unwrap();
        assert_eq!(hook.command(), "tr a-z A-Z");
        assert_eq!(hook.timeout, TIMEOUT);
        assert_eq!(TIMEOUT, Duration::from_secs(5));
    }

    #[tokio::test]
    async fn stdout_replaces_the_text() {
        assert_eq!(
            run("tr a-z A-Z", "hello world").await,
            Outcome::Replace("HELLO WORLD".to_string())
        );
    }

    #[tokio::test]
    async fn only_one_trailing_newline_goes() {
        assert_eq!(
            run("cat >/dev/null; echo hi", "x").await,
            Outcome::Replace("hi".to_string())
        );
        assert_eq!(
            run("cat >/dev/null; printf 'hi\\n\\n'", "x").await,
            Outcome::Replace("hi\n".to_string())
        );
        assert_eq!(
            run("cat", "  keep the spaces  ").await,
            Outcome::Replace("  keep the spaces  ".to_string())
        );
    }

    #[tokio::test]
    async fn exit_77_types_nothing() {
        assert_eq!(run("cat >/dev/null; exit 77", "x").await, Outcome::Skip);
        let hook = TranscriptHook::new("exit 77").unwrap();
        assert_eq!(hook.apply("hello".to_string(), &ENV).await, None);
    }

    #[tokio::test]
    async fn a_failure_or_no_output_types_the_original() {
        assert_eq!(
            run("echo partial; echo boom >&2; exit 1", "x").await,
            Outcome::Fallback {
                reason: "exited with code 1".to_string(),
                stderr: Some("boom".to_string()),
            }
        );
        assert_eq!(
            run("cat >/dev/null", "x").await,
            Outcome::Fallback {
                reason: "printed nothing".to_string(),
                stderr: None,
            }
        );
        assert!(matches!(
            run("echo", "x").await,
            Outcome::Fallback { reason, .. } if reason == "printed nothing"
        ));
        assert!(matches!(
            run("no-such-hush-hook-command", "x").await,
            Outcome::Fallback { reason, stderr: Some(_) } if reason == "exited with code 127"
        ));
        for command in ["exit 1", "cat >/dev/null"] {
            let hook = TranscriptHook::new(command).unwrap();
            assert_eq!(
                hook.apply("hello".to_string(), &ENV).await.as_deref(),
                Some("hello"),
                "{command}"
            );
        }
    }

    #[tokio::test]
    async fn a_command_that_cannot_start_types_the_original() {
        let env = HookEnv {
            raw_text: "nul\0byte",
            ..ENV
        };
        let hook = TranscriptHook::new("cat").unwrap();
        assert!(matches!(
            hook.run("hello", &env).await,
            Outcome::Fallback { reason, .. } if reason.starts_with("could not start")
        ));
        assert_eq!(
            hook.apply("hello".to_string(), &env).await.as_deref(),
            Some("hello")
        );
    }

    fn gone(pid: &str) -> bool {
        match std::fs::read_to_string(format!("/proc/{}/stat", pid)) {
            Err(_) => true,
            Ok(stat) => stat
                .rsplit_once(") ")
                .is_some_and(|(_, rest)| rest.starts_with('Z')),
        }
    }

    #[tokio::test]
    async fn a_slow_command_times_out_and_its_group_is_killed() {
        let dir = tempfile::tempdir().unwrap();
        let pids = dir.path().join("pids");
        let command = format!("sleep 10 & echo \"$$ $!\" > '{}'; wait", pids.display());
        let hook = TranscriptHook::new(&command)
            .unwrap()
            .with_timeout(Duration::from_millis(200));
        let started = Instant::now();
        let outcome = hook.run("hello", &ENV).await;
        assert!(started.elapsed() < Duration::from_secs(3));
        assert_eq!(
            outcome,
            Outcome::Fallback {
                reason: "ran longer than 200ms".to_string(),
                stderr: None,
            }
        );
        let pids = std::fs::read_to_string(&pids).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        for pid in pids.split_whitespace() {
            while !gone(pid) && Instant::now() < deadline {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            assert!(gone(pid), "process {pid} is still alive");
        }
    }

    #[tokio::test]
    async fn the_environment_arrives() {
        let env = HookEnv {
            trigger: Trigger::Ipc,
            ..ENV
        };
        let hook = TranscriptHook::new(
            r#"cat >/dev/null; printf %s "$HUSH_APP|$HUSH_PROFILE|$HUSH_RAW_TEXT|$HUSH_TRIGGER""#,
        )
        .unwrap();
        assert_eq!(
            hook.run("Hello world.", &env).await,
            Outcome::Replace("org.kde.konsole|terminal|um hello world|ipc".to_string())
        );
    }

    #[tokio::test]
    async fn keys_from_env_files_stay_out_of_the_hook() {
        std::env::set_var("HUSH_TEST_HOOK_SESSION", "from-session");
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".env");
        std::fs::write(
            &file,
            "HUSH_TEST_HOOK_KEY=secret\nHUSH_TEST_HOOK_SESSION=from-file\n",
        )
        .unwrap();
        crate::config::keys::load_with(|| {
            let _ = dotenvy::from_path(&file);
        });
        assert_eq!(std::env::var("HUSH_TEST_HOOK_KEY").as_deref(), Ok("secret"));
        assert_eq!(
            run(
                r#"printf %s "${HUSH_TEST_HOOK_KEY-unset}|$HUSH_TEST_HOOK_SESSION""#,
                "x"
            )
            .await,
            Outcome::Replace("unset|from-session".to_string())
        );
    }

    #[tokio::test]
    async fn a_whitespace_take_skips_the_hook() {
        let hook = TranscriptHook::new("exit 77").unwrap();
        for text in ["\n", "\n\n", " \t "] {
            assert_eq!(
                hook.apply(text.to_string(), &ENV).await.as_deref(),
                Some(text),
                "{text:?}"
            );
        }
        assert_eq!(hook.apply("\nword\n".to_string(), &ENV).await, None);
    }

    #[test]
    fn the_exit_status_and_output_decide() {
        assert_eq!(
            interpret(Some(0), b"ok\n".to_vec(), b""),
            Outcome::Replace("ok".to_string())
        );
        assert_eq!(interpret(Some(77), b"ignored".to_vec(), b""), Outcome::Skip);
        assert_eq!(
            interpret(None, b"ok".to_vec(), b"\n  first  \nsecond\n"),
            Outcome::Fallback {
                reason: "was stopped by a signal".to_string(),
                stderr: Some("first".to_string()),
            }
        );
        assert!(matches!(
            interpret(Some(0), vec![0xff, 0xfe], b""),
            Outcome::Fallback { reason, .. } if reason.contains("UTF-8")
        ));
        assert!(matches!(
            interpret(Some(0), vec![b'a'; MAX_OUTPUT_BYTES + 1], b""),
            Outcome::Fallback { reason, .. } if reason.contains("1 MiB")
        ));
        assert_eq!(
            interpret(Some(0), b"\n".to_vec(), b""),
            Outcome::Fallback {
                reason: "printed nothing".to_string(),
                stderr: None,
            }
        );
    }

    #[test]
    fn triggers_have_fixed_names() {
        let names: Vec<_> = [
            Trigger::Hotkey,
            Trigger::Ipc,
            Trigger::Tray,
            Trigger::Command,
            Trigger::Rewrite,
        ]
        .map(Trigger::as_str)
        .to_vec();
        assert_eq!(names, ["hotkey", "ipc", "tray", "command", "rewrite"]);
    }

    #[test]
    fn the_first_word_resolves_like_sh_finds_it() {
        let dir = tempfile::tempdir().unwrap();
        let tool = dir.path().join("fixit");
        std::fs::write(&tool, "#!/bin/sh\ncat\n").unwrap();
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
        let plain = dir.path().join("plain");
        std::fs::write(&plain, "").unwrap();
        let path = std::env::join_paths(["/nonexistent", dir.path().to_str().unwrap()]).unwrap();
        let path = Some(path.as_os_str());

        assert_eq!(
            resolve_program("fixit --loud", path, None),
            Program::OnPath(tool.clone())
        );
        assert_eq!(
            resolve_program("fixit|tr a b", path, None),
            Program::OnPath(tool.clone())
        );
        assert_eq!(
            resolve_program(&format!("{} --loud", tool.display()), path, None),
            Program::Absolute(tool.clone())
        );
        assert_eq!(
            resolve_program(&format!("'{}' x", tool.display()), None, None),
            Program::Absolute(tool.clone())
        );
        assert_eq!(
            resolve_program("~/fixit", None, Some(dir.path())),
            Program::Absolute(tool.clone())
        );
        assert_eq!(
            resolve_program(&plain.display().to_string(), None, None),
            Program::NotExecutable(plain.clone())
        );
        assert_eq!(
            resolve_program("plain", path, None),
            Program::Missing("plain".to_string()),
            "a file on PATH that is not executable does not run"
        );
        assert_eq!(
            resolve_program("nothing-here x", path, None),
            Program::Missing("nothing-here".to_string())
        );
        assert_eq!(
            resolve_program("/nonexistent/fixit", path, None),
            Program::Missing("/nonexistent/fixit".to_string())
        );
        assert_eq!(
            resolve_program("bin/fixit", path, None),
            Program::Missing("bin/fixit".to_string()),
            "a relative path depends on the daemon's working directory"
        );
    }
}
