//! Each transcript Hush delivers, kept on disk for the History page.
//!
//! The file holds one JSON object per line, oldest first, and only its owner
//! can read it. The daemon appends and the settings window deletes, so each
//! change holds an exclusive lock and rewrites the file in place: after a
//! rename, a writer that waits on the lock would write to the old file.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use tracing::warn;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transcript {
    pub at: DateTime<Utc>,
    pub text: String,
    /// The class of the focused window when the recording started; empty
    /// when the compositor does not report it
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub app: String,
    /// The name of the take's FLAC file in the audio folder, when Hush kept
    /// the audio
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<String>,
}

impl Transcript {
    pub fn new(text: impl Into<String>, app: impl Into<String>) -> Self {
        Self {
            at: Utc::now(),
            text: text.into(),
            app: app.into(),
            audio: None,
        }
    }

    pub fn with_audio(mut self, file_name: impl Into<String>) -> Self {
        self.audio = Some(file_name.into());
        self
    }
}

/// `$XDG_DATA_HOME/hush/history.jsonl`
pub fn default_path() -> PathBuf {
    crate::config::paths::data_dir().join("history.jsonl")
}

/// The transcripts in the file, oldest first. A missing file is an empty
/// history, and a line that does not parse is skipped.
pub fn read(path: &Path) -> Result<Vec<Transcript>> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("Failed to open {}", path.display())),
    };
    file.lock_shared()
        .with_context(|| format!("Failed to lock {}", path.display()))?;
    Ok(parse(&mut file)?.entries)
}

/// Add a transcript, then drop the oldest beyond `max_entries`.
pub fn append(path: &Path, entry: &Transcript, max_entries: usize) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("Failed to open {}", path.display()))?;
    file.lock()
        .with_context(|| format!("Failed to lock {}", path.display()))?;
    let Parsed { mut entries, clean } = parse(&mut file)?;
    entries.push(entry.clone());
    if entries.len() > max_entries || !clean {
        let excess = entries.len().saturating_sub(max_entries);
        entries.drain(..excess);
        return rewrite(&mut file, &entries);
    }
    file.seek(SeekFrom::End(0))?;
    file.write_all(&line(entry)?)?;
    Ok(())
}

/// Remove one transcript; returns whether the file held it.
pub fn delete(path: &Path, entry: &Transcript) -> Result<bool> {
    let Some(mut file) = open_existing(path)? else {
        return Ok(false);
    };
    let mut entries = parse(&mut file)?.entries;
    let Some(index) = entries.iter().position(|kept| kept == entry) else {
        return Ok(false);
    };
    entries.remove(index);
    rewrite(&mut file, &entries)?;
    Ok(true)
}

/// Remove every transcript.
pub fn clear(path: &Path) -> Result<()> {
    if let Some(file) = open_existing(path)? {
        file.set_len(0)
            .with_context(|| format!("Failed to clear {}", path.display()))?;
    }
    Ok(())
}

fn open_existing(path: &Path) -> Result<Option<File>> {
    let file = match OpenOptions::new().read(true).write(true).open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("Failed to open {}", path.display())),
    };
    file.lock()
        .with_context(|| format!("Failed to lock {}", path.display()))?;
    Ok(Some(file))
}

struct Parsed {
    entries: Vec<Transcript>,
    /// Every line parsed and the file ends with a newline, so a new line can
    /// go at the end as it is
    clean: bool,
}

fn parse(file: &mut File) -> Result<Parsed> {
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let mut clean = bytes.is_empty() || bytes.ends_with(b"\n");
    let mut entries = Vec::new();
    for text in String::from_utf8_lossy(&bytes).lines() {
        if text.trim().is_empty() {
            continue;
        }
        match serde_json::from_str(text) {
            Ok(entry) => entries.push(entry),
            Err(e) => {
                warn!("Skipping a history line that does not parse: {}", e);
                clean = false;
            },
        }
    }
    Ok(Parsed { entries, clean })
}

