# Hush UX Modernization Design

**Date**: 2026-02-16
**Status**: Approved
**Approach**: Layered Milestones (v1.1–v1.4)

## Context

Compared to projects like HyprVoice, Hush has strong local-first foundations (CUDA, uinput, multi-stage text pipeline) but lags in install friction, onboarding, daemon architecture, and Wayland support. This design modernizes all four areas through incremental releases, each independently shippable and valuable.

## Requirements

- **Packaging**: Universal binary + COPR (Fedora) + AUR (Arch)
- **Onboarding**: Minimal steps from install to first dictation
- **Daemon**: Background service with IPC, no terminal required
- **Wayland**: Full parity — PipeWire audio, wtype/ydotool insertion, layer-shell overlay
- **Cloud transcription**: Optional Groq + OpenAI backends alongside local Whisper default

---

## Milestone 1: Daemon + IPC (v1.1)

### Architecture

Split the foreground `hush listen` into a background daemon with IPC control:

```
┌──────────────────────┐       Unix Socket        ┌──────────────────┐
│   hush daemon start  │◄────────────────────────► │  hush toggle     │
│                      │  /run/user/UID/hush.sock  │  hush stop       │
│  - Hotkey listener   │                           │  hush status     │
│  - Audio capture     │                           └──────────────────┘
│  - Transcription     │
│  - Text insertion    │       ┌──────────────────┐
│  - Overlay           │◄──────│  Compositor bind │
│  - IPC server        │       └──────────────────┘
└──────────────────────┘
```

### IPC Protocol

JSON-over-Unix-socket, newline-delimited:

```json
// Client → Daemon
{"command": "toggle"}
{"command": "cancel"}
{"command": "status"}
{"command": "stop"}
{"command": "reload"}

// Daemon → Client
{"state": "idle"}
{"state": "recording", "elapsed_secs": 2.3}
{"state": "transcribing"}
{"state": "ready", "text": "Hello world"}
{"error": "No audio captured"}
```

### CLI Changes

```
hush daemon start [--foreground]   # Start background daemon
hush daemon stop                   # Graceful shutdown
hush daemon restart                # Stop + start

hush toggle                        # Toggle recording (talks to daemon)
hush cancel                        # Cancel current recording
hush status                        # Query daemon state

hush listen                        # DEPRECATED → daemon start --foreground
```

### Key Decisions

- **Single binary** — no separate client/server. `hush daemon start` forks to background.
- **PID file** at `/run/user/UID/hush.pid` for singleton enforcement.
- **Hotkey stays built-in** — daemon owns the hotkey listener. `hush toggle` is an additional trigger, not a replacement.
- **Overlay stays in-process** — daemon owns the overlay window.
- **Backward compat** — `hush listen` becomes alias for `hush daemon start --foreground` with deprecation notice.

---

## Milestone 2: Packaging + Zero-Config Install (v1.2)

### Distribution

- **Universal binary**: GitHub Releases with two tarballs per tag:
  - `hush-x86_64-linux-cuda.tar.gz` — dynamically links CUDA
  - `hush-x86_64-linux-cpu.tar.gz` — fully static (musl)
- **COPR**: Fedora RPM spec (`dnf copr enable andymai/hush && dnf install hush`)
- **AUR**: `hush-bin` (binary) + `hush-git` (source)

### Zero-Config First Run

`hush` with no args and no config triggers the first-run wizard:

```
Welcome to Hush! Let's get you set up.

[1/3] Downloading Whisper model (base, ~145MB)...
      ████████████████████░░░░ 78%  112MB/145MB

[2/3] Setting up text insertion...
      ✅ uinput available
      ⚠️  You need to be in the 'input' group.
      Run: sudo usermod -a -G input $USER
      Then log out and back in.

      [Press Enter after completing, or 's' to skip]

[3/3] Detecting GPU...
      ✅ NVIDIA RTX 3080 (CUDA 12.2)

✅ Setup complete! Starting Hush daemon...

Hold Ctrl+Shift+Space to dictate. Run 'hush stop' to quit.
```

