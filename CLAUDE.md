# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Hush is a local voice-to-text application for Linux developers. It runs OpenAI's Whisper models, and the opt-in Parakeet model, locally through transcribe.cpp. The GPU backend is Vulkan or CUDA, and the CPU is the fallback. Hold a hotkey, speak, release. The text appears at the cursor through kernel-level uinput, so it works in VMs, games, and secure contexts.

## Build Commands

```bash
make release           # GPU build with Vulkan
make release-cuda      # GPU build with CUDA
make release-cpu       # CPU-only build
make production        # Vulkan build with full LTO (slow)
make install           # Vulkan build into ~/.cargo/bin and ~/.cargo/lib/hush (PREFIX=...)
make build             # Debug build
make check             # Fast type checking
cargo nextest run --lib --bins  # Run tests
cargo clippy --all-targets -- -D warnings  # Lint
cargo fmt              # Format
```

`npm install` once to enable the husky hooks: commit-msg runs commitlint, pre-commit runs fmt, clippy, taplo, and cargo-machete.

The Makefile creates a `./hush` symlink to the built binary.

transcribe-cpp uses its `dynamic-backends` feature in every build.
`libtranscribe` is a shared library. It loads each ggml backend at run time as
a module: a CPU module for each x86 level, plus the Vulkan or CUDA module.
`build.rs` copies them to `target/<profile>/lib/hush`. It sets the RUNPATH
`$ORIGIN/../lib/hush` (packages, tests) plus `$ORIGIN/lib/hush` (the binary in
`target/<profile>`). `HUSH_PACKAGE=1` (release job, flake) leaves only the
package path.

`device::init_backends()` must run before any device query or model load. A
binary without `lib/hush` next to its `bin` folder does not start, so use
`make install`, not `cargo install`.

`make release` builds the Vulkan backend. It needs `glslc`, the Vulkan headers,
and `spirv-headers` at build time, and only the graphics driver at runtime, on
any vendor. `make release-cuda` needs the CUDA toolkit (`nvcc`), not just the NVIDIA
driver. On a host that ships driver-only (Fedora Atomic and similar), run it
inside a CUDA container; `cuda-preflight` fails early with instructions when
`nvcc` is absent. CUDA targets also drop sccache from `PATH`, because ggml's
CMake auto-detects it and deadlocks on the CUDA kernel fan-out.

The Makefile and CI set `TRANSCRIBE_CMAKE_ARGS=-DTRANSCRIBE_USE_SYSTEM_BLAS=OFF`.
Without it, transcribe-cpp-sys links OpenBLAS when the system has the library, and
`build.rs` warns (and fails a `HUSH_PACKAGE` build). Extra ggml flags go in the
same variable, not in `GGML_*` variables. The release job fails when code that
always loads has AVX-512 instructions, so no flag may raise the CPU floor, such
as `-march=native`. Only the AVX-512 CPU modules, and Rust functions that a runtime CPU check selects, may use it.

## Running

```bash
./hush models download base   # Download Whisper model (~147 MB)
./hush setup permissions      # udev rule for keyboard and uinput access
./hush daemon start           # Background daemon; hold RightAlt to dictate, double-tap to lock
./hush toggle                 # Start or stop recording over the daemon socket
./hush listen                 # Same session attached to the terminal
./hush record --duration 5    # Record for N seconds
./hush settings               # Setup and settings window
./hush doctor                 # Check everything and print the fix for each problem
./hush status --full          # System diagnostics
scripts/screenshots.sh target/debug/hush docs/screenshots/settings  # Redraw the settings screenshots under a headless Weston
```

## Architecture

### Core Trait System (`src/core/`)

The codebase uses trait-based dependency injection for testability:

- **`traits.rs`** - Core abstractions: `AudioSource`, `Transcriber`, `TextOutput`, `InputTrigger`
- **`types.rs`** - Type-safe newtypes with validation: `SampleRate` (8-48kHz), `Channels` (1-8), `BufferSize` (power-of-2)
- **`error.rs`** - Structured error hierarchy using `thiserror`
- **`mocks.rs`** - Mock implementations for testing

### Module Organization

