//! The audio that Hush keeps on disk, as FLAC files that only the user can
//! read, in folders that only the user can open:
//!
//! - `audio/`: the audio of each take, next to its history entry, when
//!   `history.keep_audio_days` is more than 0.
//! - `recover/`: the audio of a take that Hush could not transcribe, for
//!   `hush recover`, kept for [`RECOVERY_DAYS`] whatever the setting.
//! - `parts/`: the parts of a hands-free take until Hush inserts its text.
//!   A part that fails moves to `recover/`, and so do the parts that a
//!   stopped daemon left.
//!
//! [`sweep`] deletes files older than their limit, then the oldest files of
//! both folders while the two together hold more than [`MAX_BYTES`].

use crate::audio::file;
use crate::transcription::WHISPER_SAMPLE_RATE;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Days that the audio of a failed take stays for `hush recover`.
pub const RECOVERY_DAYS: u64 = 7;
/// The most that the two folders hold together.
pub const MAX_BYTES: u64 = 500_000_000;
const DAY: Duration = Duration::from_secs(24 * 60 * 60);
const EXTENSION: &str = "flac";

/// `$XDG_DATA_HOME/hush/audio`
pub fn takes_dir() -> PathBuf {
    crate::config::paths::data_dir().join("audio")
}

/// `$XDG_DATA_HOME/hush/recover`
pub fn recovery_dir() -> PathBuf {
    crate::config::paths::data_dir().join("recover")
}

/// `$XDG_DATA_HOME/hush/parts`
pub fn parts_dir() -> PathBuf {
    crate::config::paths::data_dir().join("parts")
}

/// Make `dir` with mode 0700, and give an existing folder that mode.
pub fn private_dir(dir: &Path) -> Result<()> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .with_context(|| format!("Failed to create {}", dir.display()))?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
        .with_context(|| format!("Failed to restrict {}", dir.display()))
}

/// Save a take recorded at 16 kHz in `dir`, named for `at`. Returns the path.
pub fn save(dir: &Path, samples: &[f32], at: DateTime<Utc>) -> Result<PathBuf> {
    save_as(dir, &stem(at), samples)
}

/// The file name of a take that started at `at`, without its extension.
pub fn stem(at: DateTime<Utc>) -> String {
    at.format("%Y%m%dT%H%M%S%3fZ").to_string()
}

/// Save a take recorded at 16 kHz in `dir` as `<stem>.flac`, or with a
/// number after the stem when that name is taken. Returns the path.
pub fn save_as(dir: &Path, stem: &str, samples: &[f32]) -> Result<PathBuf> {
    private_dir(dir)?;
    let mut path = dir.join(format!("{}.{}", stem, EXTENSION));
    let mut n = 1;
    while path.exists() {
        path = dir.join(format!("{}-{}.{}", stem, n, EXTENSION));
        n += 1;
    }
    file::write_flac(&path, samples, WHISPER_SAMPLE_RATE)?;
    Ok(path)
}

/// Move a kept file into the recovery folder. Returns its new path.
pub fn move_to_recovery(path: &Path, recovery: &Path) -> Result<PathBuf> {
    private_dir(recovery)?;
    let name = path
        .file_name()
        .with_context(|| format!("{} names no file", path.display()))?;
    let target = recovery.join(name);
    std::fs::rename(path, &target).with_context(|| {
        format!(
            "Failed to move {} to {}",
            path.display(),
            recovery.display()
        )
    })?;
    Ok(target)
}

/// Move every part that `parts` holds into `recovery`: no take waits for
/// them any more. Returns how many moved.
pub fn recover_leftover_parts(parts: &Path, recovery: &Path) -> Result<usize> {
    let mut moved = 0;
    for kept in list(parts)? {
        move_to_recovery(&kept.path, recovery)?;
        moved += 1;
    }
    Ok(moved)
}

/// One kept file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    pub path: PathBuf,
    pub modified: SystemTime,
    pub bytes: u64,
}

