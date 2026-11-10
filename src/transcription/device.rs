//! The backend modules of transcribe.cpp and the devices that they find.
//!
//! libtranscribe loads each ggml backend as a module from its own folder: of
//! the CPU modules, the one that fits this CPU best, and the GPU module of the
//! build (CUDA or Vulkan). The devices that the modules find sit in a registry,
//! so the devices that a model will use can be listed without a model load.

use crate::transcription::gpu_fallback;
use crate::Result;
use std::path::{Path, PathBuf};
use std::sync::{Once, OnceLock};
use tracing::{info, warn};
use transcribe_cpp::{Backend, Device, DeviceType};

static GPU_AVAILABILITY: OnceLock<GpuAvailability> = OnceLock::new();

static BACKENDS: OnceLock<std::result::Result<(), String>> = OnceLock::new();

/// Wait for the GPU when [`gpu_fallback::wait_for_gpu`] says so, then load the
/// backend modules. Each module lists its devices when it loads, once in each
/// process, so a model load calls this, not [`init_backends`]: a render node
/// that comes up after the first load stays unseen until a restart.
pub async fn load_backends(use_gpu: bool) -> Result<()> {
    load_after(gpu_fallback::wait_for_gpu(use_gpu)).await
}

/// Run `wait` and then [`init_backends`], or only [`init_backends`] when the
/// modules loaded already.
async fn load_after(wait: impl std::future::Future<Output = ()>) -> Result<()> {
    if !backends_loaded() {
        wait.await;
    }
    init_backends()
}

/// Whether [`init_backends`] ran in this process.
pub fn backends_loaded() -> bool {
    BACKENDS.get().is_some()
}

/// Load the backend modules from the folder of libtranscribe, once in each
/// process. Call before any device query: without the modules, ggml has no
/// device, not even the CPU.
pub fn init_backends() -> Result<()> {
    BACKENDS
        .get_or_init(|| {
            route_engine_logs();
            transcribe_cpp::init_backends_default()
                .map(|()| info!("Backend modules: {}", loaded_modules()))
                .map_err(|e| no_modules_message(loaded_modules().dir.as_deref(), &e.to_string()))
        })
        .clone()
        .map_err(anyhow::Error::msg)
}

fn no_modules_message(dir: Option<&Path>, error: &str) -> String {
    format!(
        "No ggml backend module loaded from {} ({error}). The libggml-*.so modules belong \
         next to libtranscribe: reinstall Hush, or run `make release` again in the source tree",
        module_folder(dir)
    )
}

/// The folder of the modules, for a message.
pub fn module_folder(dir: Option<&Path>) -> String {
    dir.map_or_else(
        || "the folder of libtranscribe".to_string(),
        |dir| dir.display().to_string(),
    )
}

/// The transcribe.cpp files that this process loaded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Modules {
    /// The folder of libtranscribe, which holds the backend modules
    pub dir: Option<PathBuf>,
    /// The CPU module that ggml picked for this CPU, such as `zen4` or `haswell`
    pub cpu: Option<String>,
    /// The GPU backend modules that loaded, such as `Vulkan`
    pub gpu: Vec<String>,
}

impl std::fmt::Display for Modules {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CPU code {}", self.cpu.as_deref().unwrap_or("unknown"))?;
        match self.gpu.as_slice() {
            [] => write!(f, ", no GPU backend")?,
            gpu => write!(f, ", GPU backend {}", gpu.join(" and "))?,
        }
        if let Some(dir) = &self.dir {
            write!(f, " ({})", dir.display())?;
        }
        Ok(())
    }
}

/// The modules in the memory map of this process.
pub fn loaded_modules() -> Modules {
    std::fs::read_to_string("/proc/self/maps")
        .map(|maps| modules_in(&maps))
        .unwrap_or_default()
}

/// The modules in `maps`, a copy of `/proc/<pid>/maps`. ggml keeps only the
/// CPU module that it picked loaded.
fn modules_in(maps: &str) -> Modules {
    let mut modules = Modules::default();
    for path in maps
        .lines()
        .filter_map(|line| line.find('/').map(|start| Path::new(&line[start..])))
    {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.starts_with("libtranscribe.so") {
            modules.dir = path.parent().map(Path::to_path_buf);
            continue;
        }
        let Some(backend) = name
            .strip_prefix("libggml-")
            .and_then(|name| name.strip_suffix(".so"))
        else {
            continue;
        };
        match backend.strip_prefix("cpu") {
            Some(variant) => {
                let variant = variant.strip_prefix('-').unwrap_or("generic");
                modules.cpu = Some(variant.to_string());
            },
            None => {
                let backend = match backend {
                    "vulkan" => "Vulkan".to_string(),
                    "cuda" => "CUDA".to_string(),
                    other => other.to_string(),
                };
                if !modules.gpu.contains(&backend) {
                    modules.gpu.push(backend);
                }
            },
        }
    }
    modules
}

/// Accelerator backend in use
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuType {
    /// NVIDIA CUDA
    Cuda,
    /// Vulkan (NVIDIA, AMD, Intel)
    Vulkan,
    /// CPU fallback (no GPU acceleration)
    Cpu,
}

