//! A notice when the GPU that Hush used before is gone at start. One common
//! cause has a fix that Hush can name: after a host driver update, the
//! Flatpak can lack the GL extension of the new NVIDIA driver, and ggml in the
//! sandbox then finds no GPU.

use super::device::GpuAvailability;
use super::gpu_fallback::{GpuFingerprint, GPU_BUILD};
use crate::config::paths;
use crate::notify::{notify, NoticeKind};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

const PROC_NVIDIA_VERSION: &str = "/proc/driver/nvidia/version";

/// What the last start found: a GPU, or none.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Seen {
    gpu: Option<GpuFingerprint>,
}

/// How the GPU at this start compares with the last start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// No earlier start left a record
    First,
    Same,
    Gone(GpuFingerprint),
    /// A GPU where the last start had none, or another GPU or driver
    New(GpuFingerprint),
}

pub fn compare(before: Option<&Option<GpuFingerprint>>, now: Option<&GpuFingerprint>) -> Change {
    match (before, now) {
        (None, _) => Change::First,
        (Some(Some(before)), None) => Change::Gone(before.clone()),
        (Some(before), Some(now)) if before.as_ref() != Some(now) => Change::New(now.clone()),
        _ => Change::Same,
    }
}

/// The GPU at this start, kept for the next start. Notifies when the GPU of
/// the last start is gone, or when a Flatpak lacks the GL extension of the
/// host's NVIDIA driver.
pub fn check_at_start(gpu: &GpuAvailability) {
    if !GPU_BUILD {
        return;
    }
    let now = gpu.available.then(|| GpuFingerprint::current(gpu));
    let path = seen_path();
    let before = load(&path);
    let change = compare(before.as_ref().map(|seen| &seen.gpu), now.as_ref());
    if let Err(e) = save(&path, &Seen { gpu: now.clone() }) {
        debug!("Could not keep the GPU for the next start: {:#}", e);
    }
    let missing = missing_flatpak_gl();
    let notice = match (&change, now.is_none()) {
        (Change::Gone(before), _) => Some(gone_notice(Some(before), missing.as_deref())),
        (Change::First, true) if missing.is_some() => Some(gone_notice(None, missing.as_deref())),
        _ => None,
    };
    if let Some((title, body)) = notice {
        warn!("{}: {}", title, body.replace('\n', " "));
        notify(NoticeKind::Problem, title, &body);
    }
}

/// The notice for a GPU that is gone; `missing` names the Flatpak GL
/// extension that the host driver needs.
pub fn gone_notice(
    before: Option<&GpuFingerprint>,
    missing: Option<&str>,
) -> (&'static str, String) {
    let title = "Hush runs the speech model on the CPU";
    let body = match (missing, before) {
        (Some(extension), _) => format!(
            "The Flatpak lacks the graphics driver of the host's NVIDIA driver. Install it, then \
             restart Hush:\nflatpak install flathub {}",
            extension
        ),
        (None, Some(before)) => format!(
            "Hush found no GPU at start, though it used {} before. If the graphics driver was not \
             ready yet, run: hush daemon restart",
            before.gpu
        ),
        (None, None) => {
            "Hush found no GPU at start. If the graphics driver was not ready yet, run: hush \
             daemon restart"
                .to_string()
        },
    };
    (title, body)
}

/// In a Flatpak on a host with an NVIDIA driver: the GL extension that the
/// driver needs, when the sandbox lacks it.
pub fn missing_flatpak_gl() -> Option<String> {
    #[cfg(target_os = "linux")]
    if !crate::portal::sandboxed() {
        return None;
    }
    let version = nvidia_version(&std::fs::read_to_string(PROC_NVIDIA_VERSION).ok()?)?;
    let gl = PathBuf::from(format!("/usr/lib/{}-linux-gnu/GL", std::env::consts::ARCH));
    let mounted: Vec<String> = std::fs::read_dir(&gl)
        .ok()?
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .collect();
    missing_extension(&version, &mounted)
}