/// The FLAC files in `dir`, oldest first. A missing folder holds none.
pub fn list(dir: &Path) -> Result<Vec<Kept>> {
    let mut kept: Vec<Kept> = files(dir)?
        .into_iter()
        .filter(|kept| kept.path.extension().is_some_and(|ext| ext == EXTENSION))
        .collect();
    sort_oldest_first(&mut kept);
    Ok(kept)
}

fn files(dir: &Path) -> Result<Vec<Kept>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("Failed to read {}", dir.display())),
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.with_context(|| format!("Failed to read {}", dir.display()))?;
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        files.push(Kept {
            path: entry.path(),
            modified: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            bytes: meta.len(),
        });
    }
    Ok(files)
}

fn sort_oldest_first(kept: &mut [Kept]) {
    kept.sort_by(|a, b| a.modified.cmp(&b.modified).then(a.path.cmp(&b.path)));
}

/// How long each folder keeps its files, and how much they hold together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    /// 0 keeps no take audio at all
    pub keep_days: u64,
    pub recovery_days: u64,
    pub max_bytes: u64,
}

impl Policy {
    /// `history.keep_audio_days`, with the fixed recovery days and size cap.
    pub fn new(keep_days: u64) -> Self {
        Self {
            keep_days,
            recovery_days: RECOVERY_DAYS,
            max_bytes: MAX_BYTES,
        }
    }
}

/// What a sweep deleted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Swept {
    pub files: usize,
    pub bytes: u64,
}

/// Delete the files older than the policy allows, then the oldest files of
/// both folders until they hold no more than the cap. `now` is the clock.
pub fn sweep(takes: &Path, recovery: &Path, policy: Policy, now: SystemTime) -> Result<Swept> {
    let mut swept = Swept::default();
    let mut remaining = Vec::new();
    for (dir, days) in [(takes, policy.keep_days), (recovery, policy.recovery_days)] {
        let limit = Duration::from_secs(days.saturating_mul(DAY.as_secs()));
        for kept in files(dir)? {
            let age = now.duration_since(kept.modified).unwrap_or_default();
            if days == 0 || age > limit {
                remove(&kept, &mut swept)?;
            } else {
                remaining.push(kept);
            }
        }
    }
    sort_oldest_first(&mut remaining);
    let mut total: u64 = remaining.iter().map(|kept| kept.bytes).sum();
    for kept in &remaining {
        if total <= policy.max_bytes {
            break;
        }
        remove(kept, &mut swept)?;
        total -= kept.bytes;
    }
    Ok(swept)
}

fn remove(kept: &Kept, swept: &mut Swept) -> Result<()> {
    match std::fs::remove_file(&kept.path) {
        Ok(()) => {},
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(e).with_context(|| format!("Failed to delete {}", kept.path.display()))
        },
    }
    swept.files += 1;
    swept.bytes += kept.bytes;
    Ok(())
}

/// The kept takes that a limit of `keep_days` would delete, by age alone.
pub fn older_than(kept: &[Kept], keep_days: u64, now: SystemTime) -> Swept {
    let limit = Duration::from_secs(keep_days.saturating_mul(DAY.as_secs()));
    kept.iter()
        .filter(|kept| {
            keep_days == 0 || now.duration_since(kept.modified).unwrap_or_default() > limit
        })
        .fold(Swept::default(), |swept, kept| Swept {
            files: swept.files + 1,
            bytes: swept.bytes + kept.bytes,
        })
}

/// [`sweep`] of the default folders, now.
pub fn sweep_now(keep_days: u64) -> Result<Swept> {
    sweep(
        &takes_dir(),
        &recovery_dir(),
        Policy::new(keep_days),
        SystemTime::now(),
    )
}

/// Delete one file of the take audio folder by its name. A name with a
/// folder in it deletes nothing.
pub fn delete_take(dir: &Path, name: &str) -> Result<()> {
    if Path::new(name).file_name() != Some(std::ffi::OsStr::new(name)) {
        return Ok(());
    }
    match std::fs::remove_file(dir.join(name)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(e).with_context(|| format!("Failed to delete {}", name))
        },
        _ => Ok(()),
    }
}

