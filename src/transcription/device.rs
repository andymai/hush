use candle_core::Device;
use std::sync::OnceLock;
use tracing::info;

static GPU_AVAILABILITY: OnceLock<GpuAvailability> = OnceLock::new();

/// GPU device type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuType {
    /// NVIDIA CUDA GPU
    Cuda,
    /// Apple Metal GPU (Apple Silicon or Intel Mac)
    Metal,
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
    ///
    /// Checks for CUDA and Metal in priority order, falls back to CPU.
    /// Result is cached for the lifetime of the application.
    ///
    /// # Priority Order
    ///
    /// 1. CUDA (if compiled with `cuda` feature and hardware available)
    /// 2. Metal (if compiled with `metal` feature and hardware available)
    /// 3. CPU (fallback)
    pub fn detect() -> &'static Self {
        GPU_AVAILABILITY.get_or_init(|| {
            // Try CUDA first (if available)
            #[cfg(feature = "cuda")]
            {
                match Device::cuda_if_available(0) {
                    Ok(device) => {
                        if !matches!(device, Device::Cpu) {
                            info!("🚀 CUDA GPU detected and available");
                            return Self {
                                gpu_type: GpuType::Cuda,
                                device_name: "NVIDIA CUDA GPU".to_string(),
                                expected_latency_ms: 80,
                                available: true,
                            };
                        }
                    },
                    Err(e) => {
                        warn!("⚠️  CUDA initialization failed: {}", e);
                    },
                }
            }

            // Try Metal next (if available)
            #[cfg(feature = "metal")]
            {
                match Device::new_metal(0) {
                    Ok(_device) => {
                        info!("🚀 Metal GPU detected and available (Apple Silicon/Intel Mac)");
                        return Self {
                            gpu_type: GpuType::Metal,
                            device_name: "Apple Metal GPU".to_string(),
                            expected_latency_ms: 120,
                            available: true,
                        };
                    },
                    Err(e) => {
                        warn!("⚠️  Metal initialization failed: {}", e);
                    },
                }
            }

            // Fall back to CPU
            #[cfg(all(not(feature = "cuda"), not(feature = "metal")))]
            {
                info!("ℹ️  Built without GPU support (CPU-only mode)");
            }

            #[cfg(any(feature = "cuda", feature = "metal"))]
            {
                warn!("⚠️  No GPU available, falling back to CPU");
            }

            Self {
                gpu_type: GpuType::Cpu,
                device_name: Self::get_cpu_name(),
                expected_latency_ms: 800,
                available: false, // GPU not available, using CPU
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
    ///
    /// # Returns
    ///
    /// - CUDA device if CUDA is available
    /// - Metal device if Metal is available
    /// - CPU device as fallback
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
                    // This shouldn't happen due to detect() logic, but handle it
                    Ok(Device::Cpu)
                }
            },
            GpuType::Metal => {
                #[cfg(feature = "metal")]
                {
                    Device::new_metal(0)
                        .map_err(|e| anyhow::anyhow!("Failed to create Metal device: {}", e))
                }
                #[cfg(not(feature = "metal"))]
                {
                    // This shouldn't happen due to detect() logic, but handle it
                    Ok(Device::Cpu)
                }
            },
            GpuType::Cpu => Ok(Device::Cpu),
        }
    }
}

/// Format GPU info as a human-readable string
impl std::fmt::Display for GpuAvailability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.available {
            write!(
                f,
                "✓ {} (expected latency: ~{}ms)",
                self.device_name, self.expected_latency_ms
            )
        } else {
            write!(
                f,
                "✗ {} (expected latency: ~{}ms) [No GPU acceleration]",
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

        // Should return same reference (cached)
        assert!(std::ptr::eq(first, second));
    }

    #[test]
    fn test_gpu_type_determination() {
        let gpu = GpuAvailability::detect();

        // Should be one of the three types
        assert!(matches!(
            gpu.gpu_type,
            GpuType::Cuda | GpuType::Metal | GpuType::Cpu
        ));
    }

    #[test]
    fn test_device_creation() {
        // Should not panic
        let device = GpuAvailability::create_device();
        assert!(device.is_ok());
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_cuda_feature_enabled() {
        // When built with CUDA feature, detection should work
        let gpu = GpuAvailability::detect();
        // Don't assert available=true because CI might not have GPU
        // Just verify detection runs without panic
        let _ = gpu.available;
    }

    #[cfg(feature = "metal")]
    #[test]
    fn test_metal_feature_enabled() {
        // When built with Metal feature, detection should work
        let gpu = GpuAvailability::detect();
        // Don't assert available=true because CI might not have Metal GPU
        // Just verify detection runs without panic
        let _ = gpu.available;
    }

    #[cfg(not(any(feature = "cuda", feature = "metal")))]
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
