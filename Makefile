.PHONY: build check clean dev release release-cuda release-cpu production install link help vulkan-preflight cuda-preflight

# transcribe-cpp-sys passes these to the CMake build of transcribe.cpp. Without
# the flag, the build links OpenBLAS when it finds the library, and the binary
# then needs it at runtime.
export TRANSCRIBE_CMAKE_ARGS ?= -DTRANSCRIBE_USE_SYSTEM_BLAS=OFF

# The CUDA backend module links the toolkit's stub libcuda.so; the real driver
# library (libcuda.so.1) resolves at runtime. CUDA_HOME is derived from nvcc so
# distro packages (/usr/local/cuda, /usr/local/cuda-12.x, /opt/cuda) all work.
CUDA_HOME ?= $(shell dirname $$(dirname $$(command -v nvcc 2>/dev/null || echo /usr/local/cuda/bin/nvcc)))
CUDA_STUBS := $(CUDA_HOME)/lib64/stubs

# ggml's CMake auto-detects sccache from PATH and deadlocks on the CUDA kernel
# fan-out, so CUDA builds drop it from PATH entirely; unsetting RUSTC_WRAPPER
# alone only covers rustc, not nvcc. NCCL serves several GPUs and would be one
# more runtime library, so it is off.
CUDA_ENV = PATH="$$(dirname $$(rustup which cargo)):$$(echo $$PATH | tr ':' '\n' | grep -v '\.cargo/bin' | paste -sd:)" \
	RUSTC_WRAPPER= \
	CUDA_HOME="$(CUDA_HOME)" \
	TRANSCRIBE_CMAKE_ARGS="$(TRANSCRIBE_CMAKE_ARGS) -DGGML_CUDA_NCCL=OFF"

# `make install` puts the binary in $(PREFIX)/bin and its libraries in
# $(PREFIX)/lib/hush, where the binary looks for them.
PREFIX ?= $(HOME)/.cargo

# Default target
help:
	@echo "🤫 Hush - Voice-to-Text for Linux"
	@echo ""
	@echo "Available targets:"
	@echo "  build        - Build debug version and create symlink"
	@echo "  check        - Run cargo check (fast type checking)"
	@echo "  dev          - Same as build (alias)"
	@echo "  release      - Build release version with Vulkan GPU acceleration"
	@echo "  release-cuda - Build release version with CUDA GPU acceleration"
	@echo "  release-cpu  - Build release version (CPU only, no GPU)"
	@echo "  production   - Build max-optimized Vulkan version (full LTO, slow compile)"
	@echo "  clean        - Clean build artifacts and remove symlink"
	@echo "  link         - Create/update symlink to binary"
	@echo "  install      - Build release, install to ~/.cargo/bin and ~/.cargo/lib/hush (PREFIX=...)"
	@echo "  help         - Show this help message"

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

# Verify the shader compiler, Vulkan headers, and SPIR-V headers ggml-vulkan
# needs at build time.
SPIRV_HEADERS := $(wildcard /usr/share/cmake/SPIRV-Headers /usr/lib/cmake/SPIRV-Headers \
	/usr/lib64/cmake/SPIRV-Headers /usr/lib/*/cmake/SPIRV-Headers \
	$(if $(VULKAN_SDK),$(VULKAN_SDK)/*/cmake/SPIRV-Headers))
vulkan-preflight:
	@command -v glslc >/dev/null 2>&1 && [ -e /usr/include/vulkan/vulkan.h ] && [ -n "$(SPIRV_HEADERS)" ] || { \
		echo "❌ Vulkan builds need glslc, the Vulkan headers, and the SPIR-V headers."; \
		echo "   Ubuntu/Debian: sudo apt install glslc libvulkan-dev spirv-headers"; \
		echo "   Fedora:        sudo dnf install glslc vulkan-headers vulkan-loader-devel spirv-headers-devel"; \
		echo "   Arch:          sudo pacman -S shaderc vulkan-headers vulkan-icd-loader spirv-headers"; \
		echo "   For a CPU-only build instead, run: make release-cpu"; \
		exit 1; \
	}

# Verify the CUDA toolkit is present and the link-time stub exists.
cuda-preflight:
	@command -v nvcc >/dev/null 2>&1 || { \
		echo "❌ nvcc not found: CUDA builds need the CUDA toolkit, not just the driver."; \
		echo "   If your host only ships the NVIDIA driver (e.g. Fedora Atomic/ostree),"; \
		echo "   build inside a CUDA container, then re-run this target there."; \
		echo "   For a GPU build without the toolkit, run: make release (Vulkan)"; \
		exit 1; \
	}
	@[ -e "$(CUDA_STUBS)/libcuda.so" ] || { \
		echo "❌ $(CUDA_STUBS)/libcuda.so not found."; \
		echo "   Set CUDA_HOME to your CUDA toolkit root, e.g. make release-cuda CUDA_HOME=/opt/cuda"; \
		exit 1; \
	}

# Each release target leaves the binary in target/release, and libtranscribe
# with the ggml backend modules in target/release/lib/hush: the CPU modules,
# from which ggml picks the best for this CPU at run time, and the GPU module.

# Release build (Vulkan GPU acceleration)
release: vulkan-preflight
	@echo "🚀 Building release version with Vulkan..."
	@cargo build --release --features vulkan
	@ln -sf target/release/hush ./hush
	@echo "✅ Release build complete (Vulkan enabled). Use ./hush to run."

# Release build (CUDA GPU acceleration)
release-cuda: cuda-preflight
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
production: vulkan-preflight
	@echo "🏭 Building production version (full LTO, this will take a while)..."
	@cargo build --profile production --features vulkan
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

# Install the Vulkan release build. The binary finds its libraries in
# ../lib/hush of the folder that really holds it, so a symlinked bin folder
# counts as its target. ggml loads every module in the folder, so a module
# that this build lacks goes.
install: release
	@install -Dm755 target/release/hush "$(PREFIX)/bin/hush"
	@lib="$$(dirname "$$(cd "$(PREFIX)/bin" && pwd -P)")/lib/hush"; \
	install -d "$$lib"; \
	for old in "$$lib"/libggml-*.so; do \
		[ ! -e "$$old" ] || [ -e "target/release/lib/hush/$${old##*/}" ] || rm -f "$$old"; \
	done; \
	install -m644 target/release/lib/hush/* "$$lib/"; \
	echo "📦 Installed $(PREFIX)/bin/hush and its libraries in $$lib"
	@echo "✅ You can now run 'hush' from anywhere."
