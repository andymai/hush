# T-032: Make CUDA Optional with Auto-Detection

**Priority:** High
**Effort:** Medium (4-6 hours)
**Type:** Feature Enhancement
**Status:** Available
**Created:** 2025-11-19

---

## Problem Statement

Currently, Hush hardcodes CUDA features in Cargo.toml, making it impossible to build on systems without CUDA development environment. This creates barriers for:

1. **Web-based development environments** (GitHub Codespaces, GitPod) that don't have CUDA
2. **CPU-only systems** without NVIDIA GPUs
3. **Docker containers** optimized for size (CUDA adds ~2GB)
4. **CI/CD systems** that don't need GPU acceleration for tests

### Current State

```toml
# Cargo.toml (Lines 24-26)
candle-core = { version = "0.8", features = ["cuda"] }
candle-nn = { version = "0.8", features = ["cuda"] }
candle-transformers = { version = "0.8", features = ["cuda"] }
```

All builds fail with:
```
error: failed to run custom build command for `candle-core v0.8.0`
note: CUDA toolkit not found
```

---

## Acceptance Criteria

### Must Have

1. ✅ **Feature flag for CUDA**
   - Add `cuda` feature flag in Cargo.toml (default enabled)
   - Allow users to build without CUDA: `cargo build --no-default-features`
   - Maintain backward compatibility (CUDA enabled by default)

2. ✅ **Runtime GPU detection**
   - Detect NVIDIA GPU presence at runtime
   - Check if CUDA toolkit is available (not just compile-time)
   - Gracefully fall back to CPU if CUDA unavailable
   - Log detection results with INFO level

3. ✅ **CPU fallback path**
   - Ensure Whisper transcription works on CPU-only builds
   - No runtime panics or unwraps when CUDA missing
   - Clear user messaging about performance difference

4. ✅ **Updated documentation**
   - Document how to build CPU-only version
   - Update README system requirements
   - Add performance comparison (CPU vs GPU)

### Should Have

5. ✅ **Build script validation**
   - Add build.rs to detect CUDA at compile time
   - Provide helpful error messages if CUDA requested but unavailable
   - Skip CUDA linking if feature disabled

6. ✅ **Test both paths**
   - Verify CPU-only build compiles
   - Test runtime behavior without CUDA
   - Add CI job for CPU-only builds

### Nice to Have

7. ⚠️ **Smart default selection**
   - Auto-select GPU model if available, CPU model otherwise
   - Cache detection result to avoid repeated checks
   - Allow manual override via config or CLI flag

8. ⚠️ **Performance warnings**
   - Warn user if running CPU on large models
   - Suggest GPU upgrade for better experience
   - Estimate transcription time based on audio length

---

## Implementation Plan

### Phase 1: Cargo.toml Feature Flags (1-2 hours)

**File:** `Cargo.toml`

```toml
[features]
default = ["notifications", "system-tray", "cuda"]
notifications = ["dep:notify-rust"]
system-tray = ["dep:ksni"]
cuda = [
    "candle-core/cuda",
    "candle-nn/cuda",
    "candle-transformers/cuda",
    "whisper-rs/cuda"
]

[dependencies]
# Whisper integration (CUDA optional)
candle-core = { version = "0.8" }
candle-nn = { version = "0.8" }
candle-transformers = { version = "0.8" }
tokenizers = "0.19"
hf-hub = { version = "0.3", features = ["tokio"] }
whisper-rs = { version = "0.15", features = ["tracing_backend"] }
```

**Verification:**
```bash
# Test CPU-only build
cargo build --no-default-features --features notifications,system-tray

# Test with CUDA (default)
cargo build
```

### Phase 2: Runtime CUDA Detection (2 hours)

**File:** `src/transcription/cuda.rs`

Currently only has:
```rust
pub fn is_cuda_available() -> bool {
    candle_core::cuda_is_available()
}
```