/// Delete every file in `dir`.
pub fn clear(dir: &Path) -> Result<()> {
    let mut swept = Swept::default();
    for kept in files(dir)? {
        remove(&kept, &mut swept)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn a_lower_limit_counts_the_takes_it_would_delete() {
        let now = SystemTime::now();
        let take = |days: u64, bytes: u64| Kept {
            path: PathBuf::from(format!("{days}.flac")),
            modified: now - DAY * days as u32,
            bytes,
        };
        let kept = [take(1, 10), take(5, 20), take(40, 30)];
        assert_eq!(
            older_than(&kept, 30, now),
            Swept {
                files: 1,
                bytes: 30
            }
        );
        assert_eq!(
            older_than(&kept, 3, now),
            Swept {
                files: 2,
                bytes: 50
            }
        );
        assert_eq!(
            older_than(&kept, 0, now),
            Swept {
                files: 3,
                bytes: 60
            }
        );
        assert_eq!(older_than(&kept, 60, now), Swept::default());
    }

    struct Folders {
        _root: tempfile::TempDir,
        takes: PathBuf,
        recovery: PathBuf,
    }

    fn folders() -> Folders {
        let root = tempfile::tempdir().unwrap();
        let takes = root.path().join("audio");
        let recovery = root.path().join("recover");
        Folders {
            _root: root,
            takes,
            recovery,
        }
    }

    /// A file of `bytes` bytes, last changed `days_old` days before `now`.
    fn plant(dir: &Path, name: &str, bytes: usize, now: SystemTime, days_old: f64) -> PathBuf {
        private_dir(dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, vec![0u8; bytes]).unwrap();
        let modified = now - Duration::from_secs_f64(days_old * DAY.as_secs_f64());
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        path
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = files(dir)
            .unwrap()
            .into_iter()
            .map(|kept| {
                kept.path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }

    fn policy(keep_days: u64, max_bytes: u64) -> Policy {
        Policy {
            keep_days,
            recovery_days: RECOVERY_DAYS,
            max_bytes,
        }
    }

    #[test]
    fn files_older_than_their_folder_allows_go() {
        let f = folders();
        let now = SystemTime::now();
        plant(&f.takes, "new.flac", 10, now, 1.0);
        plant(&f.takes, "old.flac", 10, now, 31.0);
        plant(&f.recovery, "fresh.flac", 10, now, 6.5);
        plant(&f.recovery, "stale.flac", 10, now, 7.5);

        let swept = sweep(&f.takes, &f.recovery, policy(30, MAX_BYTES), now).unwrap();
        assert_eq!(
            swept,
            Swept {
                files: 2,
                bytes: 20
            }
        );
        assert_eq!(names(&f.takes), ["new.flac"]);
        assert_eq!(names(&f.recovery), ["fresh.flac"]);

        let later = now + 2 * DAY;
        sweep(&f.takes, &f.recovery, policy(30, MAX_BYTES), later).unwrap();
        assert_eq!(names(&f.takes), ["new.flac"]);
        assert!(
            names(&f.recovery).is_empty(),
            "the recovery days count on the clock"
        );
    }

    #[test]
    fn retention_off_deletes_the_take_audio_but_not_the_recovery_audio() {
        let f = folders();
        let now = SystemTime::now();
        plant(&f.takes, "a.flac", 10, now, 0.0);
        plant(&f.recovery, "b.flac", 10, now, 0.0);
        sweep(&f.takes, &f.recovery, policy(0, MAX_BYTES), now).unwrap();
        assert!(names(&f.takes).is_empty());
        assert_eq!(names(&f.recovery), ["b.flac"]);
    }

    #[test]
    fn over_the_cap_the_oldest_go_first_from_both_folders() {
        let f = folders();
        let now = SystemTime::now();
        plant(&f.takes, "1.flac", 100, now, 5.0);
        plant(&f.recovery, "2.flac", 100, now, 4.0);
        plant(&f.takes, "3.flac", 100, now, 3.0);
        plant(&f.recovery, "4.flac", 100, now, 2.0);

        let swept = sweep(&f.takes, &f.recovery, policy(30, 250), now).unwrap();
        assert_eq!(
            swept,
            Swept {
                files: 2,
                bytes: 200
            }
        );
        assert_eq!(names(&f.takes), ["3.flac"]);
        assert_eq!(names(&f.recovery), ["4.flac"]);

        let swept = sweep(&f.takes, &f.recovery, policy(30, 250), now).unwrap();
        assert_eq!(swept, Swept::default(), "under the cap, nothing goes");
    }

    #[test]
    fn missing_folders_sweep_to_nothing() {
        let f = folders();
        let swept = sweep(&f.takes, &f.recovery, policy(7, 0), SystemTime::now()).unwrap();
        assert_eq!(swept, Swept::default());
        assert!(!f.takes.exists(), "a sweep creates no folder");
    }

    #[test]
    fn saved_takes_are_private_and_listed_oldest_first() {
        let f = folders();
        let at = DateTime::parse_from_rfc3339("2026-11-09T16:34:05.123Z")
            .unwrap()
            .with_timezone(&Utc);
        let samples = vec![0.25f32; 1600];
        let first = save(&f.takes, &samples, at).unwrap();
        let second = save(&f.takes, &samples, at).unwrap();
        assert_eq!(
            first.file_name().unwrap().to_string_lossy(),
            "20261109T163405123Z.flac"
        );
        assert_eq!(
            second.file_name().unwrap().to_string_lossy(),
            "20261109T163405123Z-1.flac",
            "a second take in the same millisecond gets its own name"
        );
        let dir_mode = std::fs::metadata(&f.takes).unwrap().permissions().mode();
        assert_eq!(dir_mode & 0o777, 0o700);
        let file_mode = std::fs::metadata(&first).unwrap().permissions().mode();
        assert_eq!(file_mode & 0o777, 0o600);

        File::options()
            .write(true)
            .open(&second)
            .unwrap()
            .set_modified(SystemTime::now() - DAY)
            .unwrap();
        std::fs::write(f.takes.join("notes.txt"), b"x").unwrap();
        let listed: Vec<PathBuf> = list(&f.takes)
            .unwrap()
            .into_iter()
            .map(|k| k.path)
            .collect();
        assert_eq!(listed, [second, first], "only FLAC, oldest first");
    }

    #[test]
    fn a_failed_part_moves_to_the_recovery_folder() {
        let f = folders();
        let part = save(&f.takes, &[0.1; 160], Utc::now()).unwrap();
        let moved = move_to_recovery(&part, &f.recovery).unwrap();
        assert!(!part.exists());
        assert_eq!(list(&f.recovery).unwrap()[0].path, moved);
        let mode = std::fs::metadata(&f.recovery).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
    }

    #[test]
    fn parts_that_a_stopped_daemon_left_move_to_recovery() {
        let f = folders();
        let parts = f.takes.with_file_name("parts");
        assert_eq!(recover_leftover_parts(&parts, &f.recovery).unwrap(), 0);
        save_as(&parts, "take-part01", &[0.1; 160]).unwrap();
        save_as(&parts, "take-part02", &[0.1; 160]).unwrap();
        assert_eq!(recover_leftover_parts(&parts, &f.recovery).unwrap(), 2);
        assert!(list(&parts).unwrap().is_empty());
        assert_eq!(names(&f.recovery), ["take-part01.flac", "take-part02.flac"]);
    }

    #[test]
    fn deleting_a_take_touches_only_that_file() {
        let f = folders();
        let now = SystemTime::now();
        plant(&f.takes, "a.flac", 1, now, 0.0);
        plant(&f.takes, "b.flac", 1, now, 0.0);
        delete_take(&f.takes, "a.flac").unwrap();
        delete_take(&f.takes, "a.flac").unwrap();
        delete_take(&f.takes, "../recover/c.flac").unwrap();
        assert_eq!(names(&f.takes), ["b.flac"]);
        clear(&f.takes).unwrap();
        assert!(names(&f.takes).is_empty());
    }
}