- `hush` with existing config → starts daemon
- Progress bar via `indicatif` crate
- Wizard stores state for resume after interruption (e.g., logout for group change)
- `--no-wizard` flag for scripted installs

### Systemd User Service (Optional)

Shipped but not enabled by default:

```ini
[Unit]
Description=Hush Voice-to-Text Daemon

[Service]
ExecStart=/usr/bin/hush daemon start --foreground
Restart=on-failure

[Install]
WantedBy=default.target
```

`hush install --systemd` enables it.

### CI Pipeline

Tag-triggered GitHub Actions:
1. Build CPU binary (musl static)
2. Build CUDA binary
3. Create GitHub Release with tarballs
4. Trigger COPR build
5. Update AUR PKGBUILD

---

## Milestone 3: Wayland Support (v1.3)

### Runtime Detection

`$XDG_SESSION_TYPE` or `$WAYLAND_DISPLAY` determines which backend to use. Both X11 and Wayland enabled by default via Cargo features.

### Four Subsystems

**1. Audio Capture** — CPAL already supports PipeWire as a backend. Likely zero code changes. If needed, add `pipewire-rs` as an alternative `AudioSource`.

**2. Text Insertion** — New trait implementations with runtime fallback chain:

| Backend | Works On | Mechanism |
|---------|----------|-----------|
| `UinputInserter` (existing) | Both | Kernel-level |
| `WtypeInserter` (new) | Wayland | Spawns `wtype` |
| `YdotoolInserter` (new) | Both | Spawns `ydotool` |
| `ClipboardInserter` (new) | Both | `wl-copy`/`xclip` + paste + clipboard restore |

Fallback: uinput → wtype/ydotool → clipboard. Configurable:

```toml
[text_insertion]
method = "auto"  # or "uinput", "wtype", "ydotool", "clipboard"
```

**3. Overlay** — Migrate from `egui_overlay` (X11-only) to **`iced` + `iced-layershell`**. Iced is Rust-native, supports Wayland layer-shell for always-on-top overlay positioning. On X11, Iced uses standard window hints. The overlay is small enough (pill + waveform) that migration is tractable.

**4. Window Detection** — Make optional. On Wayland, compositor-specific (Hyprland: `hyprctl`, Sway: `swaymsg`). GNOME doesn't expose it. Skip gracefully when unavailable.

### Feature Flags

```toml
[features]
default = ["x11", "wayland"]
x11 = ["x11rb"]
wayland = ["wayland-client", "iced", "iced-layershell"]
cuda = ["whisper-rs/cuda"]
```

---

## Milestone 4: Cloud Transcription (v1.4)

### Providers

| Provider | Model | Rationale |
|----------|-------|-----------|
| Groq | whisper-large-v3-turbo | Free tier, ~10x realtime, ideal for no-GPU users |
| OpenAI | whisper-1 | Highest quality, widely adopted |

### Configuration

```toml
[transcription]
backend = "local"  # "local" (default), "groq", "openai"
model_size = "base"
use_cuda = true

[transcription.cloud]
api_key_env = "GROQ_API_KEY"
timeout_secs = 10
```

### Key Decisions

- **No auto-fallback** to cloud — sending audio externally must be an explicit user choice.
- **Audio encoding** — WAV/FLAC from memory, no temp files.
- **API keys via env vars only** — consistent with existing `.env` pattern.
- **HTTP via `reqwest`** — no provider-specific SDKs.
- **No streaming in v1.4** — batch upload only. Streaming adds complexity for marginal gains on short utterances.
- **Wizard integration** — first-run wizard gains a transcription backend question.

---

## Risk Summary

| Milestone | Biggest Risk | Mitigation |
|-----------|-------------|------------|
| v1.1 Daemon | State management across IPC boundary | Reuse existing state machine, thin IPC layer |
| v1.2 Packaging | CUDA static linking for universal binary | Dynamic CUDA linking, static everything else |
| v1.3 Wayland | Overlay migration from egui to iced | Small overlay surface, prototype early |
| v1.4 Cloud | Low risk | Simplest milestone, clean trait boundary |
