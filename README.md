<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/brand/hush-lockup-dark.svg">
  <img alt="Hush" src="docs/brand/hush-lockup-light.svg" height="72">
</picture>

**Local voice-to-text for Linux.**<br>
Hold a key, speak, release; your words land at the cursor, in any app.

[![CI](https://github.com/andymai/hush/actions/workflows/ci.yml/badge.svg)](https://github.com/andymai/hush/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/andymai/hush?label=release)](https://github.com/andymai/hush/releases/latest)
[![Commit activity](https://img.shields.io/github/commit-activity/m/andymai/hush?label=commits%2Fmonth)](https://github.com/andymai/hush/commits/main)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust 1.95+](https://img.shields.io/badge/rust-1.95%2B-orange.svg)](https://www.rust-lang.org/)

**[Features](#features)** · **[Install](#install)** · **[Setup](#setup)** · **[Docs](#docs)** · **[Contributing](CONTRIBUTING.md)**

<br>

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/settings/overview.png">
  <img src="docs/screenshots/settings/overview-light.png" alt="The Overview page of the Hush settings window" width="720">
</picture>

</div>

Hush runs OpenAI's Whisper models, or the opt-in NVIDIA Parakeet model, on your own computer, on the GPU through Vulkan when you have one. It types the result through the kernel's input layer. This works in terminals, browsers, editors, chat, VMs, games, and password fields, on Wayland and X11. Audio stays on your computer unless you choose a cloud service for transcription.

## Features

### Dictation that stays out of your way

- **Hold to talk, or lock hands-free.** Hold `Right Alt` while you speak, or double-tap it to keep recording, then press once to stop. `Esc` discards a recording, and hands-free recording stops on its own after ten minutes.
- **Long recordings in parts.** A hands-free recording transcribes in parts while you speak, cut at the pauses. When you stop, only the last part still needs transcription.
- **Speak at the tone.** Hush plays the start tone when the microphone first sends audio, not when you press the key. The tone tells you that Hush hears you.
- **Any key, any script.** `hush toggle`, `hush start`, `hush stop`, and `hush cancel` talk to the daemon, so a compositor keybind or a script can drive dictation.

### Text that fits the application

- **Tone by application.** Lowercase in terminals and editors, casual in chat, full sentences in mail and documents. In a terminal, a line break becomes a space, so dictated text cannot run a command.
- **Writing styles.** Give a style its own dictation key, set a default style for an application, or rewrite the selected text with a style.
- **Command Mode.** Hold `Ctrl+Right Alt`, say what to do ("make this friendlier"), and release. Hush rewrites the selected text with a local or cloud language model, which gets text and never audio.
- **Cleaner text.** Voice commands such as "new line" and "scratch that", no filler words, and an optional LLM polish of every transcript.
- **Your own words.** Select a term that Hush misspells and run `hush learn`. From then on, Hush writes it your way.

### Every language and layout

- **Any language that Whisper knows.** Detect what you speak, or limit the detection to the languages that you use. Give each language its own key.
- **Every keyboard layout.** Hush types key by key through the XKB keymap of the active layout, with accents and Caps Lock. When a take has a character that no key types, such as an emoji, Hush pastes the take.

### Nothing lost

- **History.** Hush keeps the last 500 transcripts in a file that only you can read. The History page searches, copies, and fixes them, and it can keep the audio of each take.
- **No lost takes.** When a transcription fails, Hush keeps the audio for 7 days, and `hush recover` transcribes it later.

### At home on the desktop

- **A settings window.** `Ctrl+K` searches every setting, page, and action, and each change saves at once.
- **A panel icon.** It shows whether Hush is idle, recording, or working, and its menu has the actions of the hotkeys.
- **A doctor.** `hush doctor` checks each part of the setup and prints the one command that fixes each problem.

The [feature guide](docs/features.md) has the details of each feature.

## Install

| Method | Command |
|---|---|
| Install script (Ubuntu, Debian, Fedora, openSUSE, Arch, and most others) | `curl -fsSL https://raw.githubusercontent.com/andymai/hush/main/install.sh \| sh` |
| Fedora (COPR) | `sudo dnf copr enable veroz/hush && sudo dnf install hush` |
| Release files | Download the `.deb`, `.rpm`, `AppImage`, or tarball from the [latest release](https://github.com/andymai/hush/releases/latest), and see [INSTALL.md](INSTALL.md#release-files) |
| NixOS (flake) | `nix profile install github:andymai/hush`, and see [INSTALL.md](INSTALL.md#nix-and-nixos) for the system module |
| Source | `make install`, and see [INSTALL.md](INSTALL.md#build) |

The script detects your distribution and checks the checksum of the download. It also checks the [build provenance](INSTALL.md#verify-a-download) when your system has GitHub CLI 2.49 or later and you logged in to it. Debian and Ubuntu get the `.deb`, and Fedora, RHEL, and openSUSE get the `.rpm`. Both install through your package manager, so removal is a normal `apt remove hush` or `dnf remove hush`. Every other distribution, Arch included, gets the tarball under `~/.local`.

The prebuilt packages are for x86-64 Linux. They need only your usual graphics driver: NVIDIA, AMD, and Intel all work through Vulkan, with a CPU fallback everywhere else.

## Setup

```bash
hush setup permissions        # one polkit prompt: keyboard and uinput access, no logout
hush models download base     # ~147 MB speech model
hush daemon start             # runs in the background
```

Hold `Right Alt`, speak, release. Done.

`hush setup init` asks for the GPU, the model, and the hotkey, and writes `~/.config/hush/config.toml`. The model defaults to the model that suits your hardware, which the Speech page of `hush settings` also names. `hush setup init --defaults` writes base without a prompt. `hush install --autostart` starts the daemon at login.

## Scope

Hush deliberately does not:

- **Send audio without asking.** Transcription runs on your machine. Groq, OpenAI, and an OpenAI-compatible server are an explicit choice (`transcription.backend`), and Hush never falls back to them on its own. The `openai-compatible` backend sends the audio to the URL that you set in `transcription.server_url`. Otherwise the network carries the one-time model download and the text for the LLM that you use. If you turn on the update check, it also carries a daily request for the newest release number.
- **Act as a voice assistant.** It types what you say. It does not answer questions, run commands, or wake on a phrase.
- **Run on macOS or Windows.** Linux-only, by design. It depends on `/dev/uinput`, evdev, and Linux window protocols.
- **Stream partial words.** It types the text when the recording stops, not live as you speak, also when it transcribes a hands-free recording in parts.
- **Replace your editor's dictation UI.** There is no correction overlay; text goes straight to the cursor. A fix on the History page changes only the copy in the history.

## Docs

- [Features](docs/features.md): each feature in detail.
- [Commands](docs/commands.md): every command, the exit codes and JSON output for scripts, and a Waybar module.
- [Configuration](docs/configuration.md): every key of `config.toml`, how to move your setup, and the transcript hook.
- [Models](docs/models.md): the sizes, the opt-in Parakeet model, and the helper models with their licenses.
- [Flatpak and desktop portals](docs/portals.md): what works in a sandbox, and on which desktops.
- [Troubleshooting](docs/troubleshooting.md): the microphone, keyboard access, and the GPU.
- [Privacy and security](docs/privacy.md): what goes over the network, and when Hush writes audio to disk.
- [INSTALL.md](INSTALL.md): release files, download checks, Nix, Flatpak builds, and builds from source.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and the [Code of Conduct](CODE_OF_CONDUCT.md). Hush is MIT licensed and runs its speech models through transcribe.cpp and its [transcribe-cpp](https://crates.io/crates/transcribe-cpp) bindings, on [ggml](https://github.com/ggml-org/ggml). Release notes live in [CHANGELOG.md](CHANGELOG.md).

## License

[MIT](LICENSE).

The settings window embeds the [Geist and Geist Mono](https://github.com/vercel/geist-font) fonts under the SIL Open Font License 1.1 ([OFL.txt](src/gui/fonts/OFL.txt)). It also embeds icons from [Lucide](https://lucide.dev) under the ISC License ([LICENSE-lucide.txt](src/gui/fonts/LICENSE-lucide.txt)).
