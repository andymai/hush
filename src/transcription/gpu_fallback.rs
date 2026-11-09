//! When Whisper uses the CPU in a GPU build.
//!
//! At login, the graphics driver can start after Hush. ggml looks for the
//! GPU only once in each process, so in the first minutes after boot, the
//! first model load waits a short time for a render node. When the GPU fails
//! a take, Hush writes the GPU and its driver to a file in the state folder.
//! Later starts use the CPU on that GPU until the driver changes.

use crate::config::paths;
use crate::transcription::device::GpuAvailability;
use crate::Result;
use anyhow::Context;
use serde::{Deserialize, Serialize};
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

static FAILED_THIS_SESSION: AtomicBool = AtomicBool::new(false);

/// Whether the model load waits for a render node. ggml looks for the GPU
/// only once, so a wait after that look cannot help. A machine that booted
/// a while ago, or with an unknown uptime, does not wait: a VM or server
/// without a render node would wait at each start.
pub fn should_wait(
    gpu_build: bool,
    use_gpu: bool,
    detected: bool,
    render_node: bool,
    uptime: Option<Duration>,
) -> bool {
    gpu_build
        && use_gpu
        && !detected
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
/// Call this before the first [`GpuAvailability::detect`].
pub async fn wait_for_gpu(use_gpu: bool) {
    let dir = Path::new(RENDER_NODES);
    if !should_wait(
        GPU_BUILD,
        use_gpu,
        GpuAvailability::is_detected(),
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

/// A GPU that failed a take, and the driver that ran it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuFailure {
    /// ggml's description of the GPU
    pub gpu: String,
    /// The ggml backend: CUDA or Vulkan
    pub backend: String,
    /// The NVIDIA driver version, or the kernel release for other GPUs
    pub driver: String,
}

impl GpuFailure {
    /// `$XDG_STATE_HOME/hush/gpu-failure.json`
    pub fn path() -> PathBuf {
        paths::state_dir().join("gpu-failure.json")
    }

    /// The GPU that ggml found, with the driver that runs it now.
    pub fn current(gpu: &GpuAvailability) -> Self {
        let read = |path: &str| std::fs::read_to_string(path).ok();
        Self {
            gpu: gpu.device_name.clone(),
            backend: gpu.gpu_type.to_string(),
            driver: driver_label(
                &gpu.device_name,
                read(NVIDIA_VERSION).as_deref(),
                read(KERNEL_RELEASE).as_deref(),
            ),
        }
    }

    /// The saved record, or `None` when the file is missing or unreadable.
    pub fn load_from(path: &Path) -> Option<Self> {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(path, text).with_context(|| format!("Failed to write {}", path.display()))
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
    /// No failure is remembered: use the GPU.
    Gpu,
    /// This GPU failed with this driver: use the CPU.
    Cpu,
    /// The record names another GPU or driver: delete it and use the GPU.
    Forget,
}

/// Whether to use the CPU on `current`, from the `record` of a failure.
pub fn choose(record: Option<&GpuFailure>, current: &GpuFailure) -> Choice {
    match record {
        None => Choice::Gpu,
        Some(record) if record == current => Choice::Cpu,
        Some(_) => Choice::Forget,
    }
}

/// Whether this load uses the CPU on the GPU that ggml found, because the
/// GPU failed a take in this session or with this driver before.
pub fn avoid_gpu(gpu: &GpuAvailability) -> bool {
    if FAILED_THIS_SESSION.load(Ordering::Acquire) {
        info!("Whisper stays on the CPU, because the GPU failed a take in this session");
        return true;
    }
    let path = GpuFailure::path();
    let current = GpuFailure::current(gpu);
    match choose(GpuFailure::load_from(&path).as_ref(), &current) {
        Choice::Gpu => false,
        Choice::Cpu => {
            warn!(
                "Whisper loads on the CPU, because {} failed a take with driver {}. \
                 To try the GPU again, delete {} and run `hush daemon restart`",
                current.gpu,
                current.driver,
                path.display()
            );
            true
        },
        Choice::Forget => {
            info!(
                "The GPU or its driver changed after a GPU failure, so Hush tries {} with driver {} again",
                current.gpu, current.driver
            );
            if let Err(e) = std::fs::remove_file(&path) {
                warn!("Could not delete {}: {}", path.display(), e);
            }
            false
        },
    }
}

/// Keep the CPU for this session, and write the GPU and its driver to the
/// state folder for later starts.
pub fn remember(gpu: &GpuAvailability) {
    FAILED_THIS_SESSION.store(true, Ordering::Release);
    let path = GpuFailure::path();
    match GpuFailure::current(gpu).save_to(&path) {
        Ok(()) => info!(
            "Wrote the GPU failure to {}. Later starts use the CPU until the driver changes",
            path.display()
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
            "ggml looked already"
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
        assert_eq!(GpuFailure::load_from(&path), None);

        let rtx = failure("NVIDIA GeForce RTX 4080", "CUDA", "NVIDIA 580.95.05");
        rtx.save_to(&path).unwrap();
        assert_eq!(GpuFailure::load_from(&path), Some(rtx));

        std::fs::write(&path, "not json").unwrap();
        assert_eq!(GpuFailure::load_from(&path), None);
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