/// `620.73.04` from `NVRM version: NVIDIA UNIX Open Kernel Module for x86_64
/// 620.73.04  Release Build ...`.
fn nvidia_version(proc_text: &str) -> Option<String> {
    proc_text
        .lines()
        .next()?
        .split_whitespace()
        .find(|word| word.contains('.') && word.chars().all(|c| c.is_ascii_digit() || c == '.'))
        .map(str::to_string)
}

/// Flathub names the extension after the driver version, with dashes.
fn missing_extension(version: &str, mounted: &[String]) -> Option<String> {
    let name = format!("nvidia-{}", version.replace('.', "-"));
    (!mounted.contains(&name)).then(|| format!("org.freedesktop.Platform.GL.{}", name))
}

fn seen_path() -> PathBuf {
    paths::state_dir().join("gpu.json")
}

fn load(path: &Path) -> Option<Seen> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn save(path: &Path, seen: &Seen) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string(seen)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rtx(driver: &str) -> GpuFingerprint {
        GpuFingerprint {
            gpu: "NVIDIA GeForce RTX 4080 SUPER".into(),
            backend: "Vulkan".into(),
            driver: driver.into(),
        }
    }

    #[test]
    fn a_start_compares_with_the_last_one() {
        let gpu = rtx("NVIDIA 620.70.06");
        assert_eq!(compare(None, Some(&gpu)), Change::First);
        assert_eq!(compare(Some(&Some(gpu.clone())), Some(&gpu)), Change::Same);
        assert_eq!(
            compare(Some(&Some(gpu.clone())), None),
            Change::Gone(gpu.clone())
        );
        assert_eq!(compare(Some(&None), None), Change::Same);
        assert_eq!(compare(Some(&None), Some(&gpu)), Change::New(gpu.clone()));
        let updated = rtx("NVIDIA 620.73.04");
        assert_eq!(
            compare(Some(&Some(gpu)), Some(&updated)),
            Change::New(updated)
        );
    }

    #[test]
    fn the_driver_version_comes_from_proc() {
        let text = "NVRM version: NVIDIA UNIX Open Kernel Module for x86_64  620.73.04  \
                    Release Build  (akmods@)  Tue Nov 24 00:39:33 UTC 2026\nGCC version: 16.2.1\n";
        assert_eq!(nvidia_version(text).as_deref(), Some("620.73.04"));
        assert_eq!(nvidia_version("NVRM version: none\n"), None);
    }

    #[test]
    fn the_flatpak_lacks_the_extension_of_a_new_driver() {
        let mounted = vec!["default".to_string(), "nvidia-620-70-06".to_string()];
        assert_eq!(
            missing_extension("620.73.04", &mounted).as_deref(),
            Some("org.freedesktop.Platform.GL.nvidia-620-73-04")
        );
        assert_eq!(missing_extension("620.70.06", &mounted), None);
    }

    #[test]
    fn the_notice_names_the_fix() {
        let (_, flatpak) = gone_notice(
            Some(&rtx("NVIDIA 620.70.06")),
            Some("org.freedesktop.Platform.GL.nvidia-620-73-04"),
        );
        assert!(flatpak
            .ends_with("flatpak install flathub org.freedesktop.Platform.GL.nvidia-620-73-04"));
        let (_, late) = gone_notice(Some(&rtx("NVIDIA 620.70.06")), None);
        assert!(
            late.contains("NVIDIA GeForce RTX 4080 SUPER") && late.contains("hush daemon restart")
        );
    }

    #[test]
    fn the_record_survives_a_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state/gpu.json");
        assert_eq!(load(&path), None);
        let seen = Seen {
            gpu: Some(rtx("NVIDIA 620.73.04")),
        };
        save(&path, &seen).unwrap();
        assert_eq!(load(&path), Some(seen));
    }
}
