//! `hush transcribe <file>` and `hush recover`: text from audio files, with
//! the daemon's model when the daemon runs, else with a model loaded here.

use super::utils::print_line;
use crate::audio::file;
use crate::audio::store::{self, Kept};
use crate::cli::exit::{Exit, ExitStatus};
use crate::config::{keys, paths, Config};
use crate::ipc::client::{self, NotRunning};
use crate::ipc::{DaemonCommand, TakeResult};
use crate::transcription::{SpeechToText, WHISPER_SAMPLE_RATE};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

const NO_SPEECH: &str = "Hush heard no speech.";

/// Turns an audio file into text; an empty string means no speech.
#[async_trait(?Send)]
pub trait FileTranscriber {
    async fn transcribe(&mut self, path: &Path) -> Result<String>;
}

/// The daemon's model, or a model that this process loads when no daemon
/// answers.
enum Engine {
    Daemon,
    Local(Box<SpeechToText>),
}

#[async_trait(?Send)]
impl FileTranscriber for Engine {
    async fn transcribe(&mut self, path: &Path) -> Result<String> {
        if matches!(self, Engine::Daemon) {
            if let Some(text) = ask_daemon(path).await? {
                return Ok(text);
            }
        }
        let samples = file::read_for_whisper(path)?;
        if matches!(self, Engine::Daemon) {
            *self = Engine::Local(Box::new(load_local().await?));
        }
        let Engine::Local(engine) = self else {
            return Err(anyhow!("No transcriber is ready"));
        };
        Ok(engine
            .transcribe_with_prompt(&samples, WHISPER_SAMPLE_RATE, None)
            .await?
            .text)
    }
}

/// The daemon's text, or `None` when no daemon runs or it is too old to
/// transcribe a file.
async fn ask_daemon(path: &Path) -> Result<Option<String>> {
    let command = DaemonCommand::Transcribe {
        path: path.to_path_buf(),
    };
    let reply = match client::send(command).await {
        Ok(reply) => reply,
        Err(e) if e.downcast_ref::<NotRunning>().is_some() => return Ok(None),
        Err(e) => return Err(e),
    };
    if !reply.ok {
        let error = reply.error.unwrap_or_default();
        if error.starts_with("Invalid command") {
            debug!("The daemon is too old to transcribe a file: {}", error);
            return Ok(None);
        }
        return Err(anyhow!(error));
    }
    match reply.take {
        Some(TakeResult::Ok { text }) => Ok(Some(text)),
        Some(TakeResult::NoSpeech) => Ok(Some(String::new())),
        Some(TakeResult::Error { message }) => Err(anyhow!(message)),
        None => Ok(None),
    }
}

async fn load_local() -> Result<SpeechToText> {
    let config = Config::load()?;
    keys::load_env_files();
    SpeechToText::from_config(
        &config.transcription,
        &paths::models_dir(),
        &keys::env_file(),
    )
    .await
}

/// `hush transcribe <file>`
pub async fn handle_transcribe(file: &Path) -> Result<()> {
    let path = std::fs::canonicalize(file)
        .with_context(|| format!("Failed to open {}", file.display()))?;
    let text = Engine::Daemon.transcribe(&path).await?;
    if text.trim().is_empty() {
        return Err(Exit::new(ExitStatus::NoSpeech, NO_SPEECH).into());
    }
    print_line(&text)
}

/// What `hush recover` did with the files it got.
#[derive(Debug, Default)]
pub struct Recovery {
    /// The text of each file that had speech, in the order of the files
    pub texts: Vec<String>,
    /// Files without speech; they are deleted too
    pub silent: Vec<PathBuf>,
    /// Files that did not transcribe, and why; they stay
    pub failed: Vec<(PathBuf, String)>,
}

/// Transcribe each file in turn and give the texts to `deliver`. When the
/// delivery works, delete each file that transcribed. A file that failed
/// stays for the next try.
pub async fn recover(
    files: &[Kept],
    engine: &mut dyn FileTranscriber,
    deliver: impl FnOnce(&[String]) -> Result<()>,
) -> Result<Recovery> {
    let mut recovery = Recovery::default();
    let mut done = Vec::new();
    for kept in files {
        match engine.transcribe(&kept.path).await {
            Ok(text) if text.trim().is_empty() => {
                recovery.silent.push(kept.path.clone());
                done.push(&kept.path);
            },
            Ok(text) => {
                recovery.texts.push(text.trim().to_string());
                done.push(&kept.path);
            },
            Err(e) => recovery
                .failed
                .push((kept.path.clone(), format!("{:#}", e))),
        }
    }
    if !recovery.texts.is_empty() {
        deliver(&recovery.texts)?;
    }
    for path in done {
        if let Err(e) = std::fs::remove_file(path) {
            warn!("Could not delete {}: {}", path.display(), e);
        }
    }
    Ok(recovery)
}

