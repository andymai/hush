# T-036: Add Metal GPU Acceleration Support for Apple Silicon

**Priority:** High
**Effort:** Medium (4-6 hours)
**Type:** Platform Support - Performance
**Status:** Available
**Created:** 2025-11-19

---

## Problem Statement

Currently, GPU acceleration is hardcoded to CUDA (NVIDIA), which doesn't work on macOS. Apple Silicon Macs (M1, M2, M3+) have powerful Neural Engine and Metal GPU support. Candle already supports Metal backend, but it's not enabled in Hush.

### Performance Comparison

- **CPU only**: ~800ms first token latency
- **CUDA (NVIDIA)**: ~80ms first token latency (10x faster)
- **Metal (Apple Silicon)**: ~120ms first token latency (6-8x faster)

Without Metal support, macOS users lose significant performance.

---

## Goals

1. ✅ Add `metal` feature flag alongside `cuda`
2. ✅ Make Metal the default on macOS targets
3. ✅ Ensure CPU fallback works when Metal unavailable
4. ✅ Update Whisper integration to use Metal backend
5. ✅ Maintain Linux CUDA functionality
6. ✅ Document Metal vs CUDA performance expectations

---

## Implementation Steps

### 1. Add Metal Feature Flag

**Update: `Cargo.toml`**

```toml
[features]
# Default features per platform
default = ["notifications", "system-tray"]

# GPU acceleration features (mutually exclusive)
cuda = [
    "candle-core/cuda",
    "candle-nn/cuda",
    "candle-transformers/cuda",
    "whisper-rs/cuda"
]

metal = [
    "candle-core/metal",
    "candle-nn/metal",
    "candle-transformers/metal",
]

# Platform-specific defaults
[target.'cfg(target_os = "linux")'.features]
default = ["cuda"]  # CUDA on Linux (if available)

[target.'cfg(target_os = "macos")'.features]
default = ["metal"]  # Metal on macOS
```

**Notes:**
- `whisper-rs` doesn't have Metal support, only Candle does
- CUDA and Metal should be mutually exclusive
- Both should gracefully fall back to CPU if hardware unavailable

### 2. Update Whisper Adapter for Metal

**Update: `src/adapters/transcription/whisper_adapter.rs`**

```rust
use candle_core::{Device, DType};
use tracing::{info, warn};

impl WhisperAdapter {
    pub fn new(model_path: &Path) -> Result<Self> {
        info!("Initializing Whisper adapter");

        // Select device based on available features and hardware
        let device = Self::select_device()?;
        info!("Using device: {:?}", device);

        // Load model with selected device
        let model = Self::load_model(model_path, &device)?;

        Ok(Self { model, device })
    }

    fn select_device() -> Result<Device> {
        // Priority: GPU (CUDA/Metal) > CPU

        #[cfg(feature = "cuda")]
        {
            match Device::new_cuda(0) {
                Ok(device) => {
                    info!("CUDA GPU device available, using GPU acceleration");
                    return Ok(device);
                }
                Err(e) => {
                    warn!("CUDA requested but initialization failed: {}", e);
                    warn!("Falling back to CPU");
                }
            }
        }

        #[cfg(feature = "metal")]
        {
            match Device::new_metal(0) {
                Ok(device) => {
                    info!("Metal GPU device available, using GPU acceleration");
                    return Ok(device);
                }
                Err(e) => {
                    warn!("Metal requested but initialization failed: {}", e);
                    warn!("Falling back to CPU");
                }
            }
        }

        // Fallback to CPU
        info!("Using CPU for transcription (no GPU acceleration)");
        Ok(Device::Cpu)
    }
}
```

### 3. Add Device Detection Utility

**New file: `src/adapters/transcription/device.rs`**

