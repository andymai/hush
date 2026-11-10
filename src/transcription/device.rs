//! GPU detection through the device registry of transcribe.cpp.
//!
//! transcribe.cpp registers every ggml backend it was built with (CPU, CUDA,
//! Vulkan) in a static registry, so the devices it will actually use can be
//! listed without loading a model.

use crate::transcription::cpu;
use std::sync::{Once, OnceLock};
use tracing::info;
use transcribe_cpp::{Backend, Device, DeviceType};

static GPU_AVAILABILITY: OnceLock<GpuAvailability> = OnceLock::new();

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

    /// Whether [`GpuAvailability::detect`] ran in this process. ggml looks
    /// for devices only once, so a GPU that comes up later stays unseen.
    pub fn is_detected() -> bool {
        GPU_AVAILABILITY.get().is_some()
    }

    /// Check if any GPU is available
    pub fn is_gpu_available() -> bool {
        Self::detect().available
    }

    /// Get the GPU type being used
    pub fn gpu_type() -> GpuType {
        Self::detect().gpu_type
    }

    /// The speech engine and every device it can run on, one per line.
    pub fn system_info() -> String {
        if let Err(e) = cpu::ensure_supported() {
            return e.to_string();
        }
        let mut info = format!("transcribe.cpp {}", transcribe_cpp::version());
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
/// stderr. Call before the first registry call or model load.
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

/// The registry's devices, or none when this CPU is below the binary's
/// floor. The first registry call registers the CPU backend, and that runs
/// ggml code built for the floor.
fn registry() -> Vec<Device> {
    if !cpu::supported() {
        return Vec::new();
    }
    route_engine_logs();
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
    fn system_info_names_the_engine_and_the_cpu() {
        let info = GpuAvailability::system_info();
        assert!(info.starts_with("transcribe.cpp "), "{info}");
        assert!(info.contains("\nCPU: "), "{info}");
    }
}
