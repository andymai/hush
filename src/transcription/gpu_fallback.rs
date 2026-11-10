//! When the speech model uses the CPU in a GPU build.
//!
//! At login, the graphics driver can start after Hush. ggml looks for the
//! GPU only once in each process, so in the first minutes after boot, the
//! first model load waits a short time for a render node. When the GPU fails
//! a take, Hush writes the GPU and its driver under the model family in a
//! file in the state folder. Later starts of that family use the CPU on that
//! GPU until the driver changes.

use crate::config::paths;
use crate::transcription::device::{self, GpuAvailability};
use crate::transcription::whisper::Family;
use crate::Result;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// Whether this binary has a GPU backend.
pub const GPU_BUILD: bool = cfg!(any(feature = "cuda", feature = "vulkan"));

const RENDER_NODES: &str = "/dev/dri";
const RENDER_NODE_WAIT: Duration = Duration::from_secs(5);
const RENDER_NODE_POLL: Duration = Duration::from_millis(250);
/// The graphics driver can start after Hush only this soon after boot.
const BOOT_WINDOW: Duration = Duration::from_secs(180);

const UPTIME: &str = "/proc/uptime";
const NVIDIA_VERSION: &str = "/sys/module/nvidia/version";
const KERNEL_RELEASE: &str = "/proc/sys/kernel/osrelease";

static WHISPER_FAILED: AtomicBool = AtomicBool::new(false);
static PARAKEET_FAILED: AtomicBool = AtomicBool::new(false);

/// Whether `family` failed a take on the GPU in this session. A failure of
/// one family says nothing about the GPU memory or kernels of the other.
fn failed_this_session(family: Family) -> &'static AtomicBool {
    match family {
        Family::Whisper => &WHISPER_FAILED,
        Family::Parakeet => &PARAKEET_FAILED,
    }
}

/// Whether the model load waits for a render node. ggml looks for the GPU
/// only once, when the backend modules load, so a wait after that load
/// cannot help. A machine that booted a while ago, or with an unknown uptime,
/// does not wait: a VM or server without a render node would wait at each
/// start.
pub fn should_wait(
    gpu_build: bool,
    use_gpu: bool,
    loaded: bool,
    render_node: bool,
    uptime: Option<Duration>,
) -> bool {
    gpu_build
        && use_gpu
        && !loaded
        && !render_node
        && uptime.is_some_and(|uptime| uptime < BOOT_WINDOW)
}

/// The first field of `/proc/uptime`: seconds since boot.
fn parse_uptime(text: &str) -> Option<Duration> {
    let seconds: f64 = text.split_whitespace().next()?.parse().ok()?;
    Duration::try_from_secs_f64(seconds).ok()
}

fn uptime() -> Option<Duration> {
    parse_uptime(&std::fs::read_to_string(UPTIME).ok()?)
}

/// Wait up to 5 seconds for a render node, when [`should_wait`] says so.
/// [`device::load_backends`] calls this before the backend modules load.
pub async fn wait_for_gpu(use_gpu: bool) {
    let dir = Path::new(RENDER_NODES);
    if !should_wait(
        GPU_BUILD,
        use_gpu,
        device::backends_loaded(),
        has_render_node(dir),
        uptime(),
    ) {
        return;
    }
    info!(
        "No GPU render node is in {} yet, so Hush waits up to {} s for the graphics driver",
        dir.display(),
        RENDER_NODE_WAIT.as_secs()
    );
    match wait_for_render_node(dir, RENDER_NODE_WAIT, RENDER_NODE_POLL).await {
        Some(waited) => info!(
            waited_ms = waited.as_millis() as u64,
            "A GPU render node appeared after {} ms",
            waited.as_millis()
        ),
        None => warn!(
            "No GPU render node appeared in {} s",
            RENDER_NODE_WAIT.as_secs()
        ),
    }
}

/// The time until a render node was in `dir`, or `None` when none came
/// within `limit`.
async fn wait_for_render_node(dir: &Path, limit: Duration, poll: Duration) -> Option<Duration> {
    let started = Instant::now();
    loop {
        if has_render_node(dir) {
            return Some(started.elapsed());
        }
        if started.elapsed() >= limit {
            return None;
        }
        tokio::time::sleep(poll).await;
    }
}