**Enhance to:**
```rust
use std::sync::OnceLock;
use tracing::{info, warn};

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
                let available = candle_core::cuda_is_available();

                if available {
                    let device_count = candle_core::cuda_device_count().unwrap_or(0);
                    let device_name = candle_core::cuda_device_name(0).ok();
                    let cuda_version = candle_core::cuda_version().ok().map(|v| v.to_string());

                    info!(
                        "🚀 CUDA detected: {} device(s), {:?}",
                        device_count,
                        device_name
                    );

                    CudaAvailability {
                        available: true,
                        device_count,
                        device_name,
                        cuda_version,
                    }
                } else {
                    warn!("⚠️  CUDA not available, falling back to CPU");
                    CudaAvailability {
                        available: false,
                        device_count: 0,
                        device_name: None,
                        cuda_version: None,
                    }
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

// Legacy compatibility
pub fn is_cuda_available() -> bool {
    CudaAvailability::is_available()
}
```

### Phase 3: Update Transcription Code (1 hour)

**Files to update:**
- `src/transcription/whisper.rs`
- `src/transcription/models.rs`
- `src/adapters/transcription/whisper_adapter.rs`

**Current code pattern (whisper.rs:368):**
```rust
// TODO: Make this configurable based on caller preference
let device = if candle_core::cuda_is_available() {
    candle_core::Device::new_cuda(0)?
} else {
    candle_core::Device::Cpu
};
```

**Update to:**
```rust
use crate::transcription::cuda::CudaAvailability;

let cuda = CudaAvailability::detect();
let device = if cuda.available {
    info!("Using GPU: {:?}", cuda.device_name);
    candle_core::Device::new_cuda(0)?
} else {
    info!("Using CPU (expect slower transcription)");
    candle_core::Device::Cpu
};
```

**Add to WhisperTranscriber:**
```rust
pub fn is_using_gpu(&self) -> bool {
    CudaAvailability::is_available()
}

pub fn device_info(&self) -> String {
    let cuda = CudaAvailability::detect();
    if cuda.available {
        format!("GPU: {:?}", cuda.device_name.as_deref().unwrap_or("Unknown"))
    } else {
        "CPU".to_string()
    }
}
```

### Phase 4: Update Status Command (30 min)

**File:** `src/cli/commands/status.rs`

Add GPU detection to status output:

```rust
// System Information
let cuda = CudaAvailability::detect();
println!("\n📊 System Information:");
println!("  OS: {}", std::env::consts::OS);
println!("  Architecture: {}", std::env::consts::ARCH);

if cuda.available {
    println!("  🚀 GPU: {} (CUDA {})",
        cuda.device_name.as_deref().unwrap_or("Unknown"),
        cuda.cuda_version.as_deref().unwrap_or("Unknown"));
    println!("     Devices: {}", cuda.device_count);
} else {
    #[cfg(feature = "cuda")]
    println!("  ⚠️  GPU: Not detected (using CPU)");

    #[cfg(not(feature = "cuda"))]
    println!("  💻 GPU: Not compiled (CPU-only build)");
}
```

### Phase 5: Documentation Updates (1 hour)

**Update:** `README.md`

```markdown
## System Requirements

### Minimum Requirements (CPU-only)
- **OS:** Linux (Ubuntu 20.04+, Fedora 35+, or equivalent)
- **CPU:** x86_64 with AVX2 support
- **RAM:** 4GB+ (8GB+ for larger models)
- **Rust:** 1.70+

### Recommended (GPU-accelerated)
- **GPU:** NVIDIA GPU with CUDA 12.0+ support
- **VRAM:** 4GB+ for base model, 8GB+ for large models
- **CUDA:** CUDA Toolkit 12.0+

### Build Options

**GPU-accelerated (default):**
```bash
cargo build --release
# Requires: CUDA toolkit, NVIDIA GPU
# Performance: ~0.5s for 3s audio (base model)
```

**CPU-only:**
```bash
cargo build --release --no-default-features --features notifications,system-tray
# No CUDA required
# Performance: ~4-6s for 3s audio (base model)
```
```