```rust
use candle_core::Device;
use tracing::info;

pub struct DeviceInfo {
    pub device_type: DeviceType,
    pub name: String,
    pub expected_latency_ms: u32,
}

pub enum DeviceType {
    CudaGpu,
    MetalGpu,
    Cpu,
}

impl DeviceInfo {
    pub fn detect() -> Self {
        #[cfg(feature = "cuda")]
        {
            if let Ok(device) = Device::new_cuda(0) {
                return Self {
                    device_type: DeviceType::CudaGpu,
                    name: Self::get_cuda_name(),
                    expected_latency_ms: 80,
                };
            }
        }

        #[cfg(feature = "metal")]
        {
            if let Ok(device) = Device::new_metal(0) {
                return Self {
                    device_type: DeviceType::MetalGpu,
                    name: Self::get_metal_name(),
                    expected_latency_ms: 120,
                };
            }
        }

        Self {
            device_type: DeviceType::Cpu,
            name: Self::get_cpu_name(),
            expected_latency_ms: 800,
        }
    }

    fn get_metal_name() -> String {
        use std::process::Command;

        // Try to get Apple Silicon chip name
        if let Ok(output) = Command::new("sysctl")
            .args(&["-n", "machdep.cpu.brand_string"])
            .output()
        {
            if let Ok(brand) = String::from_utf8(output.stdout) {
                let brand = brand.trim();
                if brand.contains("Apple") {
                    return format!("Apple Silicon ({})", brand);
                }
            }
        }

        "Metal GPU".to_string()
    }

    fn get_cuda_name() -> String {
        // Try to get NVIDIA GPU name
        use std::process::Command;

        if let Ok(output) = Command::new("nvidia-smi")
            .args(&["--query-gpu=name", "--format=csv,noheader"])
            .output()
        {
            if let Ok(name) = String::from_utf8(output.stdout) {
                return name.trim().to_string();
            }
        }

        "CUDA GPU".to_string()
    }

    fn get_cpu_name() -> String {
        "CPU".to_string()
    }
}

// Add CLI command to show device info
pub fn print_device_info() {
    let info = DeviceInfo::detect();

    println!("🔧 GPU Acceleration Status\n");

    match info.device_type {
        DeviceType::CudaGpu => {
            println!("✅ CUDA GPU: {}", info.name);
            println!("   Expected latency: ~{}ms", info.expected_latency_ms);
            println!("   Feature: cuda");
        }
        DeviceType::MetalGpu => {
            println!("✅ Metal GPU: {}", info.name);
            println!("   Expected latency: ~{}ms", info.expected_latency_ms);
            println!("   Feature: metal");
        }
        DeviceType::Cpu => {
            println!("⚠️  CPU Only: {}", info.name);
            println!("   Expected latency: ~{}ms", info.expected_latency_ms);
            println!("   No GPU acceleration enabled");

            #[cfg(target_os = "macos")]
            println!("\n💡 To enable Metal GPU acceleration:");
            println!("   cargo build --features metal");

            #[cfg(target_os = "linux")]
            println!("\n💡 To enable CUDA GPU acceleration:");
            println!("   cargo build --features cuda");
        }
    }
}
```

### 4. Update CLI to Show Device Info

**Update: `src/cli/mod.rs`**

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "hush")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start listening for voice input
    Listen,

    /// Show GPU device information
    DeviceInfo,

    // ... other commands
}

// In main.rs or appropriate handler:
Commands::DeviceInfo => {
    crate::adapters::transcription::device::print_device_info();
}
```

### 5. Update Documentation

**Update: `README.md`**

```markdown
## GPU Acceleration

### Linux (NVIDIA)
```bash
# Build with CUDA support
cargo build --release --features cuda

# Requires CUDA 12.0+ toolkit
# See INSTALL.md for CUDA installation
```

### macOS (Apple Silicon)
```bash
# Build with Metal support (default on macOS)
cargo build --release --features metal

# Requires M1, M2, or M3 chip
# No additional installation needed
```

### Performance Expectations
- **CUDA (NVIDIA)**: ~80ms first token latency (10x faster)
- **Metal (Apple Silicon)**: ~120ms first token latency (6-8x faster)
- **CPU only**: ~800ms first token latency (baseline)

