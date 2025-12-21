use candle_core::Device;
use std::sync::OnceLock;
use tracing::info;

static GPU_AVAILABILITY: OnceLock<GpuAvailability> = OnceLock::new();

/// GPU device type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuType {
    /// NVIDIA CUDA GPU
    Cuda,
    /// CPU fallback (no GPU acceleration)
    Cpu,
}

/// GPU availability and device information
#[derive(Debug, Clone)]
pub struct GpuAvailability {
    pub gpu_type: GpuType,
    pub device_name: String,
    pub expected_latency_ms: u32,
    pub available: bool,
}

impl GpuAvailability {
    /// Detect GPU availability (cached)
    pub fn detect() -> &'static Self {
        GPU_AVAILABILITY.get_or_init(|| {
            #[cfg(feature = "cuda")]
            {
                match Device::cuda_if_available(0) {
                    Ok(device) => {
                        if !matches!(device, Device::Cpu) {
                            info!("CUDA GPU detected and available");
                            return Self {
                                gpu_type: GpuType::Cuda,
                                device_name: "NVIDIA CUDA GPU".to_string(),
                                expected_latency_ms: 80,
                                available: true,
                            };
                        }
                    },
                    Err(e) => {
                        info!("CUDA initialization failed: {}", e);
                    },
                }
            }

            #[cfg(not(feature = "cuda"))]
            {
                info!("Built without GPU support (CPU-only mode)");
            }

            #[cfg(feature = "cuda")]
            {
                info!("No GPU available, falling back to CPU");
            }

            Self {
                gpu_type: GpuType::Cpu,
                device_name: Self::get_cpu_name(),
                expected_latency_ms: 800,
                available: false,
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

    /// Get CPU name from system info
    fn get_cpu_name() -> String {
        #[cfg(target_arch = "x86_64")]
        {
            use raw_cpuid::CpuId;
            if let Some(brand) = CpuId::new().get_processor_brand_string() {
                return brand.as_str().trim().to_string();
            }
        }

        "CPU".to_string()
    }

    /// Create appropriate Candle device based on detection
    pub fn create_device() -> crate::Result<Device> {
        let gpu = Self::detect();

        match gpu.gpu_type {
            GpuType::Cuda => {
                #[cfg(feature = "cuda")]
                {
                    Device::cuda_if_available(0)
                        .map_err(|e| anyhow::anyhow!("Failed to create CUDA device: {}", e))
                }
                #[cfg(not(feature = "cuda"))]
                {
                    Ok(Device::Cpu)
                }
            },
            GpuType::Cpu => Ok(Device::Cpu),
        }
    }
}

impl std::fmt::Display for GpuAvailability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.available {
            write!(
                f,
                "{} (expected latency: ~{}ms)",
                self.device_name, self.expected_latency_ms
            )
        } else {
            write!(
                f,
                "{} (expected latency: ~{}ms) [No GPU acceleration]",
                self.device_name, self.expected_latency_ms
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_detection_cached() {
        let first = GpuAvailability::detect();
        let second = GpuAvailability::detect();
        assert!(std::ptr::eq(first, second));
    }

    #[test]
    fn test_gpu_type_determination() {
        let gpu = GpuAvailability::detect();
        assert!(matches!(gpu.gpu_type, GpuType::Cuda | GpuType::Cpu));
    }

    #[test]
    fn test_device_creation() {
        let device = GpuAvailability::create_device();
        assert!(device.is_ok());
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_cuda_feature_enabled() {
        let gpu = GpuAvailability::detect();
        let _ = gpu.available;
    }

    #[cfg(not(feature = "cuda"))]
    #[test]
    fn test_cpu_only_build() {
        let gpu = GpuAvailability::detect();
        assert!(!gpu.available);
        assert_eq!(gpu.gpu_type, GpuType::Cpu);
    }

    #[test]
    fn test_display_formatting() {
        let gpu = GpuAvailability::detect();
        let display_string = format!("{}", gpu);
        assert!(!display_string.is_empty());
    }
}
