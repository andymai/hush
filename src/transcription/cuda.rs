use std::sync::OnceLock;
use tracing::info;
#[cfg(feature = "cuda")]
use tracing::warn;

static CUDA_AVAILABLE: OnceLock<CudaAvailability> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct CudaAvailability {
    pub available: bool,
    pub device_count: usize,
    pub device_name: Option<String>,
    pub cuda_version: Option<String>,
}

impl CudaAvailability {
    /// Detect CUDA availability (cached)
    pub fn detect() -> &'static Self {
        CUDA_AVAILABLE.get_or_init(|| {
            #[cfg(feature = "cuda")]
            {
                // Try to create a CUDA device to check availability
                match candle_core::Device::cuda_if_available(0) {
                    Ok(_device) => {
                        // CUDA is available
                        // Note: Device count and detailed info are not easily accessible
                        // from candle_core's public API in newer versions
                        info!("🚀 CUDA detected and available");

                        CudaAvailability {
                            available: true,
                            device_count: 1, // At least one device is available
                            device_name: Some("CUDA Device".to_string()), // Generic name
                            cuda_version: None, // Version info not easily accessible
                        }
                    },
                    Err(_) => {
                        warn!("⚠️  CUDA not available, falling back to CPU");
                        CudaAvailability {
                            available: false,
                            device_count: 0,
                            device_name: None,
                            cuda_version: None,
                        }
                    },
                }
            }

            #[cfg(not(feature = "cuda"))]
            {
                info!("ℹ️  Built without CUDA support (CPU-only mode)");
                CudaAvailability {
                    available: false,
                    device_count: 0,
                    device_name: None,
                    cuda_version: None,
                }
            }
        })
    }

    pub fn is_available() -> bool {
        Self::detect().available
    }
}

// Legacy compatibility functions
pub fn check_cuda_availability() -> crate::Result<bool> {
    Ok(CudaAvailability::is_available())
}

pub fn get_optimal_device() -> crate::Result<String> {
    let cuda = CudaAvailability::detect();
    if cuda.available {
        Ok(format!(
            "GPU: {}",
            cuda.device_name.as_deref().unwrap_or("Unknown")
        ))
    } else {
        Ok("CPU".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cuda_detection_cached() {
        let first = CudaAvailability::detect();
        let second = CudaAvailability::detect();

        // Should return same reference (cached)
        assert!(std::ptr::eq(first, second));
    }

    #[test]
    fn test_legacy_function() {
        // Legacy function should work
        let _ = check_cuda_availability();
        let _ = get_optimal_device();
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_cuda_feature_enabled() {
        // When built with CUDA feature, detection should work
        let cuda = CudaAvailability::detect();
        // Don't assert available=true because CI might not have GPU
        // Just verify detection runs without panic
        let _ = cuda.available;
    }

    #[cfg(not(feature = "cuda"))]
    #[test]
    fn test_cpu_only_build() {
        let cuda = CudaAvailability::detect();
        assert!(!cuda.available);
        assert_eq!(cuda.device_count, 0);
    }
}