fn has_render_node(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries
            .flatten()
            .any(|entry| entry.file_name().to_string_lossy().starts_with("renderD"))
    })
}

/// A GPU that failed a take, the driver that ran it, and the model family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuFailure {
    /// ggml's description of the GPU
    pub gpu: String,
    /// The ggml backend: CUDA or Vulkan
    pub backend: String,
    /// The NVIDIA driver version, or the kernel release for other GPUs
    pub driver: String,
    /// The model family that failed: Whisper or Parakeet. A record without
    /// it is a Whisper failure: a Hush that writes no family runs only Whisper.
    #[serde(default = "whisper_name")]
    pub family: String,
}

fn whisper_name() -> String {
    Family::Whisper.name().to_string()
}

impl GpuFailure {
    /// The GPU that ggml found, with the driver that runs it now, for a
    /// model of `family`.
    pub fn current(gpu: &GpuAvailability, family: Family) -> Self {
        let read = |path: &str| std::fs::read_to_string(path).ok();
        Self {
            gpu: gpu.device_name.clone(),
            backend: gpu.gpu_type.to_string(),
            driver: driver_label(
                &gpu.device_name,
                read(NVIDIA_VERSION).as_deref(),
                read(KERNEL_RELEASE).as_deref(),
            ),
            family: family.name().to_string(),
        }
    }
}

/// The remembered GPU failure of each model family. A failure of one family
/// never removes the record of the other.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GpuFailures(BTreeMap<String, GpuFailure>);

/// The file holds a record for each family by name. A file that holds one
/// bare record is a Whisper failure from a Hush that runs only Whisper.
#[derive(Deserialize)]
#[serde(untagged)]
enum FailureFile {
    Families(BTreeMap<String, GpuFailure>),
    One(GpuFailure),
}

impl GpuFailures {
    /// `$XDG_STATE_HOME/hush/gpu-failure.json`
    pub fn path() -> PathBuf {
        paths::state_dir().join("gpu-failure.json")
    }

    /// The saved records, or none when the file is missing or unreadable.
    pub fn load_from(path: &Path) -> Self {
        let Some(file) = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
        else {
            return Self::default();
        };
        match file {
            FailureFile::Families(records) => Self(
                records
                    .into_iter()
                    .map(|(family, record)| {
                        let record = GpuFailure {
                            family: family.clone(),
                            ..record
                        };
                        (family, record)
                    })
                    .collect(),
            ),
            FailureFile::One(record) => Self(BTreeMap::from([(record.family.clone(), record)])),
        }
    }

    /// Write the records, or delete the file when none is left.
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if self.0.is_empty() {
            return match std::fs::remove_file(path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    Err(e).with_context(|| format!("Failed to delete {}", path.display()))
                },
                _ => Ok(()),
            };
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        let text = serde_json::to_string_pretty(&self.0)?;
        std::fs::write(path, text).with_context(|| format!("Failed to write {}", path.display()))
    }

    pub fn get(&self, family: Family) -> Option<&GpuFailure> {
        self.0.get(family.name())
    }

    /// Keep `record` in place of the record of its family.
    pub fn insert(&mut self, record: GpuFailure) {
        self.0.insert(record.family.clone(), record);
    }

    pub fn remove(&mut self, family: Family) -> Option<GpuFailure> {
        self.0.remove(family.name())
    }

    pub fn iter(&self) -> impl Iterator<Item = &GpuFailure> {
        self.0.values()
    }
}

/// The NVIDIA module version for an NVIDIA GPU. Other GPUs get the kernel
/// release: it is cheap to read, and their kernel drivers change with it.
fn driver_label(gpu: &str, nvidia: Option<&str>, kernel: Option<&str>) -> String {
    let nvidia = nvidia.map(str::trim).filter(|v| !v.is_empty());
    let kernel = kernel.map(str::trim).filter(|v| !v.is_empty());
    match (nvidia, kernel) {
        (Some(version), _) if gpu.to_ascii_lowercase().contains("nvidia") => {
            format!("NVIDIA {}", version)
        },
        (_, Some(release)) => format!("Linux {}", release),
        _ => "unknown".to_string(),
    }
}

