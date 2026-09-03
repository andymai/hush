.PHONY: build check clean dev release release-cpu production install link help cuda-preflight

# CUDA builds need the toolkit's stub libcuda.so at link time; the real driver
# library (libcuda.so.1) resolves at runtime. Pointing the linker at the stubs
# directory avoids creating a system-wide symlink. CUDA_HOME is derived from
# nvcc so distro packages (/usr/local/cuda, /usr/local/cuda-12.x, /opt/cuda)
# all work.
CUDA_HOME ?= $(shell dirname $$(dirname $$(command -v nvcc 2>/dev/null || echo /usr/local/cuda/bin/nvcc)))
CUDA_STUBS := $(CUDA_HOME)/lib64/stubs

# ggml's CMake auto-detects sccache from PATH and deadlocks on the CUDA kernel
# fan-out, so CUDA builds drop it from PATH entirely; unsetting RUSTC_WRAPPER
# alone only covers rustc, not nvcc. mold cannot link the CUDA stub library,
# so CUDA builds force the bfd linker regardless of any local .cargo/config.
CUDA_ENV = PATH="$$(dirname $$(rustup which cargo)):$$(echo $$PATH | tr ':' '\n' | grep -v '\.cargo/bin' | paste -sd:)" \
	RUSTC_WRAPPER= \
	RUSTFLAGS="-C link-arg=-fuse-ld=bfd -L $(CUDA_STUBS)"

# Default target
help:
	@echo "🤫 Hush - Voice-to-Text for Linux"
	@echo ""
	@echo "Available targets:"
	@echo "  build       - Build debug version and create symlink"
	@echo "  check       - Run cargo check (fast type checking)"
	@echo "  dev         - Same as build (alias)"
	@echo "  release     - Build release version with CUDA GPU acceleration"
	@echo "  release-cpu - Build release version (CPU only, no GPU)"
	@echo "  production  - Build max-optimized CUDA version (full LTO, slow compile)"
	@echo "  clean       - Clean build artifacts and remove symlink"
	@echo "  link        - Create/update symlink to binary"
	@echo "  install     - Build release and install to ~/.cargo/bin"
	@echo "  help        - Show this help message"

# Development build (default)
build:
	@echo "🔨 Building debug version..."
	@cargo build
	@$(MAKE) -s link

dev: build

# Fast type checking
check:
	@echo "🔍 Running cargo check..."
	@cargo check
	@echo "✅ Check complete."

# Verify the CUDA toolkit is present and the link-time stub exists.
cuda-preflight:
	@command -v nvcc >/dev/null 2>&1 || { \
		echo "❌ nvcc not found: CUDA builds need the CUDA toolkit, not just the driver."; \
		echo "   If your host only ships the NVIDIA driver (e.g. Fedora Atomic/ostree),"; \
		echo "   build inside a CUDA container, then re-run this target there."; \
		echo "   For a CPU-only build instead, run: make release-cpu"; \
		exit 1; \
	}
	@[ -e "$(CUDA_STUBS)/libcuda.so" ] || { \
		echo "❌ $(CUDA_STUBS)/libcuda.so not found."; \
		echo "   Set CUDA_HOME to your CUDA toolkit root, e.g. make release CUDA_HOME=/opt/cuda"; \
		exit 1; \
	}

# Release build (with CUDA GPU acceleration)
release: cuda-preflight
	@echo "🚀 Building release version with CUDA..."
	@$(CUDA_ENV) cargo build --release --features cuda
	@ln -sf target/release/hush ./hush
	@echo "✅ Release build complete (CUDA enabled). Use ./hush to run."

# CPU-only release build (no GPU acceleration)
release-cpu:
	@echo "🚀 Building CPU-only release version..."
	@cargo build --release
	@ln -sf target/release/hush ./hush
	@echo "✅ Release build complete (CPU only). Use ./hush to run."

# Maximum optimization build (full LTO, slow compile)
production: cuda-preflight
	@echo "🏭 Building production version (full LTO, this will take a while)..."
	@$(CUDA_ENV) cargo build --profile production --features cuda
	@ln -sf target/production/hush ./hush
	@echo "✅ Production build complete. Use ./hush to run."

# Clean everything
clean:
	@echo "🧹 Cleaning build artifacts..."
	@cargo clean
	@rm -f ./hush
	@echo "✅ Clean complete."

# Create/update symlink
link:
	@if [ -f target/debug/hush ]; then \
		ln -sf target/debug/hush ./hush; \
		echo "🔗 Symlink updated: ./hush -> target/debug/hush"; \
	else \
		echo "⚠️  Binary not found. Run 'make build' first."; \
	fi

# Install to user's cargo bin
install: release
	@echo "📦 Installing to ~/.cargo/bin..."
	@cargo install --path . --features cuda
	@echo "✅ Installed! You can now run 'hush' from anywhere."