impl std::fmt::Display for GpuType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            GpuType::Cuda => "CUDA",
            GpuType::Vulkan => "Vulkan",
            GpuType::Cpu => "CPU",
        })
    }
}

/// One device known to the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendDevice {
    /// Backend device name, such as `CUDA0`, `Vulkan0`, or `CPU`
    pub name: String,
    /// Human-readable description, such as the GPU or CPU model
    pub description: String,
    /// Whether ggml classifies the device as a discrete GPU
    pub is_gpu: bool,
    /// Whether the device is the CPU, not an integrated GPU or accelerator
    pub is_cpu: bool,
}

/// GPU availability and device information
#[derive(Debug, Clone)]
pub struct GpuAvailability {
    pub gpu_type: GpuType,
    pub device_name: String,
    pub available: bool,
}

impl GpuAvailability {
    /// Detect GPU availability (cached)
    pub fn detect() -> &'static Self {
        GPU_AVAILABILITY.get_or_init(|| {
            let devices = backend_devices();
            match devices.iter().find(|d| d.is_gpu) {
                Some(gpu) => {
                    info!("GPU detected: {} ({})", gpu.description, gpu.name);
                    Self {
                        gpu_type: gpu_type_for(&gpu.name),
                        device_name: gpu.description.clone(),
                        available: true,
                    }
                },
                None => {
                    let cpu = devices
                        .iter()
                        .find(|d| d.is_cpu)
                        .map(|d| d.description.clone())
                        .unwrap_or_else(|| "CPU".to_string());
                    info!("No GPU backend available; using CPU ({})", cpu);
                    Self {
                        gpu_type: GpuType::Cpu,
                        device_name: cpu,
                        available: false,
                    }
                },
            }
        })
    }

    /// Check if any GPU is available
    pub fn is_gpu_available() -> bool {
        Self::detect().available
    }

    /// Get the GPU type being used
    pub fn gpu_type() -> GpuType {
        Self::detect().gpu_type
    }

    /// The speech engine, the backend modules that loaded, and every device
    /// that the engine can run on, one per line.
    pub fn system_info() -> String {
        let mut info = format!("transcribe.cpp {}", transcribe_cpp::version());
        if let Err(e) = init_backends() {
            info.push_str(&format!("\n{e}"));
            return info;
        }
        info.push_str(&format!("\nModules: {}", loaded_modules()));
        for device in backend_devices() {
            info.push_str(&format!("\n{}: {}", device.name, device.description));
        }
        info
    }
}

impl std::fmt::Display for GpuAvailability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.available {
            write!(f, "{} via {}", self.device_name, self.gpu_type)
        } else {
            write!(f, "{} [No GPU acceleration]", self.device_name)
        }
    }
}

fn gpu_type_for(backend_name: &str) -> GpuType {
    let name = backend_name.to_ascii_lowercase();
    if name.starts_with("cuda") {
        GpuType::Cuda
    } else if name.starts_with("vulkan") {
        GpuType::Vulkan
    } else {
        GpuType::Cpu
    }
}

/// Send the log lines of transcribe.cpp and ggml to tracing instead of
/// stderr. [`init_backends`] calls it first.
pub fn route_engine_logs() {
    static ROUTE: Once = Once::new();
    ROUTE.call_once(|| {
        // A subscriber that `init()` installed has a log bridge already.
        let _ = tracing_log::LogTracer::builder()
            .with_max_level(tracing_log::log::LevelFilter::Debug)
            .init();
        transcribe_cpp::init_logging();
    });
}

/// The registry's devices, or none when no backend module loaded.
fn registry() -> Vec<Device> {
    if let Err(e) = init_backends() {
        static SAID: Once = Once::new();
        SAID.call_once(|| warn!("{e}"));
        return Vec::new();
    }
    transcribe_cpp::devices()
}

/// Every device registered with ggml, in registry order.
pub fn backend_devices() -> Vec<BackendDevice> {
    registry()
        .into_iter()
        .map(|device| BackendDevice {
            is_gpu: device.device_type == DeviceType::Gpu,
            is_cpu: device.device_type == DeviceType::Cpu,
            name: device.name,
            description: device.description,
        })
        .collect()
}

/// The GPU that [`GpuAvailability::detect`] reports, with the backend
/// request that loads a model on it.
pub fn gpu_device() -> Option<(Backend, Device)> {
    registry()
        .into_iter()
        .find(|device| device.device_type == DeviceType::Gpu)
        .and_then(|device| backend_for(&device.kind).map(|backend| (backend, device)))
}