/// What a remembered GPU failure means for a model load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// No failure of this model family is remembered: use the GPU.
    Gpu,
    /// This GPU failed with this driver: use the CPU.
    Cpu,
    /// The record names another GPU or driver: delete the record of this
    /// family and use the GPU.
    Forget,
}

/// Whether to use the CPU on `current`, from the `record` of a failure. A
/// record of the other model family stays for that family.
pub fn choose(record: Option<&GpuFailure>, current: &GpuFailure) -> Choice {
    match record {
        None => Choice::Gpu,
        Some(record) if record.family != current.family => Choice::Gpu,
        Some(record) if record == current => Choice::Cpu,
        Some(_) => Choice::Forget,
    }
}

/// Whether this load of a `family` model uses the CPU on the GPU that ggml
/// found, because the GPU failed a take of that family in this session or
/// with this driver before.
pub fn avoid_gpu(gpu: &GpuAvailability, family: Family) -> bool {
    if failed_this_session(family).load(Ordering::Acquire) {
        info!(
            "{} stays on the CPU, because the GPU failed a {} take in this session",
            family.name(),
            family.name()
        );
        return true;
    }
    let path = GpuFailures::path();
    let mut records = GpuFailures::load_from(&path);
    let current = GpuFailure::current(gpu, family);
    match choose(records.get(family), &current) {
        Choice::Gpu => false,
        Choice::Cpu => {
            warn!(
                "{} loads on the CPU, because {} failed a {} take with driver {}. \
                 To try the GPU again, delete {} and run `hush daemon restart`",
                current.family,
                current.gpu,
                current.family,
                current.driver,
                path.display()
            );
            true
        },
        Choice::Forget => {
            info!(
                "The GPU or its driver changed after a GPU failure, so Hush tries {} on {} with driver {} again",
                current.family, current.gpu, current.driver
            );
            records.remove(family);
            if let Err(e) = records.save_to(&path) {
                warn!("Could not update {}: {:#}", path.display(), e);
            }
            false
        },
    }
}

/// Keep `family` on the CPU for the rest of this session, and write nothing.
pub fn keep_on_cpu(family: Family) {
    failed_this_session(family).store(true, Ordering::Release);
}