```
src/
├── core/           # Traits, types, errors, state machine
├── adapters/       # Bridge legacy code to traits (cpal, hotkey, text inserter, whisper)
├── application/    # HushApp orchestrator with builder pattern
├── cli/            # Clap-based CLI, command handlers in cli/commands/, exit codes (exit.rs)
├── ipc/            # Unix socket between the daemon and `hush toggle`, `stop`, and the other client commands, plus the PID lock
├── audio/          # CPAL capture, tones, kept audio (FLAC), hands-free part cutting
├── transcription/  # Whisper and opt-in Parakeet via transcribe-cpp (whisper.rs), model catalogue and downloads (models.rs), backend modules and GPU detection (device.rs)
├── text_processing/# Pipeline: filler removal → vocabulary → entities → LLM polish
├── text/           # uinput typing through the XKB keymap (xkb.rs, typing.rs, layout.rs), clipboard paste fallback, window detection (X11, Hyprland, Sway, KWin)
├── hotkey/         # Hotkey combination parser, evdev backend, X11 fallback, gesture detector, menu mask
├── overlay/        # egui floating window UI
├── gui/            # eframe window (`hush settings`): own title bar and sidebar (chrome.rs), controls (widgets.rs), save on each change and one debounced daemon restart (autosave.rs), palette, fonts, and sizes (theme.rs), pages in gui/pages, evdev key capture, mic level meter
├── tray/           # StatusNotifierItem panel icon and the Hush mark
├── config/         # TOML settings, API keys from the environment and .env, `config export` and `import` (transfer.rs)
├── permissions.rs  # The udev rule that grants keyboard and uinput access
├── updates.rs      # The opt-in release check; a package-manager install never runs it
├── desktop.rs      # The .desktop and autostart entries
└── logging.rs      # Structured tracing with request correlation
```

Outside `src/`: `packaging/` holds the desktop file, icon, udev rule, systemd user unit, AppRun, and the spec and PKGBUILD templates. `scripts/e2e/` and `tests/e2e/` hold the end-to-end harness. `scripts/screenshots.sh` draws the settings screenshots.

### Text Processing Pipeline (`src/text_processing/`)

Transcribed text flows through: filler word removal → vocabulary expansion → entity capitalization → optional LLM polishing. Each step has its own module.

## Code Style

- **No `.unwrap()` in production code** - use `?` operator with proper error handling; `.expect()` only for regex literals
- Async-first with tokio; use `async_trait` for trait methods
- Error context via `anyhow::Context`
- Type-safe newtypes prevent mixing parameters at compile-time

## Pinned Dependencies

These constraints hold:
- egui and eframe 0.29 (egui_overlay 0.9 requires egui ^0.29; Dependabot ignores their minor and major updates)
- `SequentialSlice` comes from rubato's `audioadapter_buffers` re-export, not a
  direct dependency, so its `Adapter`/`AdapterMut` impls always match rubato's
- rodio needs the `playback` feature explicitly, since it uses
  `default-features = false` and `playback` gates `OutputStream`

## Configuration

- **`config/default.toml`** - Compiled-in defaults (16kHz sample rate, "base" model, RightAlt hotkey, Escape cancels, Ctrl+RightAlt for Command Mode). User overrides live in `$XDG_CONFIG_HOME/hush/config.toml`; `--config-file` or `HUSH_CONFIG` point elsewhere. Models live in `$XDG_DATA_HOME/hush/models`.
- **`.env`** - API keys, never in `config.toml`: `ANTHROPIC_API_KEY` for the Anthropic provider, `GROQ_API_KEY` and `OPENAI_API_KEY` for cloud transcription, and the variables that `transcription.server_key_env` and `llm.server_key_env` name. Hush reads `~/.config/hush/.env` and `./.env`, and a variable in the environment wins. `[llm]` in the config picks Ollama, Anthropic, an OpenAI-compatible server, or none
- **Logging verbosity**: `-v` (INFO), `-vv` (DEBUG), `-vvv` (TRACE)

## Testing

Unit tests live in modules, in `#[cfg(test)] mod tests`. Use mock implementations from `core/mocks.rs` for trait testing. `.github/workflows/ci.yml` runs these jobs in parallel: fmt, clippy (also with the `vulkan` feature), nextest, MSRV (1.95), rustdoc, cargo-deny, cargo-audit, cargo-machete, and taplo. A required `CI Pass` check gates them. `osv-scan.yml` scans both lockfiles. Doctests run as a separate step, so examples in `///` comments must compile.

`e2e.yml` runs `scripts/e2e/run.sh` nightly, and it is not a required check. CI does not build CUDA, so test those builds locally.

## Commits and Releases

Commit messages and PR titles follow Conventional Commits (commitlint locally, `pr-title.yml` on PRs). release-please opens the release PR and bumps `Cargo.toml` and `CHANGELOG.md`; never edit those by hand. Branch names are `<type>/<kebab-description>`.

A published release runs `.github/workflows/release.yml`. It builds the `.deb`, `.rpm`, tarball, and AppImage with `--features vulkan`, attests them, and uploads them with `SHA256SUMS`. It then submits a source RPM that repackages the tarball (`packaging/hush.spec.template`) to COPR. The AUR job (`packaging/aur/PKGBUILD.template`) runs only when the repository variable `AUR_PUBLISH` is `true`. `install.sh` downloads the release files, and `flake.nix` builds from source.

## Platform

Linux-only. Uses `#[cfg(target_os = "linux")]` for platform-specific code. Hotkeys read `/dev/input` through evdev on any session type, with an X11 grab as fallback. Keyboard emulation uses uinput. Window detection uses X11, Hyprland, Sway, or KWin.
