# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Hush is a local voice-to-text application for Linux developers. It runs OpenAI's Whisper models locally with optional CUDA GPU acceleration. Hold a hotkey, speak, release—text appears at cursor via kernel-level uinput (works in VMs, games, secure contexts).

## Build Commands

```bash
make release           # GPU build with CUDA
make release-cpu       # CPU-only build
make build             # Debug build
make check             # Fast type checking
cargo test             # Run tests
cargo clippy -- -D warnings  # Lint
cargo fmt              # Format
```

The Makefile creates a `./hush` symlink to the built binary.

## Running

```bash
./hush models download base   # Download Whisper model (~145MB)
./hush setup uinput --quick   # Enable kernel-level text insertion
./hush listen                 # Hold Ctrl+Alt+V to dictate
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
├── transcription/  # Whisper integration (whisper.rs 842 LOC, models.rs for downloads)
├── text_processing/# Pipeline: filler removal → vocabulary → entities → LLM polish
├── text/           # uinput_keyboard.rs for kernel-level text insertion
├── hotkey/         # Global hotkey listener
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

These versions are pinned due to breaking changes in newer releases:
- cpal 0.15 (0.16 breaking)
- enigo 0.2 (0.6 major breaking)
- global-hotkey 0.6 (0.7 may break)
- dirs 5.0 (6.0 breaking API)
- egui 0.23 (must match egui_overlay 0.5)

## Configuration

- **`config/default.toml`** - App defaults (16kHz sample rate, "base" model, Ctrl+Alt+V hotkey)
- **`.env`** - LLM keys: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`
- **Logging verbosity**: `-v` (INFO), `-vv` (DEBUG), `-vvv` (TRACE)

## Testing

Unit tests are embedded in modules via `#[cfg(test)] mod tests`. Use mock implementations from `core/mocks.rs` for trait testing. CI runs format check, clippy, build, test, and cargo-audit.

## Platform

Linux-only. Uses `#[cfg(target_os = "linux")]` for platform-specific code. X11 for window detection, uinput for keyboard emulation.
