<div align="center">

# Hush

Local voice-to-text for Linux. Hold a key, speak, release; your words land at the cursor, in any app.

[![CI](https://github.com/andymai/hush/actions/workflows/ci.yml/badge.svg)](https://github.com/andymai/hush/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/andymai/hush?label=release)](https://github.com/andymai/hush/releases/latest)
[![Commit activity](https://img.shields.io/github/commit-activity/m/andymai/hush?label=commits%2Fmonth)](https://github.com/andymai/hush/commits/main)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust 1.88+](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org/)

**[Install](#install)** · **[Setup](#setup)** · **[Features](#features)** · **[Commands](#commands)** · **[Configure](#configure)** · **[Models](#models)** · **[Troubleshooting](#troubleshooting)**

</div>

Hush turns speech into text on any Linux desktop. Hold a key, talk, let go, and the words appear wherever your cursor is. It runs OpenAI's Whisper models on your own machine (on the GPU through Vulkan when you have one) and types the result through the kernel's input layer, so it works in terminals, browsers, editors, chat, VMs, games, and password fields, on both Wayland and X11. Audio never leaves your computer.

It is built to disappear into a keyboard-driven workflow: bind any key to it, tune the output per application, and teach it the words you use.

## Install

| Method | Command |
|---|---|
| Install script (Ubuntu, Debian, Fedora, openSUSE, Arch, and most others) | `curl -fsSL https://raw.githubusercontent.com/andymai/hush/main/install.sh \| sh` |
| Fedora (repository) | `sudo dnf copr enable andymai/hush && sudo dnf install hush` |
| Manual | Grab the `.deb`, `.rpm`, `AppImage`, or tarball from the [latest release](https://github.com/andymai/hush/releases/latest) |

The script detects your distribution, verifies the download's checksum, and installs through your package manager, so removal is a normal `apt remove hush` or `dnf remove hush`. On Arch it installs the tarball under `~/.local` until an AUR package is published. It needs only your usual graphics driver: NVIDIA, AMD, and Intel all work through Vulkan, with a CPU fallback everywhere else.

To build from source, see [INSTALL.md](INSTALL.md).

## Setup

```bash
hush setup permissions        # one polkit prompt: keyboard and uinput access, no logout
hush models download base     # ~147 MB speech model
hush daemon start             # runs in the background
```

Hold `Right Alt`, speak, release. Done.

`hush setup init` walks through model, GPU, and hotkey choices and writes `~/.config/hush/config.toml`. It picks a model for your hardware: medium on a GPU with 16 GB or more of RAM, base on a CPU. `hush install --autostart` starts the daemon at login.

## Features

- **Hold to talk, or lock hands-free.** Holding the hotkey while you speak is the default. Double-tap it to keep recording hands-free, then press once to stop; a lone tap records nothing. `Esc` discards a recording, and hands-free recording stops on its own after ten minutes, with a warning a minute before. Set `mode = "toggle"` under `[hotkey]` to lock on a single tap instead.
- **Bind any key to it.** `hush toggle`, `hush start`, `hush stop`, and `hush cancel` talk to the daemon over a socket, so any compositor keybind can drive dictation. Hyprland: `bind = , F9, exec, hush toggle`. Sway: `bindsym F9 exec hush toggle`.
- **Tone by application.** Text lands the way the focused app expects: lowercase and unpunctuated in terminals and editors, casual in chat, full sentences in mail and documents. The window title and your learned terms also prime Whisper, so names come out spelled your way. Works on X11, Hyprland, Sway, and KDE Plasma on Wayland.
- **Any language Whisper knows.** Set `language = "auto"` under `[transcription]` to detect what you speak, or pick one in the settings window. On a keyboard layout that is not US, Hush pastes rather than typing key by key, so the letters come out right.
- **Teach it your words.** Select a term Hush keeps misspelling and run `hush learn` (or `hush learn Kubernetes`); from then on it is written exactly that way. `hush paste-last` re-types the last transcript, and when insertion fails the text waits on the clipboard.
- **Command Mode.** Hold `Ctrl+Right Alt`, say what to do ("make this friendlier", "turn it into a bullet list"), release. Hush rewrites the selected text, or the last thing it typed when nothing is selected, through a local Ollama server or Anthropic, whichever `[llm]` resolves to. Only text is sent, never audio.
- **Cleaner text, automatically.** Voice commands cover "new line", "new paragraph", "undo", and "scratch that". Filler words ("um", "uh", "like") are removed. Optional LLM polishing rewrites every transcript.
- **Panel icon.** A status icon shows whether Hush is idle, recording, or working, and its menu offers the same actions the hotkeys do. Turn it off with `tray.enabled = false`.
- **A window and a doctor.** `hush settings` opens one window for what still needs setting up and every setting. `hush doctor` checks the configuration, keyboard access, model, GPU, microphone, hotkey, window detection, polishing, panel icon, autostart, and the daemon, then prints the one command that fixes each problem.

## Scope

Hush deliberately does not:

- **Send audio anywhere.** Transcription runs entirely on your machine. The only network traffic is the one-time model download and, if you turn it on, text-only LLM polishing.
- **Act as a voice assistant.** It types what you say. It does not answer questions, run commands, or wake on a phrase.
- **Run on macOS or Windows.** Linux-only, by design. It depends on `/dev/uinput`, evdev, and Linux window protocols.
- **Stream partial words.** It transcribes when you release the key, not live as you speak.
- **Replace your editor's dictation UI.** There is no correction overlay or transcript editor; text goes straight to the cursor.

## Commands

Everyday dictation:

| Command | Does |
|---|---|
| `hush toggle` | Start or stop recording over the daemon socket |
| `hush start` / `hush stop` / `hush cancel` | Start, stop, or discard a recording |
| `hush listen` | Run a dictation session attached to the terminal |
| `hush record [-d secs]` | Record for N seconds (default 30) and transcribe |
| `hush learn [words…]` | Add the selection, or the given words, to the vocabulary |
| `hush paste-last` | Re-type the last transcript |
| `hush settings` | Open the setup and settings window |
| `hush doctor` | Check every component and print the fix for each problem |

Daemon:

| Command | Does |
|---|---|
| `hush daemon start [--foreground]` | Start the background daemon |
| `hush daemon stop` / `restart` / `status` | Stop, restart, or query the daemon |

Models:

| Command | Does |
|---|---|
| `hush models list [--downloaded] [--details]` | List models and what is downloaded |
| `hush models download <size> [-f]` | Download a model |
| `hush models set <size>` | Select the active model |
| `hush models remove <size>` | Delete a downloaded model |
| `hush models info` / `verify [size] [--fix]` | Show stats, or re-check model files |

Setup:

| Command | Does |
|---|---|
| `hush setup permissions [--check] [--print]` | Grant keyboard and uinput access (one polkit prompt) |
| `hush setup init [--defaults] [-f]` | Guided config: model, GPU, hotkey |
| `hush setup wizard [--auto]` | Full interactive setup |
| `hush setup audio` / `hotkeys` / `uinput` / `diagnose-uinput` | Configure or diagnose one subsystem |

Diagnostics and lifecycle:

| Command | Does |
|---|---|
| `hush status [--full] [--devices] [--config]` | System diagnostics; `--devices` lists microphones |
| `hush test audio\|transcription\|text-insertion\|hotkeys\|window\|pipeline\|all` | Targeted self-tests |
| `hush install [--autostart] [--desktop] [--system]` | Set up autostart and the desktop entry |
| `hush uninstall [--autostart] [--desktop] [--system]` | Reverse the above |

Global flags: `-c, --config-file <path>` (or `HUSH_CONFIG`) points at another config file; `-v`, `-vv`, `-vvv` set INFO, DEBUG, and TRACE logging; `--no-notifications` silences desktop notifications. `hush --help` lists everything.

## Configure

`~/.config/hush/config.toml` only needs the keys you change; every default lives in [`config/default.toml`](config/default.toml).

| Key | Meaning | Default |
|---|---|---|
| `transcription.model_size` | tiny, base, small, medium, large, large-v2, large-v3 | base |
| `transcription.language` | Two-letter code, or `auto` to detect what you speak | en |
| `transcription.use_gpu` | Use the GPU backend the binary was built with | true |
| `transcription.context_prompt` | Prime Whisper with the focused window's title and learned terms | true |
| `hotkey.combination` | Key or button: `RightAlt`, `RightCtrl`, `F13`, `Mouse4`, or a chord like `Ctrl+Shift+Space` | RightAlt |
| `hotkey.mode` | `hold` (double-tap locks hands-free) or `toggle` (a tap locks) | hold |
| `hotkey.backend` | `auto`, `evdev`, or `x11` | auto |
| `hotkey.exclusive` | Grab the hotkey's device so applications never see the key or button | false |
| `hotkey.cancel` | Key that discards a recording in progress; `""` disables it | Escape |
| `hotkey.tap_ms` | Presses shorter than this are taps rather than holds | 300 |
| `hotkey.paste_last` | Chord that re-types the last transcript, such as `Shift+RightAlt` | unbound |
| `hotkey.learn` | Chord that adds the selected text to the vocabulary | unbound |
| `hotkey.command` | Command Mode chord; `""` disables it | Ctrl+RightAlt |
| `llm.provider` | `auto` (Ollama when reachable, else Anthropic when a key exists), `ollama`, `anthropic`, or `none` | auto |
| `llm.ollama_url`, `llm.ollama_model` | The local Ollama server and model | localhost:11434, llama3.2 |
| `llm.anthropic_model` | Anthropic model; the key comes from `ANTHROPIC_API_KEY` | claude-haiku-4-5 |
| `llm.polish` | Rewrite every transcript with the LLM; Command Mode works either way | true |
| `profiles.enabled` | Adapt output to the focused application; per-profile keys add window classes | true |
| `audio.max_recording_secs` | Stop and transcribe after this long, with a warning a minute before; 0 disables | 600 |
| `audio.device` | Microphone name from `hush status --devices` | system default |
| `feedback.audio_enabled` | Tones on start, stop, cancel, and the cap warning | true |
| `insertion.method` | `auto` (type, then paste what uinput cannot type), `uinput`, or `clipboard` | auto |
| `tray.enabled` | Status icon on the desktop panel | true |

Point at a different file with `hush -c other.toml …` or `HUSH_CONFIG=other.toml`.

## Models

| Model | Download |
|---|---|
| tiny | 78 MB |
| **base** (default) | **147 MB** |
| small | 488 MB |
| medium | 1.5 GB |
| large, large-v2, large-v3 | 3.1 GB each |

Larger models are more accurate and slower. `hush setup init` picks one for your hardware: medium on a GPU with 16 GB or more of RAM, small on a smaller-RAM GPU, base on a CPU. Manage them with `hush models list`, `hush models download <size>`, and `hush models set <size>`. Downloads come from the whisper.cpp catalogue and are checksum-verified.

## Troubleshooting

```bash
hush doctor                      # checks everything and prints the fix for each problem
hush status --full               # every component, with the device ggml selected
hush setup permissions --check   # keyboard and uinput access
hush status --devices            # microphones
hush test audio -d 3             # record and play back
hush -vv daemon start --foreground
```

If `hush setup permissions` cannot run where you are (a container, an SSH session), `hush setup permissions --print` prints the udev rule to apply as root.

## Privacy and security

Everything runs locally. The only network traffic is the one-time model download, and text-only LLM polishing if you turn it on (never audio). Hush reads keyboard events and types through `/dev/input` and `/dev/uinput`; one udev rule grants the logged-in user access to those for the length of the session. Details in [SECURITY.md](SECURITY.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and the [Code of Conduct](CODE_OF_CONDUCT.md). Hush is MIT licensed and built on [whisper.cpp](https://github.com/ggerganov/whisper.cpp) through [whisper-rs](https://github.com/tazz4843/whisper-rs). Release notes live in [CHANGELOG.md](CHANGELOG.md).

## License

[MIT](LICENSE).