/// Keep `family` on the CPU for this session, and write the GPU and its
/// driver under the family to the state folder for later starts.
pub fn remember(gpu: &GpuAvailability, family: Family) {
    keep_on_cpu(family);
    let path = GpuFailures::path();
    let mut records = GpuFailures::load_from(&path);
    records.insert(GpuFailure::current(gpu, family));
    match records.save_to(&path) {
        Ok(()) => info!(
            "Wrote the GPU failure to {}. Later starts of {} use the CPU until the driver changes",
            path.display(),
            family.name()
        ),
        Err(e) => warn!("Could not write the GPU failure: {:#}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failure(gpu: &str, backend: &str, driver: &str) -> GpuFailure {
        GpuFailure {
            gpu: gpu.to_string(),
            backend: backend.to_string(),
            driver: driver.to_string(),
            family: "Whisper".to_string(),
        }
    }

    const JUST_BOOTED: Option<Duration> = Some(Duration::from_secs(20));

    #[test]
    fn only_a_gpu_build_without_a_render_node_waits() {
        assert!(should_wait(true, true, false, false, JUST_BOOTED));
        assert!(
            !should_wait(true, true, false, true, JUST_BOOTED),
            "the node is there"
        );
        assert!(
            !should_wait(true, true, true, false, JUST_BOOTED),
            "the backend modules loaded already"
        );
        assert!(
            !should_wait(true, false, false, false, JUST_BOOTED),
            "use_gpu is off"
        );
        assert!(
            !should_wait(false, true, false, false, JUST_BOOTED),
            "a CPU-only build"
        );
    }

    #[test]
    fn only_a_machine_that_booted_less_than_180_s_ago_waits() {
        let wait = |uptime| should_wait(true, true, false, false, uptime);
        assert!(wait(Some(Duration::ZERO)));
        assert!(wait(Some(Duration::from_millis(179_999))));
        assert!(!wait(Some(Duration::from_secs(180))), "the limit");
        assert!(!wait(Some(Duration::from_secs(86_400))), "a VM or server");
        assert!(!wait(None), "an unknown uptime");
    }

    #[test]
    fn the_uptime_is_the_first_field_of_proc_uptime() {
        assert_eq!(
            parse_uptime("42.57 160.12\n"),
            Some(Duration::from_secs_f64(42.57))
        );
        assert_eq!(parse_uptime("12345 1\n"), Some(Duration::from_secs(12_345)));
        assert_eq!(parse_uptime(""), None);
        assert_eq!(parse_uptime("soon 1"), None);
        assert_eq!(parse_uptime("-5 1"), None);
    }

    #[test]
    fn the_same_gpu_and_driver_use_the_cpu() {
        let rtx = failure("NVIDIA GeForce RTX 4080", "Vulkan", "NVIDIA 580.95.05");
        assert_eq!(choose(None, &rtx), Choice::Gpu);
        assert_eq!(choose(Some(&rtx), &rtx), Choice::Cpu);
    }

    #[test]
    fn a_new_driver_gpu_or_backend_forgets_the_failure() {
        let rtx = failure("NVIDIA GeForce RTX 4080", "Vulkan", "NVIDIA 580.95.05");
        let new_driver = failure("NVIDIA GeForce RTX 4080", "Vulkan", "NVIDIA 580.105.08");
        let new_gpu = failure("NVIDIA GeForce RTX 5090", "Vulkan", "NVIDIA 580.95.05");
        let new_backend = failure("NVIDIA GeForce RTX 4080", "CUDA", "NVIDIA 580.95.05");
        assert_eq!(choose(Some(&rtx), &new_driver), Choice::Forget);
        assert_eq!(choose(Some(&rtx), &new_gpu), Choice::Forget);
        assert_eq!(choose(Some(&rtx), &new_backend), Choice::Forget);
    }

    #[test]
    fn a_failure_of_one_family_leaves_the_other_on_the_gpu() {
        let whisper = failure("NVIDIA GeForce RTX 4080", "Vulkan", "NVIDIA 580.95.05");
        let parakeet = GpuFailure {
            family: "Parakeet".to_string(),
            ..whisper.clone()
        };
        assert_eq!(choose(Some(&parakeet), &whisper), Choice::Gpu);
        assert_eq!(choose(Some(&whisper), &parakeet), Choice::Gpu);
        assert_eq!(choose(Some(&parakeet), &parakeet), Choice::Cpu);
        let new_driver = GpuFailure {
            driver: "NVIDIA 580.105.08".to_string(),
            ..parakeet.clone()
        };
        assert_eq!(choose(Some(&parakeet), &new_driver), Choice::Forget);
    }

    #[test]
    fn a_record_without_a_family_is_a_whisper_failure() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gpu-failure.json");
        std::fs::write(
            &path,
            r#"{"gpu": "NVIDIA GeForce RTX 4080", "backend": "Vulkan", "driver": "NVIDIA 580.95.05"}"#,
        )
        .unwrap();
        let records = GpuFailures::load_from(&path);
        assert_eq!(
            records.get(Family::Whisper),
            Some(&failure(
                "NVIDIA GeForce RTX 4080",
                "Vulkan",
                "NVIDIA 580.95.05"
            ))
        );
        assert_eq!(records.get(Family::Parakeet), None);
    }

    #[test]
    fn keeping_one_family_on_the_cpu_leaves_the_other_on_the_gpu() {
        keep_on_cpu(Family::Parakeet);
        assert!(failed_this_session(Family::Parakeet).load(Ordering::Acquire));
        assert!(!failed_this_session(Family::Whisper).load(Ordering::Acquire));
    }

    #[test]
    fn each_family_keeps_its_own_record() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state/gpu-failure.json");
        let whisper = failure("NVIDIA GeForce RTX 4080", "Vulkan", "NVIDIA 580.95.05");
        let parakeet = GpuFailure {
            family: "Parakeet".to_string(),
            ..whisper.clone()
        };
        let mut records = GpuFailures::load_from(&path);
        records.insert(whisper.clone());
        records.save_to(&path).unwrap();
        let mut records = GpuFailures::load_from(&path);
        records.insert(parakeet.clone());
        records.save_to(&path).unwrap();

        let mut records = GpuFailures::load_from(&path);
        assert_eq!(records.get(Family::Whisper), Some(&whisper));
        assert_eq!(records.get(Family::Parakeet), Some(&parakeet));

        records.remove(Family::Parakeet);
        records.save_to(&path).unwrap();
        let mut records = GpuFailures::load_from(&path);
        assert_eq!(
            records.get(Family::Whisper),
            Some(&whisper),
            "forgetting Parakeet keeps the Whisper record"
        );
        assert_eq!(records.get(Family::Parakeet), None);

        records.remove(Family::Whisper);
        records.save_to(&path).unwrap();
        assert!(!path.exists(), "no record left, no file");
    }

    #[test]
    fn an_nvidia_gpu_names_the_module_version_and_others_the_kernel() {
        let kernel = Some("6.17.7-300.fc43.x86_64\n");
        assert_eq!(
            driver_label("NVIDIA GeForce RTX 4080", Some("580.95.05\n"), kernel),
            "NVIDIA 580.95.05"
        );
        assert_eq!(
            driver_label(
                "AMD Radeon RX 7900 XTX (RADV NAVI31)",
                Some("580.95.05"),
                kernel
            ),
            "Linux 6.17.7-300.fc43.x86_64",
            "a second GPU from NVIDIA does not drive this one"
        );
        assert_eq!(
            driver_label("NVIDIA GeForce GTX 1060 (NVK)", None, kernel),
            "Linux 6.17.7-300.fc43.x86_64"
        );
        assert_eq!(driver_label("Intel Arc A770", Some(""), None), "unknown");
    }

    #[test]
    fn a_record_survives_a_save_and_a_bad_file_is_no_record() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state/gpu-failure.json");
        assert_eq!(GpuFailures::load_from(&path), GpuFailures::default());

        let rtx = failure("NVIDIA GeForce RTX 4080", "CUDA", "NVIDIA 580.95.05");
        let mut records = GpuFailures::default();
        records.insert(rtx.clone());
        records.save_to(&path).unwrap();
        assert_eq!(
            GpuFailures::load_from(&path).get(Family::Whisper),
            Some(&rtx)
        );

        std::fs::write(&path, "not json").unwrap();
        assert_eq!(GpuFailures::load_from(&path), GpuFailures::default());
    }

    #[test]
    fn only_a_render_node_counts() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!has_render_node(&dir.path().join("missing")));
        std::fs::write(dir.path().join("card0"), "").unwrap();
        assert!(!has_render_node(dir.path()));
        std::fs::write(dir.path().join("renderD128"), "").unwrap();
        assert!(has_render_node(dir.path()));
    }

    #[tokio::test]
    async fn the_wait_ends_when_a_render_node_appears_or_at_the_limit() {
        let dir = tempfile::tempdir().unwrap();
        let limit = Duration::from_millis(60);
        let poll = Duration::from_millis(5);
        assert_eq!(wait_for_render_node(dir.path(), limit, poll).await, None);

        let node = dir.path().join("renderD128");
        let late = tokio::spawn({
            let node = node.clone();
            async move {
                tokio::time::sleep(Duration::from_millis(20)).await;
                std::fs::write(node, "").unwrap();
            }
        });
        let waited = wait_for_render_node(dir.path(), Duration::from_secs(5), poll).await;
        late.await.unwrap();
        assert!(waited.is_some_and(|waited| waited < Duration::from_secs(5)));

        assert!(
            wait_for_render_node(dir.path(), Duration::ZERO, poll)
                .await
                .is_some(),
            "a node that is there needs no wait"
        );
    }
}
