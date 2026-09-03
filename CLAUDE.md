# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Hush is a local voice-to-text application for Linux developers. It runs OpenAI's Whisper models locally with optional CUDA GPU acceleration. Hold a hotkey, speak, release—text appears at cursor via kernel-level uinput (works in VMs, games, secure contexts).

## Build Commands

```bash
make release           # GPU build with Vulkan
make release-cuda      # GPU build with CUDA
make release-cpu       # CPU-only build
make build             # Debug build
make check             # Fast type checking
cargo nextest run --lib --bins  # Run tests
cargo clippy --all-targets -- -D warnings  # Lint
cargo fmt              # Format
```

`npm install` once to enable the husky hooks: commit-msg runs commitlint, pre-commit runs fmt, clippy, taplo, and cargo-machete.

The Makefile creates a `./hush` symlink to the built binary.

`make release` builds the Vulkan backend and needs `glslc` plus the Vulkan
headers at build time; at runtime only the graphics driver is required, on any
vendor. `make release-cuda` needs the CUDA toolkit (`nvcc`), not just the NVIDIA
driver. On a host that ships driver-only (Fedora Atomic and similar), run it
inside a CUDA container; `cuda-preflight` fails early with instructions when
`nvcc` is absent. CUDA targets also drop sccache from `PATH`, because ggml's
CMake auto-detects it and deadlocks on the CUDA kernel fan-out.

## Running

```bash
./hush models download base   # Download Whisper model (~145MB)
./hush setup permissions      # udev rule for keyboard and uinput access
./hush daemon start           # Background daemon; hold Ctrl+Shift+Space to dictate
./hush toggle                 # Start or stop recording over the daemon socket
./hush listen                 # Same session attached to the terminal
./hush record --duration 5    # Record for N seconds
./hush status --full          # System diagnostics
```

## Architecture

### Core Trait System (`src/core/`)

The codebase uses trait-based dependency injection for testability:

- **`traits.rs`** - Core abstractions: `AudioSource`, `Transcriber`, `TextOutput`, `InputTrigger`, `ConfigProvider`
- **`types.rs`** - Type-safe newtypes with validation: `SampleRate` (8-48kHz), `Channels` (1-8), `BufferSize` (power-of-2)
- **`error.rs`** - Structured error hierarchy using `thiserror`
- **`mocks.rs`** - Mock implementations for testing

### Module Organization

```
src/
├── core/           # Traits, types, errors, state machine
├── adapters/       # Bridge legacy code to traits (cpal, whisper, x11, hotkey)
├── application/    # HushApp orchestrator with builder pattern
├── cli/            # Clap-based CLI, command handlers in cli/commands/
├── audio/          # CPAL-based audio capture
├── transcription/  # whisper.cpp via whisper-rs (whisper.rs), model catalogue and downloads (models.rs), GPU detection (device.rs)
├── text_processing/# Pipeline: filler removal → vocabulary → entities → LLM polish
├── text/           # uinput_keyboard.rs for kernel-level text insertion
├── hotkey/         # Hotkey combination parser, evdev backend, X11 fallback
├── overlay/        # egui floating window UI
├── config/         # TOML settings
└── logging.rs      # Structured tracing with request correlation
```

### Text Processing Pipeline (`src/text_processing/`)

Transcribed text flows through: filler word removal → vocabulary expansion → entity capitalization → optional LLM polishing. Each step has its own module.

## Code Style

- **No `.unwrap()` in production code** - use `?` operator with proper error handling
- Async-first with tokio; use `async_trait` for trait methods
- Error context via `anyhow::Context`
- Type-safe newtypes prevent mixing parameters at compile-time

## Pinned Dependencies

These versions are constrained:
- dirs 5.0 (6.0 breaking API)
- egui 0.29 (must match egui_overlay 0.9)
- audioadapter-buffers 2.0 (must match the version rubato depends on; a mismatch
  makes `SequentialSlice` fail rubato's `Adapter`/`AdapterMut` bounds)
- rodio needs the `playback` feature explicitly, since it uses
  `default-features = false` and `playback` gates `OutputStream`

## Configuration

- **`config/default.toml`** - Compiled-in defaults (16kHz sample rate, "base" model, Ctrl+Shift+Space hotkey). User overrides live in `$XDG_CONFIG_HOME/hush/config.toml`; `--config-file` or `HUSH_CONFIG` point elsewhere. Models live in `$XDG_DATA_HOME/hush/models`.
- **`.env`** - LLM keys: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`
- **Logging verbosity**: `-v` (INFO), `-vv` (DEBUG), `-vvv` (TRACE)

## Testing

Unit tests are embedded in modules via `#[cfg(test)] mod tests`. Use mock implementations from `core/mocks.rs` for trait testing. `.github/workflows/ci.yml` runs fmt, clippy, nextest, MSRV (1.88), rustdoc, cargo-deny, cargo-audit, cargo-machete, and taplo as parallel jobs behind a required `CI Pass` check; `osv-scan.yml` scans both lockfiles. Doctests are excluded until the module-level examples compile again (#91). CUDA builds are not covered by CI, so verify those locally.

## Commits and Releases

Commit messages and PR titles follow Conventional Commits (commitlint locally, `pr-title.yml` on PRs). release-please opens the release PR and bumps `Cargo.toml` and `CHANGELOG.md`; never edit those by hand. Branch names are `<type>/<kebab-description>`.

## Platform

Linux-only. Uses `#[cfg(target_os = "linux")]` for platform-specific code. Hotkeys read `/dev/input` through evdev on any session type, with an X11 grab as fallback; uinput for keyboard emulation; X11 for window detection.
