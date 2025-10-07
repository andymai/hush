.PHONY: build clean dev release install link help

# Default target
help:
	@echo "🤫 Hush - Voice-to-Text for Linux Developers"
	@echo ""
	@echo "Available targets:"
	@echo "  build    - Build debug version and create symlink"
	@echo "  dev      - Same as build (alias)"
	@echo "  release  - Build release version and create symlink"
	@echo "  clean    - Clean build artifacts and remove symlink"
	@echo "  link     - Create/update symlink to binary"
	@echo "  install  - Build release and install to ~/.cargo/bin"
	@echo "  help     - Show this help message"

# Development build (default)
build: link
	@echo "🔨 Building debug version..."
	@PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig:$$PKG_CONFIG_PATH cargo build
	@$(MAKE) -s link

dev: build

# Release build
release:
	@echo "🚀 Building release version..."
	@PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig:$$PKG_CONFIG_PATH cargo build --release
	@ln -sf target/release/hush ./hush
	@echo "✅ Release build complete. Use ./hush to run."

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