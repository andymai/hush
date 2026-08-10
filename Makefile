.PHONY: build check clean dev release release-cpu production install link help cuda-preflight

# ggml's CMake auto-detects sccache from PATH and deadlocks on the CUDA kernel
# fan-out, so CUDA builds drop it from PATH entirely; unsetting RUSTC_WRAPPER
# alone only covers rustc, not nvcc.
CUDA_ENV = PATH="$$(dirname $$(rustup which cargo)):$$(echo $$PATH | tr ':' '\n' | grep -v '\.cargo/bin' | paste -sd:)" \
	RUSTC_WRAPPER= \
	PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig:$$PKG_CONFIG_PATH \
	RUSTFLAGS="-C linker=clang -C link-arg=-fuse-ld=bfd"

# Default target
help:
	@echo "🤫 Hush - Voice-to-Text for Linux Developers"
	@echo ""
	@echo "Available targets:"
	@echo "  build       - Build debug version and create symlink"
	@echo "  check       - Run cargo check (fast type checking)"
	@echo "  dev         - Same as build (alias)"
	@echo "  release     - Build release version with CUDA GPU acceleration"
	@echo "  release-cpu - Build release version (CPU only, no GPU)"
	@echo "  production  - Build max-optimized version (full LTO, slow compile)"
	@echo "  clean       - Clean build artifacts and remove symlink"
	@echo "  link        - Create/update symlink to binary"
	@echo "  install     - Build release and install to ~/.cargo/bin"
	@echo "  help        - Show this help message"

# Development build (default)
build: link
	@echo "🔨 Building debug version..."
	@PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig:$$PKG_CONFIG_PATH cargo build
	@$(MAKE) -s link

dev: build

# Fast type checking
check:
	@echo "🔍 Running cargo check..."
	@PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig:$$PKG_CONFIG_PATH cargo check
	@echo "✅ Check complete."

# Verify the CUDA toolkit is present and the link-time stub is in place.
# Only libcuda.so (the stub) is ever created here. libcuda.so.1 is the real
# driver, which is bind-mounted in containers; deleting it breaks CUDA at runtime.
cuda-preflight:
	@command -v nvcc >/dev/null 2>&1 || { \
		echo "❌ nvcc not found: CUDA builds need the CUDA toolkit, not just the driver."; \
		echo "   If your host only ships the NVIDIA driver (e.g. Fedora Atomic/ostree),"; \
		echo "   build inside a CUDA container, then re-run this target there."; \
		echo "   For a CPU-only build instead, run: make release-cpu"; \
		exit 1; \
	}
	@if [ ! -e /usr/lib/x86_64-linux-gnu/libcuda.so ]; then \
		echo "🔗 Creating CUDA stub symlink (one-time, needs sudo)..."; \
		sudo ln -sf /usr/local/cuda/lib64/stubs/libcuda.so /usr/lib/x86_64-linux-gnu/libcuda.so; \
	fi

# Release build (with CUDA GPU acceleration)
# Note: Uses GNU ld instead of mold for CUDA builds (mold can't handle CUDA stub libraries)
release: cuda-preflight
	@echo "🚀 Building release version with CUDA..."
	@$(CUDA_ENV) cargo build --release --features cuda
	@ln -sf target/release/hush ./hush
	@echo "✅ Release build complete (CUDA enabled). Use ./hush to run."

# CPU-only release build (no GPU acceleration)
release-cpu:
	@echo "🚀 Building CPU-only release version..."
	@PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig:$$PKG_CONFIG_PATH cargo build --release
	@ln -sf target/release/hush ./hush
	@echo "✅ Release build complete (CPU only). Use ./hush to run."

# Maximum optimization build (full LTO, slow compile)
# Note: Uses GNU ld instead of mold for CUDA builds (mold can't handle CUDA stub libraries)
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
	@cargo install --path .
	@echo "✅ Installed! You can now run 'hush' from anywhere."