fn line(entry: &Transcript) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(entry).context("Failed to encode a transcript")?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn rewrite(file: &mut File, entries: &[Transcript]) -> Result<()> {
    let mut bytes = Vec::new();
    for entry in entries {
        bytes.extend(line(entry)?);
    }
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(&bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn texts(path: &Path) -> Vec<String> {
        read(path)
            .unwrap()
            .into_iter()
            .map(|entry| entry.text)
            .collect()
    }

    #[test]
    fn a_missing_file_is_an_empty_history() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.jsonl");
        assert!(read(&path).unwrap().is_empty());
        assert!(!delete(&path, &Transcript::new("x", "")).unwrap());
        clear(&path).unwrap();
        assert!(!path.exists(), "clearing creates nothing");
    }

    #[test]
    fn appends_keep_order_and_only_the_owner_can_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("history.jsonl");
        append(&path, &Transcript::new("first", "org.kde.konsole"), 10).unwrap();
        append(&path, &Transcript::new("second", ""), 10).unwrap();
        let entries = read(&path).unwrap();
        assert_eq!(texts(&path), ["first", "second"]);
        assert_eq!(entries[0].app, "org.kde.konsole");
        assert_eq!(entries[1].app, "");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn the_oldest_go_beyond_the_cap() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.jsonl");
        for n in 0..5 {
            append(&path, &Transcript::new(n.to_string(), ""), 3).unwrap();
        }
        assert_eq!(texts(&path), ["2", "3", "4"]);
        append(&path, &Transcript::new("5", ""), 1).unwrap();
        assert_eq!(texts(&path), ["5"], "a lower cap trims at the next append");
    }

    #[test]
    fn delete_removes_one_match_and_clear_empties() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.jsonl");
        let keep = Transcript::new("keep", "");
        let gone = Transcript::new("gone", "kitty");
        append(&path, &keep, 10).unwrap();
        append(&path, &gone, 10).unwrap();
        let stored = read(&path).unwrap();
        assert_eq!(stored[1], gone, "a transcript survives the round trip");
        assert!(delete(&path, &stored[1]).unwrap());
        assert!(!delete(&path, &stored[1]).unwrap(), "already gone");
        assert_eq!(texts(&path), ["keep"]);
        clear(&path).unwrap();
        assert!(read(&path).unwrap().is_empty());
        append(&path, &Transcript::new("after", ""), 10).unwrap();
        assert_eq!(texts(&path), ["after"]);
    }

    #[test]
    fn entries_with_and_without_audio_share_one_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.jsonl");
        std::fs::write(
            &path,
            "{\"at\":\"2026-01-02T03:04:05Z\",\"text\":\"from an older Hush\",\"app\":\"kitty\"}\n",
        )
        .unwrap();
        let kept = Transcript::new("with sound", "").with_audio("20261109T163405123Z.flac");
        append(&path, &kept, 10).unwrap();
        append(&path, &Transcript::new("without", ""), 10).unwrap();

        let entries = read(&path).unwrap();
        assert_eq!(entries[0].text, "from an older Hush");
        assert_eq!(entries[0].audio, None);
        assert_eq!(entries[1], kept);
        assert_eq!(entries[2].audio, None);
        let raw = std::fs::read_to_string(&path).unwrap();
        assert_eq!(raw.matches("\"audio\"").count(), 1, "{raw}");
        assert!(delete(&path, &kept).unwrap());
        assert_eq!(texts(&path), ["from an older Hush", "without"]);
    }

    #[test]
    fn a_damaged_file_is_repaired_on_the_next_append() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.jsonl");
        append(&path, &Transcript::new("good", ""), 10).unwrap();
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(b"not json\n{\"at\":\"2026-").unwrap();
        drop(file);
        assert_eq!(texts(&path), ["good"]);
        append(&path, &Transcript::new("next", ""), 10).unwrap();
        assert_eq!(texts(&path), ["good", "next"]);
        let raw = std::fs::read_to_string(&path).unwrap();
        assert_eq!(raw.lines().count(), 2, "the broken lines are gone");
    }
}