fn backend_for(kind: &str) -> Option<Backend> {
    match kind {
        "vulkan" => Some(Backend::Vulkan),
        "cuda" => Some(Backend::Cuda),
        "rocm" => Some(Backend::Rocm),
        "metal" => Some(Backend::Metal),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_is_cached() {
        let first = GpuAvailability::detect();
        let second = GpuAvailability::detect();
        assert!(std::ptr::eq(first, second));
    }

    #[test]
    fn registry_always_lists_the_cpu() {
        let devices = backend_devices();
        assert!(devices.iter().any(|d| d.is_cpu && !d.is_gpu), "{devices:?}");
        assert!(devices.iter().all(|d| !d.name.is_empty()));
    }

    #[test]
    fn gpu_type_follows_the_backend_name() {
        assert_eq!(gpu_type_for("CUDA0"), GpuType::Cuda);
        assert_eq!(gpu_type_for("Vulkan1"), GpuType::Vulkan);
        assert_eq!(gpu_type_for("CPU"), GpuType::Cpu);
    }

    #[test]
    fn a_gpu_kind_maps_to_its_backend_request() {
        assert_eq!(backend_for("vulkan"), Some(Backend::Vulkan));
        assert_eq!(backend_for("cuda"), Some(Backend::Cuda));
        assert_eq!(backend_for("cpu"), None);
        assert_eq!(backend_for("unknown"), None);
    }

    #[cfg(not(any(feature = "cuda", feature = "vulkan")))]
    #[test]
    fn cpu_only_build_reports_no_gpu() {
        let gpu = GpuAvailability::detect();
        assert!(!gpu.available);
        assert_eq!(gpu.gpu_type, GpuType::Cpu);
        assert!(!gpu.device_name.is_empty());
        assert!(gpu_device().is_none());
    }

    #[test]
    fn system_info_names_the_engine_the_modules_and_the_cpu() {
        let info = GpuAvailability::system_info();
        assert!(info.starts_with("transcribe.cpp "), "{info}");
        assert!(info.contains("\nModules: CPU code "), "{info}");
        assert!(info.contains("\nCPU: "), "{info}");
    }

    #[tokio::test]
    async fn the_gpu_wait_runs_before_the_first_backend_load() {
        use std::sync::atomic::{AtomicBool, Ordering};

        // nextest runs each test in its own process, so the modules are not
        // loaded yet; `cargo test` can load them in another test first.
        let loaded_before = backends_loaded();
        let waited = AtomicBool::new(false);
        load_after(async {
            assert!(!backends_loaded(), "the wait comes before the load");
            waited.store(true, Ordering::SeqCst);
        })
        .await
        .unwrap();
        assert_eq!(waited.load(Ordering::SeqCst), !loaded_before);
        assert!(backends_loaded());

        let again = AtomicBool::new(false);
        load_after(async { again.store(true, Ordering::SeqCst) })
            .await
            .unwrap();
        assert!(
            !again.load(Ordering::SeqCst),
            "after the first load, a wait cannot help"
        );
    }

    #[test]
    fn the_backends_load_from_the_folder_of_libtranscribe() {
        init_backends().unwrap();
        let modules = loaded_modules();
        assert!(modules.cpu.is_some(), "{modules:?}");
        let dir = modules.dir.unwrap();
        assert!(dir.join("libggml-base.so.0").is_file(), "{}", dir.display());
    }

    #[test]
    fn the_memory_map_names_the_cpu_variant_and_the_gpu_backend() {
        let maps = "\
7f0000000000-7f0000001000 r--p 00000000 00:2a 1 /usr/lib/hush/libtranscribe.so.0.4
7f0000001000-7f0000002000 r-xp 00001000 00:2a 1 /usr/lib/hush/libtranscribe.so.0.4
7f0000003000-7f0000004000 r-xp 00000000 00:2a 2 /usr/lib/hush/libggml-base.so.0
7f0000005000-7f0000006000 r-xp 00000000 00:2a 3 /usr/lib/hush/libggml-cpu-zen4.so
7f0000007000-7f0000008000 r-xp 00000000 00:2a 4 /usr/lib/hush/libggml-vulkan.so
7f0000009000-7f000000a000 r-xp 00000000 00:2a 4 /usr/lib/hush/libggml-vulkan.so
7f000000b000-7f000000c000 rw-p 00000000 00:00 0
7ffd00000000-7ffd00001000 rw-p 00000000 00:00 0 [stack]
";
        let modules = modules_in(maps);
        assert_eq!(modules.dir.as_deref(), Some(Path::new("/usr/lib/hush")));
        assert_eq!(modules.cpu.as_deref(), Some("zen4"));
        assert_eq!(modules.gpu, ["Vulkan"]);
        assert_eq!(
            modules.to_string(),
            "CPU code zen4, GPU backend Vulkan (/usr/lib/hush)"
        );

        let cpu_only = modules_in("7f0-7f1 r-xp 0 0:0 1 /opt/hush lib/libggml-cpu.so\n");
        assert_eq!(cpu_only.cpu.as_deref(), Some("generic"));
        assert_eq!(cpu_only.to_string(), "CPU code generic, no GPU backend");
        assert_eq!(modules_in(""), Modules::default());
    }

    #[test]
    fn missing_modules_name_their_folder_and_the_fix() {
        let message = no_modules_message(Some(Path::new("/usr/lib/hush")), "no device");
        assert!(
            message.contains("from /usr/lib/hush (no device)"),
            "{message}"
        );
        assert!(message.contains("reinstall Hush"), "{message}");
        assert!(no_modules_message(None, "x").contains("the folder of libtranscribe"));
    }
}