/// `hush recover [--all] [--list] [--type]`
pub async fn handle_recover(all: bool, list: bool, type_text: bool) -> Result<()> {
    let dir = store::recovery_dir();
    let mut kept = store::list(&dir)?;
    if kept.is_empty() {
        return print_line("Hush keeps no audio to recover.");
    }
    if list {
        for file in &kept {
            print_line(&describe(file))?;
        }
        return Ok(());
    }
    if !all {
        kept = kept.split_off(kept.len() - 1);
    }
    let mut engine = Engine::Daemon;
    let recovery = recover(&kept, &mut engine, |texts| {
        if type_text {
            type_texts(texts)
        } else {
            texts.iter().try_for_each(|text| print_line(text))
        }
    })
    .await?;
    for path in &recovery.silent {
        eprintln!("{} had no speech. Hush deleted it.", path.display());
    }
    for (path, error) in &recovery.failed {
        eprintln!(
            "Could not transcribe {}: {}. The file stays.",
            path.display(),
            error
        );
    }
    match (recovery.failed.is_empty(), recovery.texts.is_empty()) {
        (false, _) => Err(Exit::quiet(ExitStatus::Error).into()),
        (true, true) => Err(Exit::quiet(ExitStatus::NoSpeech).into()),
        (true, false) => Ok(()),
    }
}

#[cfg(target_os = "linux")]
fn type_texts(texts: &[String]) -> Result<()> {
    let joined = texts.join(" ");
    let typed = crate::TextInserter::new().and_then(|mut inserter| inserter.insert_text(&joined));
    if let Err(e) = typed {
        eprintln!("Could not type the text ({:#}), so here it is:", e);
        print_line(&joined)?;
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn type_texts(texts: &[String]) -> Result<()> {
    print_line(&texts.join(" "))
}

fn describe(kept: &Kept) -> String {
    let when: chrono::DateTime<chrono::Local> = kept.modified.into();
    let length = file::flac_length(&kept.path)
        .map(|length| format!("{:>6.1} s", length.as_secs_f64()))
        .unwrap_or_else(|_| "     ? s".to_string());
    format!(
        "{}  {}  {}",
        when.format("%Y-%m-%d %H:%M"),
        length,
        kept.path.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;

    /// Answers by file: a text, or an error when the map has none.
    struct Fake {
        texts: HashMap<PathBuf, String>,
        asked: Vec<PathBuf>,
    }

    #[async_trait(?Send)]
    impl FileTranscriber for Fake {
        async fn transcribe(&mut self, path: &Path) -> Result<String> {
            self.asked.push(path.to_path_buf());
            self.texts
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow!("the model did not load"))
        }
    }

    #[tokio::test]
    async fn a_failed_take_is_kept_then_recovered_and_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let recovery = dir.path().join("recover");
        let speech = store::save(&recovery, &[0.2; 1600], Utc::now()).unwrap();
        let silence = store::save(&recovery, &[0.0; 1600], Utc::now()).unwrap();
        let broken = store::save(&recovery, &[0.3; 1600], Utc::now()).unwrap();
        let kept = store::list(&recovery).unwrap();
        assert_eq!(kept.len(), 3);

        let mut engine = Fake {
            texts: HashMap::from([
                (speech.clone(), " Ship it. ".to_string()),
                (silence.clone(), String::new()),
            ]),
            asked: Vec::new(),
        };
        let mut delivered = Vec::new();
        let result = recover(&kept, &mut engine, |texts| {
            delivered = texts.to_vec();
            Ok(())
        })
        .await
        .unwrap();

        assert_eq!(engine.asked.len(), 3, "every file is tried");
        assert_eq!(delivered, ["Ship it."]);
        assert_eq!(result.silent, std::slice::from_ref(&silence));
        assert_eq!(result.failed.len(), 1);
        assert_eq!(result.failed[0].0, broken);
        assert!(result.failed[0].1.contains("did not load"));
        assert!(!speech.exists() && !silence.exists());
        assert!(broken.exists(), "a failed file stays for the next try");
        assert_eq!(store::list(&recovery).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_failed_delivery_keeps_every_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = store::save(dir.path(), &[0.2; 1600], Utc::now()).unwrap();
        let kept = store::list(dir.path()).unwrap();
        let mut engine = Fake {
            texts: HashMap::from([(path.clone(), "Hello.".to_string())]),
            asked: Vec::new(),
        };
        let result = recover(&kept, &mut engine, |_| Err(anyhow!("stdout is closed"))).await;
        assert!(result.is_err());
        assert!(path.exists());
    }
}
