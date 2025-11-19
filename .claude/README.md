# Claude Code Configuration

This directory contains configuration for Claude Code sessions.

## SessionStart Hook

The `hooks/session-start.sh` script automatically runs when a Claude Code web session starts. It:

1. **Installs system dependencies** (Ubuntu/Debian packages):
   - build-essential - C/C++ compiler toolchain
   - pkg-config - Package configuration tool
   - libasound2-dev - ALSA audio library (for audio capture)
   - libx11-dev - X11 window system library
   - libdbus-1-dev - D-Bus message bus library (for notifications)

2. **Downloads Rust dependencies**:
   - Runs `cargo fetch` to cache dependencies

## Current Limitations

**⚠️ CUDA Dependency Issue**

This project currently has hardcoded CUDA dependencies in `Cargo.toml`:
```toml
candle-core = { version = "0.8", features = ["cuda"] }
candle-nn = { version = "0.8", features = ["cuda"] }
candle-transformers = { version = "0.8", features = ["cuda"] }
```

**Impact on Claude Code Web:**
- ❌ `cargo check` - Fails (requires CUDA/nvcc)
- ❌ `cargo build` - Fails (requires CUDA/nvcc)
- ❌ `cargo test` - Fails (requires CUDA/nvcc)
- ❌ `cargo clippy` - Fails (requires CUDA/nvcc)
- ✅ Code navigation - Works
- ✅ Code review - Works
- ✅ Documentation editing - Works
- ✅ rust-analyzer - Partially works (IDE features)

**Recommended Fix:**

Make CUDA optional by creating a feature flag:

```toml
[features]
default = ["cuda"]
cuda = []

[dependencies]
candle-core = { version = "0.8", features = [], optional = true }
candle-core-cuda = { package = "candle-core", version = "0.8", features = ["cuda"], optional = true }
# ... similar for other candle crates
```

This would allow the web environment to build without CUDA while still supporting GPU acceleration in local development.

## What Works in Web Environment

Despite the CUDA limitation, Claude Code web sessions are still useful for:
- 📝 Documentation work (README, INSTALL.md, etc.)
- 🔍 Code review and analysis
- 📋 Task planning and architecture discussions
- 📄 Creating/editing configuration files
- 🎯 Issue/PR creation and management
- 📊 Project management (task tracking in `.ai/tasks/`)

## Hook Execution Mode

**Mode:** Synchronous (default)

**Pros:**
- Guarantees system dependencies are installed before session starts
- Prevents race conditions where Claude might reference uninstalled packages
- Container state is cached, so subsequent sessions start instantly

**Cons:**
- First session startup takes ~30-60 seconds
- Subsequent sessions start in <5 seconds (cached state)

To switch to async mode (faster initial startup but potential race conditions), edit `.claude/hooks/session-start.sh` and add:
```bash
echo '{"async": true, "asyncTimeout": 300000}'
```
at the beginning of the script (after the remote check).

## Files

- `hooks/session-start.sh` - Startup hook script
- `settings.json` - Claude Code configuration
- `README.md` - This file