### Check Your Device
```bash
./hush device-info
```
```

---

## Success Criteria

- [ ] `metal` feature flag compiles on macOS
- [ ] Metal GPU acceleration works on Apple Silicon Macs
- [ ] CPU fallback works when Metal unavailable (Intel Macs)
- [ ] CUDA still works on Linux (no regression)
- [ ] Device detection correctly identifies GPU type
- [ ] `hush device-info` command shows current configuration
- [ ] Performance tests show 6-8x speedup on M1/M2
- [ ] Documentation updated with Metal instructions

---

## Verification Steps

```bash
# On Apple Silicon Mac (M1/M2/M3):
# 1. Build with Metal
cargo build --release --features metal

# 2. Check device detection
./target/release/hush device-info
# Expected output: "✅ Metal GPU: Apple Silicon (Apple M2)"

# 3. Benchmark transcription
time ./target/release/hush listen
# Speak for 3 seconds, measure latency
# Expected: ~120ms first token

# 4. Compare with CPU-only
cargo build --release --no-default-features
time ./target/release/hush listen
# Expected: ~800ms first token (6-8x slower)

# On Intel Mac:
# Metal should fail gracefully, fall back to CPU
cargo build --release --features metal
./target/release/hush device-info
# Expected: "⚠️  CPU Only"

# On Linux:
# CUDA should still work
cargo build --release --features cuda
./target/release/hush device-info
# Expected: "✅ CUDA GPU: NVIDIA RTX 4090" (or similar)
```

---

## Files to Create

- `src/adapters/transcription/device.rs` (~150 lines)

## Files to Modify

- `Cargo.toml` - Add metal feature flag
- `src/adapters/transcription/whisper_adapter.rs` - Metal device selection
- `src/cli/mod.rs` - Add device-info command
- `README.md` - Document Metal support
- `INSTALL.md` - Add macOS Metal section (if not exists)

---

## Dependencies

**Blocks:**
- T-039 (performance tests need Metal working)
- T-040 (documentation needs Metal instructions)

**Blocked by:**
- T-033 (needs candle-core/metal dependency)

---

## References

- **Candle Metal Backend**: https://github.com/huggingface/candle/tree/main/candle-metal
- **Metal Performance Shaders**: https://developer.apple.com/documentation/metalperformanceshaders
- **Apple Silicon Performance**: https://github.com/huggingface/candle/issues/123
- **Device Selection**: Candle automatically selects Metal on Apple Silicon

---

## Performance Testing

**Benchmark Script** (add to `benches/metal_performance.rs`):

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use hush::adapters::transcription::WhisperAdapter;

fn benchmark_transcription(c: &mut Criterion) {
    let adapter = WhisperAdapter::new("models/whisper-base").unwrap();
    let audio_samples = load_test_audio();

    c.bench_function("whisper_transcription", |b| {
        b.iter(|| {
            adapter.transcribe(black_box(&audio_samples))
        });
    });
}

criterion_group!(benches, benchmark_transcription);
criterion_main!(benches);
```

**Expected Results:**
- M1 Mac: ~120ms
- M2 Mac: ~100ms
- M3 Mac: ~80ms
- Intel Mac (CPU): ~800ms

---

## Known Limitations

1. **Metal backend maturity**: Candle's Metal support is newer than CUDA, may have edge cases
2. **Intel Macs**: No Metal GPU acceleration, CPU only
3. **Older macOS versions**: Metal requires macOS 10.13+
4. **Feature exclusivity**: Cannot enable both CUDA and Metal simultaneously

These are acceptable trade-offs for macOS support.

---

## Troubleshooting

**"Metal device initialization failed"**
- Check macOS version (requires 10.13+)
- On Intel Macs, Metal GPU is slower than CPU - this is expected
- Verify with `hush device-info`

**"Expected ~120ms but seeing ~800ms"**
- Metal feature not enabled: rebuild with `--features metal`
- Running on Intel Mac: Metal not effective, use CPU
- Model not loaded on GPU: check logs for device selection

**Build errors with Metal**
- Update candle crates to latest version
- Check Rust version (requires 1.70+)
- Verify Xcode Command Line Tools installed
