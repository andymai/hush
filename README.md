# Hush

**Local voice-to-text for Linux.** Hold a key, speak, release. Your words appear at the cursor, in any app.

[![CI](https://github.com/andymai/hush/actions/workflows/ci.yml/badge.svg)](https://github.com/andymai/hush/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Hush runs OpenAI's Whisper models on your machine, on the GPU through Vulkan when you have one, and types the result through the kernel's input layer. That means it works in terminals, browsers, editors, VMs, games, and password fields, on Wayland and X11. Audio never leaves your computer.

## Install

| Distro | Command |
|---|---|
| Ubuntu, Debian, Fedora, openSUSE, anything else | `curl -fsSL https://raw.githubusercontent.com/andymai/hush/main/install.sh \| sh` |
| Arch | `paru -S hush-bin` |
| Manual | Grab the `.deb`, `.rpm`, AppImage, or tarball from the [latest release](https://github.com/andymai/hush/releases/latest) |

The installer picks the package for your distro, verifies its checksum, and installs it through your package manager. It needs only your normal graphics driver: NVIDIA, AMD, and Intel all work through Vulkan, and everything falls back to the CPU.

To build from source instead, see [INSTALL.md](INSTALL.md).

## Set up

```bash
hush setup permissions        # one polkit prompt: keyboard and uinput access, no logout
hush models download base     # ~145 MB; large-v3-turbo is the accurate choice on a GPU
hush daemon start             # runs in the background
```

Hold `Ctrl+Shift+Space`, speak, release. Done.

`hush setup init` walks through model, GPU, and hotkey choices and writes `~/.config/hush/config.toml`. `hush install --autostart` starts the daemon at login.

## Use

- **Hold to talk** is the default. Set `mode = "toggle"` under `[hotkey]` for press-to-start, press-to-stop.
- **Any keybind can drive it.** `hush toggle`, `hush start`, `hush stop`, and `hush cancel` talk to the daemon over its socket. Hyprland: `bind = , F9, exec, hush toggle`. Sway: `bindsym F9 exec hush toggle`.
- **Voice commands**: "new line", "new paragraph", "undo" or "scratch that".
- **Filler words** ("um", "uh", "like") are removed. Optional polishing through Claude when `ANTHROPIC_API_KEY` is set in `~/.config/hush/.env` or the environment; only text is sent, never audio.
- `hush daemon status`, `hush daemon stop`, `hush listen` (same session, attached to the terminal).

## Configure

`~/.config/hush/config.toml` only needs the keys you change; defaults come from [`config/default.toml`](config/default.toml).

| Key | Meaning | Default |
|---|---|---|
| `transcription.model_size` | tiny, base, small, medium, large, large-v2, large-v3 | base |
| `transcription.language` | Whisper language code, or `auto` | en |
| `transcription.use_gpu` | Use the GPU backend the binary was built with | true |
| `hotkey.combination` | The key | Ctrl+Shift+Space |
| `hotkey.mode` | `hold` or `toggle` | hold |
| `hotkey.backend` | `auto`, `evdev`, or `x11` | auto |
| `insertion.method` | `auto` (type, paste what uinput cannot type), `uinput`, or `clipboard` | auto |
| `audio.device` | Microphone name from `hush status --devices` | system default |

`hush -c other.toml …` or `HUSH_CONFIG=other.toml` point at a different file.

## Models

| Model | Download | Speed on GPU | Speed on CPU |
|---|---|---|---|
| tiny | 75 MB | instant | ~2 s |
| **base** | **145 MB** | **instant** | **~5 s** |
| small | 466 MB | <1 s | ~10 s |
| medium | 1.5 GB | ~1 s | ~20 s |
| large-v3 | 2.9 GB | ~2 s | ~40 s |

`hush models list`, `hush models download <size>`, `hush models set <size>`. Downloads come from the whisper.cpp catalogue and are checksum-verified.

## Troubleshooting

```bash
hush status --full               # every component, with the device ggml found
hush setup permissions --check   # keyboard and uinput access
hush status --devices            # microphones
hush test audio --duration 3
hush -vv daemon start --foreground
```

If `hush setup permissions` cannot be run where you are (a container, an SSH session), `hush setup permissions --print` prints the udev rule to apply as root.

## Privacy and security

Everything runs locally. The only network traffic is the model download, and text polishing if you turn it on. Hush reads keyboard events and types through `/dev/input` and `/dev/uinput`; one udev rule grants the logged-in user access to those for the length of the session. Details in [SECURITY.md](SECURITY.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Hush is MIT licensed and built on [whisper.cpp](https://github.com/ggerganov/whisper.cpp) through [whisper-rs](https://github.com/tazz4843/whisper-rs).
