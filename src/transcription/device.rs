//! GPU detection through ggml's backend registry.
//!
//! whisper.cpp registers every backend it was built with (CPU, CUDA, Vulkan)
//! in a static registry, so the devices it will actually use can be listed
//! without loading a model.

use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::OnceLock;
use tracing::info;
use whisper_rs::whisper_rs_sys as sys;

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

/// One device known to ggml's backend registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendDevice {
    /// Backend device name, such as `CUDA0`, `Vulkan0`, or `CPU`
    pub name: String,
    /// Human-readable description, such as the GPU or CPU model
    pub description: String,
    /// Whether ggml classifies the device as a GPU
    pub is_gpu: bool,
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
                        .find(|d| !d.is_gpu)
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

    /// whisper.cpp's own summary of the compiled backends and CPU features
    pub fn system_info() -> String {
        // SAFETY: whisper_print_system_info returns a pointer to a static
        // buffer owned by whisper.cpp; it is never null and is not freed.
        unsafe { cstr_to_string(sys::whisper_print_system_info()) }
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

/// Every device registered with ggml, in registry order.
pub fn backend_devices() -> Vec<BackendDevice> {
    // SAFETY: the registry is populated by static initializers in the linked
    // backends; every index below `ggml_backend_dev_count()` yields a live
    // device whose name and description strings outlive the process.
    unsafe {
        let count = sys::ggml_backend_dev_count();
        (0..count)
            .filter_map(|index| {
                let device = sys::ggml_backend_dev_get(index);
                if device.is_null() {
                    return None;
                }
                Some(BackendDevice {
                    name: cstr_to_string(sys::ggml_backend_dev_name(device)),
                    description: cstr_to_string(sys::ggml_backend_dev_description(device)),
                    is_gpu: sys::ggml_backend_dev_type(device)
                        == sys::ggml_backend_dev_type_GGML_BACKEND_DEVICE_TYPE_GPU,
                })
            })
            .collect()
    }
}

unsafe fn cstr_to_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        CStr::from_ptr(ptr).to_string_lossy().into_owned()
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
        assert!(devices.iter().any(|d| !d.is_gpu), "{devices:?}");
        assert!(devices.iter().all(|d| !d.name.is_empty()));
    }

    #[test]
    fn gpu_type_follows_the_backend_name() {
        assert_eq!(gpu_type_for("CUDA0"), GpuType::Cuda);
        assert_eq!(gpu_type_for("Vulkan1"), GpuType::Vulkan);
        assert_eq!(gpu_type_for("CPU"), GpuType::Cpu);
    }

    #[cfg(not(any(feature = "cuda", feature = "vulkan")))]
    #[test]
    fn cpu_only_build_reports_no_gpu() {
        let gpu = GpuAvailability::detect();
        assert!(!gpu.available);
        assert_eq!(gpu.gpu_type, GpuType::Cpu);
        assert!(!gpu.device_name.is_empty());
    }

    #[test]
    fn system_info_mentions_the_cpu_features() {
        assert!(!GpuAvailability::system_info().is_empty());
    }
}