**Update:** `.ai/knowledge/conventions.md`

Add section on CUDA feature usage:

```markdown
## CUDA Feature Flag

### Building with CUDA (default)
```bash
cargo build                  # CUDA enabled
cargo build --all-features   # CUDA enabled
```

### Building without CUDA
```bash
cargo build --no-default-features --features notifications,system-tray
```

### Runtime Behavior
- CUDA availability is detected at runtime
- Graceful fallback to CPU if GPU unavailable
- Detection is cached (no repeated checks)
- Check status: `./hush status --full`
```

### Phase 6: Testing (1 hour)

**Add tests:**

```rust
// src/transcription/cuda.rs
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
        let _ = is_cuda_available();
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
```

**Add CI job:** `.github/workflows/ci.yml`

```yaml
test-cpu-only:
  name: Test CPU-only build
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4

    - name: Install Rust
      uses: dtolnay/rust-toolchain@stable

    - uses: Swatinem/rust-cache@v2

    - name: Install system dependencies
      run: |
        sudo apt-get update
        sudo apt-get install -y libasound2-dev pkg-config libx11-dev libdbus-1-dev

    - name: Build CPU-only
      run: cargo build --no-default-features --features notifications,system-tray

    - name: Test CPU-only
      run: cargo test --no-default-features --features notifications,system-tray
```

---

## Verification Steps

After implementation:

1. ✅ **CPU-only build works:**
   ```bash
   cargo build --no-default-features --features notifications,system-tray
   ./hush status --full
   # Should show: "GPU: Not compiled (CPU-only build)"
   ```

2. ✅ **CUDA build works (on GPU system):**
   ```bash
   cargo build --release
   ./hush status --full
   # Should show: "GPU: NVIDIA GeForce RTX 3080 (CUDA 12.0)"
   ```

3. ✅ **CUDA build gracefully falls back (on non-GPU system):**
   ```bash
   cargo build --release
   ./hush status --full
   # Should show: "GPU: Not detected (using CPU)"
   ```

4. ✅ **Transcription works in both modes:**
   ```bash
   # Test with either build
   ./hush record --duration 5
   # Should complete successfully, just slower on CPU
   ```

5. ✅ **CI passes:**
   - Regular CI with CUDA features
   - New CPU-only CI job

---

## Files to Modify

| File | Changes | Lines |
|------|---------|-------|
| `Cargo.toml` | Add `cuda` feature flag | ~10 |
| `src/transcription/cuda.rs` | Enhanced detection | ~80 |
| `src/transcription/whisper.rs` | Use new detection | ~5 |
| `src/transcription/models.rs` | Use new detection | ~5 |
| `src/cli/commands/status.rs` | Add GPU info to status | ~15 |
| `README.md` | Document CPU/GPU builds | ~30 |
| `.ai/knowledge/conventions.md` | Add CUDA feature docs | ~20 |
| `.github/workflows/ci.yml` | Add CPU-only CI job | ~20 |
| **Total** | | **~185 lines** |

---

## Success Metrics

1. ✅ Web environments (Codespaces) can build Hush
2. ✅ CPU-only Docker images are possible
3. ✅ Users can build without CUDA toolkit
4. ✅ Runtime detection provides clear feedback
5. ✅ No performance regression on GPU systems
6. ✅ CI validates both build configurations

---

## References

- **Issue from code review:** "CUDA Requirement - Dependencies hardcode CUDA features"
- **Current CUDA detection:** `src/transcription/cuda.rs:9`
- **TODO comment:** `src/transcription/whisper.rs:368`
- **Similar pattern:** Rust's `tokio` crate with `rt` and `rt-multi-thread` features

---

## Notes

- This task resolves the build barrier mentioned in the comprehensive code review
- Maintains backward compatibility (CUDA enabled by default)
- Enables development in web-based IDEs
- Reduces Docker image size for CPU-only deployments
- Performance difference is documented and user-visible
