<div align="center">

# Hush

Local voice-to-text for Linux. Hold a key, speak, release; your words land at the cursor, in any app.

[![CI](https://github.com/andymai/hush/actions/workflows/ci.yml/badge.svg)](https://github.com/andymai/hush/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/andymai/hush?label=release)](https://github.com/andymai/hush/releases/latest)
[![Commit activity](https://img.shields.io/github/commit-activity/m/andymai/hush?label=commits%2Fmonth)](https://github.com/andymai/hush/commits/main)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust 1.95+](https://img.shields.io/badge/rust-1.95%2B-orange.svg)](https://www.rust-lang.org/)

**[Install](#install)** · **[Setup](#setup)** · **[Features](#features)** · **[Commands](#commands)** · **[Configure](#configure)** · **[Models](#models)** · **[Troubleshooting](#troubleshooting)**

</div>

Hush turns speech into text on any Linux desktop. Hold a key, talk, let go, and the words appear wherever your cursor is. It runs OpenAI's Whisper models on your own machine (on the GPU through Vulkan when you have one) and types the result through the kernel's input layer, so it works in terminals, browsers, editors, chat, VMs, games, and password fields, on both Wayland and X11. Audio stays on your computer unless you choose a cloud service for transcription.

It is built to disappear into a keyboard-driven workflow: bind any key to it, tune the output per application, and teach it the words you use.

## Install

| Method | Command |
|---|---|
| Install script (Ubuntu, Debian, Fedora, openSUSE, Arch, and most others) | `curl -fsSL https://raw.githubusercontent.com/andymai/hush/main/install.sh \| sh` |
| Fedora (repository) | `sudo dnf copr enable veroz/hush && sudo dnf install hush` |
| Manual | Grab the `.deb`, `.rpm`, `AppImage`, or tarball from the [latest release](https://github.com/andymai/hush/releases/latest) |
| NixOS (flake) | `nix profile install github:andymai/hush`, and see [INSTALL.md](INSTALL.md#nix-and-nixos) for the system module |

The script detects your distribution, verifies the download's checksum (and its [build provenance](INSTALL.md#verify-a-download) when `gh` is installed), and installs through your package manager, so removal is a normal `apt remove hush` or `dnf remove hush`. On Arch it installs the tarball under `~/.local` until an AUR package is published. It needs only your usual graphics driver: NVIDIA, AMD, and Intel all work through Vulkan, with a CPU fallback everywhere else.

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
- **Long recordings in parts.** A hands-free recording (locked, toggle mode, `hush toggle`, or the panel icon) transcribes in parts while you speak. Hush cuts the audio at each pause of about 0.8 seconds, or after about 30 seconds without one, and transcribes each part in the background on the same model. Each part continues from the text before it, so sentences keep their case and punctuation. Every part is in the language of the recording. With `detect_languages`, the first part with speech detects the language, and the other parts keep it. When you stop, only the last part is left, so the text comes soon after. Hush then polishes the joined text once and types it. A held recording stays one pass.
- **Speak at the tone.** Hush plays the start tone and shows the recording overlay when the first audio from the microphone arrives, not when you press the key. A microphone that sleeps between recordings can need a moment to wake, and the tone tells you that Hush hears you. If no audio arrives in five seconds, Hush discards the recording and shows a notification.
- **Bind any key to it.** `hush toggle`, `hush start`, `hush stop`, and `hush cancel` talk to the daemon over a socket, so any compositor keybind can drive dictation. Hyprland: `bind = , F9, exec, hush toggle`. Sway: `bindsym F9 exec hush toggle`.
- **Tone by application.** Text lands the way the focused app expects: lowercase and unpunctuated in terminals and editors, casual in chat, full sentences in mail and documents. In a terminal, each line break becomes a space, so dictated text cannot run a command. In chat, Hush types a line break as Shift+Enter, so a line break does not send the message. Add a window class to `insertion.submit_apps`, and Hush presses Enter after the text. The window title and your learned terms also prime Whisper, so names come out spelled your way. Works on X11, Hyprland, Sway, and KDE Plasma on Wayland.
- **Any language Whisper knows.** Set `language = "auto"` under `[transcription]` to detect what you speak, or pick one in the settings window. With `detect_languages = ["en", "de"]`, a small language-ID model picks among only the languages you speak, and local Whisper writes in that language. On every keyboard layout, Hush types key by key through the XKB keymap of the layout. It types accents through dead keys when the layout has them. When a take has a character that no key types, such as an emoji, Hush pastes the full take. With more than one layout, Hush must know which layout is active. KDE Plasma, GNOME, Hyprland, Sway, and X11 report the active layout. If Hush cannot find the active layout, it pastes, because a wrong layout types wrong letters. On GNOME, Hush also pastes when each window keeps its own input source. On X11, Hush types only on the default layout of the X server, because its own keyboard gets that layout. `hush doctor` shows whether Hush types or pastes, and why.
- **A key for each language.** `languages = { "Shift+RightAlt" = "de" }` under `[hotkey]` gives German its own key, with the same hold, double-tap lock, and cancel as the hotkey. The Language menu of the panel icon sets the language of the hotkey until you change it again, also after a restart. `hush start --lang de` and `hush toggle --lang de` set the language of one recording.
- **Writing styles.** A style has a name, one line for the polish prompt, and a polish switch. Add styles under `[styles]` in the config or on the Text page of `hush settings`. The built-in `verbatim` style turns polish off for one recording. `styles = { "Super+RightAlt" = "verbatim" }` under `[hotkey]` gives a style its own dictation key, and `hush start --style casual` sets the style of one recording. `app_styles = { slack = "casual" }` under `[profiles]` sets a default style for an application. A style key or `--style` comes first, then the style of the application, then only the tone of the application.
- **Rewrite with a style.** `rewrite = { "Super+F" = "formal" }` under `[hotkey]` rewrites the selected text with the prompt of the `formal` style, with no speech. `hush rewrite formal` does the same from a script or a compositor bind. With nothing selected, Hush shows a notice and sends nothing to the LLM. When the LLM fails, the selection does not change. The style needs polish on and a prompt, so `verbatim` cannot rewrite.
- **Teach it your words.** Select a term Hush keeps misspelling and run `hush learn` (or `hush learn Kubernetes`); from then on it is written exactly that way. The Text page of `hush settings` lists, edits, and removes your words. A fix on the History page also suggests words to learn. `hush paste-last` re-types the last transcript, and when insertion fails the text waits on the clipboard.
- **History.** Hush keeps the last 500 transcripts on your computer, in a file only you can read. The History page of `hush settings` searches them, copies one again, fixes one, or deletes them. Turn it off with `history.enabled = false`. Turn on Keep the audio on that page, or set `history.keep_audio_days`, and Hush also keeps the audio of each take as FLAC for that many days. The page then plays the audio and transcribes it again with the current model. When you fix an entry, the page suggests the new names and terms in your correction. These are words that are not common English words, and words with a new case. Only Add puts a word into the vocabulary, as `hush learn` does.
- **No lost takes.** When a transcription fails, Hush keeps the audio of the take for 7 days, in a file only you can read. Fix the problem, then run `hush recover`: it transcribes the audio, prints or types the text, and deletes the file. `hush transcribe <file>` prints the text of any WAV or FLAC file.
- **Command Mode.** Hold `Ctrl+Right Alt`, say what to do ("make this friendlier", "turn it into a bullet list"), release. Hush rewrites the selected text, or the last thing it typed when nothing is selected, through a local Ollama server, Anthropic, or an OpenAI-compatible server, whichever `[llm]` resolves to. Only text is sent, never audio.
- **Cleaner text, automatically.** Voice commands cover "new line", "new paragraph", "undo", and "scratch that". Filler words ("um", "uh", "like") are removed. Optional LLM polishing rewrites every transcript.
- **Panel icon.** A status icon shows whether Hush is idle, recording, or working, and its menu offers the same actions the hotkeys do. Turn it off with `tray.enabled = false`.
- **A window and a doctor.** `hush settings` opens one window for what still needs setting up and every setting. `hush doctor` checks the configuration, keyboard access, model, GPU, microphone, hotkey, window detection, polishing, transcript hook, panel icon, update check, autostart, and the daemon, then prints the one command that fixes each problem.

## Scope

Hush deliberately does not:

- **Send audio without asking.** Transcription runs on your machine. Groq, OpenAI, and an OpenAI-compatible server are an explicit choice (`transcription.backend`), and Hush never falls back to them on its own. The `openai-compatible` backend sends the audio to the URL that you set in `transcription.server_url`. Otherwise the only network traffic is the one-time model download and, if you turn them on, text-only LLM polishing and a daily check for a new release.
- **Act as a voice assistant.** It types what you say. It does not answer questions, run commands, or wake on a phrase.
- **Run on macOS or Windows.** Linux-only, by design. It depends on `/dev/uinput`, evdev, and Linux window protocols.
- **Stream partial words.** It types the text when the recording stops, not live as you speak, also when it transcribes a hands-free recording in parts.
- **Replace your editor's dictation UI.** There is no correction overlay; text goes straight to the cursor. A fix on the History page changes only the copy in the history.

## Commands

Everyday dictation:

| Command | Does |
|---|---|
| `hush toggle` | Start or stop recording over the daemon socket |
| `hush start` / `hush stop` / `hush cancel` | Start, stop, or discard a recording |
| `hush start --lang de` / `hush toggle --lang de` | Start a recording in that language: a Whisper code, or `auto` |
| `hush start --style casual` / `hush toggle --style casual` | Start a recording in that writing style: `verbatim`, or a style under `[styles]`. An unknown style exits with code 2 |
| `hush stop --print [--json]` | Stop, wait for the text, and print it instead of typing it |
| `hush listen` | Run a dictation session attached to the terminal |
| `hush record [-d secs]` | Record for N seconds (default 30) and transcribe |
| `hush learn [words…]` | Add the selection, or the given words, to the vocabulary |
| `hush rewrite <style>` | Rewrite the selected text with the prompt of a style. The style needs polish on and a prompt. Another style exits with code 2 |
| `hush paste-last` | Re-type the last transcript |
| `hush transcribe <file>` | Print the text of a WAV or FLAC file, with the daemon's model when the daemon runs |
| `hush recover [--all] [--list] [--type]` | Transcribe the newest take that failed (or `--all` of them), print or type the text, and delete its audio. `--list` only lists them |
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
| `hush models download <size> [-f]` | Download a model, and the voice activity detection model when it is missing |
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
| `hush config export <file> [-f]` / `import <file> [--merge]` | Move the settings and the vocabulary to another machine. See [Move your setup](#move-your-setup) |

Diagnostics and lifecycle:

| Command | Does |
|---|---|
| `hush status [--full] [--devices] [--config]` | System diagnostics; `--devices` lists microphones |
| `hush status --json [--follow]` | Daemon state and setup as JSON; `--follow` prints a Waybar line for each change |
| `hush test audio\|transcription\|text-insertion\|hotkeys\|window\|pipeline\|all` | Targeted self-tests |
| `hush install [--autostart] [--desktop] [--system]` | Set up autostart and the desktop entry |
| `hush uninstall [--autostart] [--desktop] [--system]` | Reverse the above |

Global flags: `-c, --config-file <path>` (or `HUSH_CONFIG`) points at another config file; `-v`, `-vv`, `-vvv` set INFO, DEBUG, and TRACE logging; `--no-notifications` silences desktop notifications. `hush --help` lists everything.

## Scripts

`hush toggle`, `hush start`, `hush stop`, `hush cancel`, `hush status`, and `hush paste-last` use fixed exit codes:

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | An error |
| 2 | A usage error |
| 3 | No daemon runs |
| 4 | The recording has no speech (`hush stop --print`) |

`hush stop --print` stops the recording, waits up to 60 seconds for the final text, and prints it. Hush types nothing, and the history keeps the transcript. With no recording in progress, it prints a message on stderr and exits with 1. If you set `audio.silence_stop_secs`, a pause can stop the recording before your script does, and then Hush types the text.

```bash
hush start
sleep 5
text="$(hush stop --print)" && notify-send "Hush heard" "$text"
```

`hush stop --json` prints one JSON object with the keys `status` (`ok`, `no_speech`, or `error`), `text`, `chars`, and `message`. A key without a value is `null`. Add `--print` to get the text in `text`.

`hush status --json` prints one JSON object:

| Key | Value |
|---|---|
| `state` | `idle`, `starting`, `recording`, `transcribing`, `inserting`, `stopping`, or `stopped` when no daemon runs |
| `running` | `true` when the daemon answers |
| `elapsed_ms` | Length of the current recording, else `null` |
| `model_state` | `loaded`, `loading`, or `unloaded` for the local model; `none` for a cloud or server backend; `null` when no daemon runs |
| `backend` | `local`, `groq`, `openai`, or `openai-compatible` |
| `model` | Model size for `local`, model name for a cloud backend |
| `model_path` | Model file for `local`, else `null` |
| `use_gpu` | The `transcription.use_gpu` setting |
| `gpu` | Name of the GPU that Hush can use, else `null` |
| `gpu_type` | `cuda`, `vulkan`, or `cpu` |
| `sample_rate` | Audio sample rate in Hz |
| `hotkey` | The dictation hotkey |
| `language` | The language of the recording in progress, else of the next recording: a Whisper code or `auto`. Without a daemon, `transcription.language` |
| `devices` | Names of the audio input devices |
| `os`, `arch` | Operating system and CPU architecture |
| `config_error` | Why the configuration did not load, else `null` |

### Waybar

`hush status --follow --json` prints one line at start and one line for each state change, in the JSON format of a Waybar custom module. `alt` and `class` are the state name. When the daemon stops, the module shows `stopped`, and it recovers when the daemon starts again.

```json
"custom/hush": {
    "return-type": "json",
    "exec": "hush status --follow --json",
    "on-click": "hush toggle"
}
```

Give each state a color in the Waybar style sheet, for example `#custom-hush.recording { color: #e5484d; }`.

## Configure

`~/.config/hush/config.toml` only needs the keys you change; every default lives in [`config/default.toml`](config/default.toml).

| Key | Meaning | Default |
|---|---|---|
| `transcription.model_size` | tiny, base, small, medium, large, large-v2, large-v3 | base |
| `transcription.language` | A Whisper language code such as `en` or `de`, or `auto` to detect what you speak | en |
| `transcription.detect_languages` | With `language = "auto"`, a small language-ID model picks the most likely of these languages, such as `["en", "de"]`, and local Whisper writes in it. Hush downloads the model (27 MB) the first time, and tries again later when the download fails. Until the model is there, Whisper detects the language, and a take that it hears in another language runs again in the first language of the list. `[]` allows every language, and Whisper detects it. Cloud and server backends detect the language on their own | `[]` |
| `transcription.use_gpu` | Use the GPU backend the binary was built with | true |
| `transcription.context_prompt` | Prime Whisper with the focused window's title and learned terms | true |
| `transcription.vad` | Send local Whisper only the speech in each take, found by the Silero model that `hush models download` gets. Without the model, Whisper gets the whole take | true |
| `transcription.unload_after_mins` | Free the local model's RAM and GPU memory after this many minutes without a take. The next take records at once and loads the model while you speak; 0 never frees it. Cloud and server backends ignore it | 0 |
| `transcription.drop_phrases` | Drop a transcript that is only one of these phrases, without case and punctuation. When set, it replaces the built-in list of text that Whisper invents for silence, such as "thanks for watching"; `[]` drops nothing | built-in list |
| `transcription.backend` | `local` (Whisper through transcribe.cpp), or `groq` or `openai`, which send the audio of each dictation to that service. The key comes from `GROQ_API_KEY` or `OPENAI_API_KEY`. `openai-compatible` sends the audio of each dictation to the URL in `transcription.server_url` | local |
| `transcription.groq_model`, `transcription.openai_model` | The model each service runs | whisper-large-v3-turbo, gpt-4o-mini-transcribe |
| `transcription.server_url`, `transcription.server_model` | The OpenAI-compatible server and its model. Hush adds `/v1/audio/transcriptions` when the URL does not end with it. `hush doctor` shows whether the server is local | none |
| `transcription.server_key_env` | The environment variable that holds the server's API key; `""` sends no key | none |
| `hotkey.combination` | Key or button: `RightAlt`, `RightCtrl`, `F13`, `Mouse4`, or a chord like `Ctrl+Shift+Space` | RightAlt |
| `hotkey.mode` | `hold` (double-tap locks hands-free) or `toggle` (a tap locks) | hold |
| `hotkey.backend` | `auto`, `evdev`, or `x11` | auto |
| `hotkey.exclusive` | Grab the hotkey's device so applications never see the key or button | false |
| `hotkey.mask_menu` | When a key or chord with only Alt and Super keys goes down, such as `RightAlt` or `Super+LeftAlt`, Hush taps Ctrl through its own virtual keyboard. Applications then open no menu bar, and the desktop opens no launcher. Hush does not tap with `hotkey.exclusive` or the `x11` backend, because they hide the key. Hush also does not tap when it cannot read the XKB options of the session, as on Sway, because an option can move Ctrl. There, `hotkey.exclusive` hides the key | true |
| `hotkey.cancel` | Key that discards a recording in progress; `""` disables it | Escape |
| `hotkey.tap_ms` | Presses shorter than this are taps rather than holds | 300 |
| `hotkey.paste_last` | Chord that re-types the last transcript, such as `Shift+RightAlt` | unbound |
| `hotkey.learn` | Chord that adds the selected text to the vocabulary | unbound |
| `hotkey.command` | Command Mode chord; `""` disables it | Ctrl+RightAlt |
| `hotkey.languages` | A dictation key for each language, such as `{ "Shift+RightAlt" = "de" }`, with the same hold, double-tap lock, toggle mode, and cancel as `hotkey.combination`. The Language menu of the panel icon sets the language of `hotkey.combination` | none |
| `hotkey.styles` | A dictation key for each writing style, such as `{ "Super+RightAlt" = "verbatim" }`, with the same hold, double-tap lock, toggle mode, and cancel as `hotkey.combination` | none |
| `hotkey.rewrite` | A key that rewrites the selected text with the prompt of a style, such as `{ "Super+F" = "formal" }`. The style needs polish on and a prompt | none |
| `styles.<name>.prompt`, `styles.<name>.polish` | A writing style. `prompt` is one line that Hush adds to the polish prompt next to the tone of the application. `polish` turns the LLM polish on or off for recordings in this style, whatever `llm.polish` says. A name uses lowercase letters, digits, `-`, and `_`. `verbatim` is built in and turns polish off | only `verbatim` |
| `llm.provider` | `auto` (Ollama when reachable, else Anthropic when a key exists), `ollama`, `anthropic`, `openai-compatible`, or `none`. `auto` never picks `openai-compatible` | auto |
| `llm.ollama_url`, `llm.ollama_model` | The local Ollama server and model | localhost:11434, llama3.2 |
| `llm.anthropic_model` | Anthropic model; the key comes from `ANTHROPIC_API_KEY` | claude-haiku-4-5 |
| `llm.server_url`, `llm.server_model`, `llm.server_key_env` | The OpenAI-compatible server, its model, and the variable that holds its key. Hush adds `/v1/chat/completions` when the URL does not end with it | none |
| `llm.polish` | Rewrite every transcript with the LLM; Command Mode works either way | true |
| `hooks.transcript` | A command that changes each transcript before Hush types it. See [Transcript hook](#transcript-hook) | `""` (off) |
| `profiles.enabled` | Adapt output to the focused application, including one-line text in terminals and Shift+Enter line breaks in chat; per-profile keys add window classes | true |
| `profiles.app_styles` | A default writing style by window class, such as `{ slack = "casual" }`. A rule names the whole class or its last part after a dot. A style key or `--style` comes first. Hush ignores it when `profiles.enabled` is false | none |
| `audio.max_recording_secs` | Stop and transcribe after this long, with a warning a minute before; 0 disables | 600 |
| `audio.silence_stop_secs` | Stop a hands-free recording after this many seconds of silence that follow speech; 0 disables | 0 |
| `audio.pause_media` | Pause the media players that play while you record, and resume them when the recording stops. Hush resumes only the players that it paused, and only when they are still paused. The players must support MPRIS, as most do | false |
| `audio.device` | Microphone name from `hush status --devices`. Without it, Hush uses the default microphone. On PipeWire, Hush follows a change of the default, because PipeWire moves its stream. With a name set, Hush keeps that device | system default |
| `feedback.audio_enabled` | Tones on start, stop, cancel, and the cap warning | true |
| `insertion.method` | `auto` (type with the keys of the active layout; paste the full take when a character has no key, or when the take is very long), `uinput`, or `clipboard` | auto |
| `insertion.uinput_apps`, `insertion.clipboard_apps` | Window classes that always type or always paste, whatever `insertion.method` says; part of the class is enough | none |
| `insertion.paste_keys` | Paste chord by window class: `ctrl+v`, `ctrl+shift+v`, or `shift+insert`. A rule names the whole class or its last part after a dot. Without a rule, terminals get `ctrl+shift+v`; Konsole, VS Code, VSCodium, and Cursor get `shift+insert`; other applications get `ctrl+v` | none |
| `insertion.submit_apps` | Window classes where Hush presses Enter after it inserts a dictation, for example to send a chat message: 150 ms after typed text, 400 ms after a paste. A rule names the whole class or its last part after a dot. The rule matches the window class, so a browser rule presses Enter in every tab. Hush does not press Enter after a failed or empty insertion, when the transcript hook types nothing, after Command Mode, or for `hush stop --print` | none |
| `history.enabled`, `history.max_entries` | Keep delivered transcripts in `~/.local/share/hush/history.jsonl`, and how many | true, 500 |
| `history.keep_audio_days` | Keep the audio of each take as FLAC in `~/.local/share/hush/audio` for this many days. 0 keeps none and deletes the audio that Hush kept. The audio of a failed take stays in `~/.local/share/hush/recover` for 7 days for `hush recover`, whatever this says. When the two folders hold more than 500 MB, the oldest files go | 0 |
| `tray.enabled` | Status icon on the desktop panel | true |
| `updates.check` | Ask GitHub once a day whether a newer release exists (AppImage and tarball installs only) | false |

Point at a different file with `hush -c other.toml …` or `HUSH_CONFIG=other.toml`.

### Move your setup

Write your settings and your vocabulary to one file, then load that file on the other machine:

```sh
hush config export hush-setup.toml
hush config import hush-setup.toml
```

- The file has only the keys in your `config.toml` and the entries in your `vocabulary.json`. It has no API keys, so copy `~/.config/hush/.env` yourself.
- `export` does not replace a file that exists. Add `--force` to replace it.
- `import` replaces the settings and the vocabulary. With `--merge`, it keeps your current values and adds only the keys and entries that you do not have.
- Before `import` writes a file, it checks the whole file and keeps the old file next to it as `config.toml.bak` or `vocabulary.json.bak`. A running daemon restarts with the new settings.

The Text page of `hush settings` also imports and exports the vocabulary as a plain-text list, with one word on each line.

### Transcript hook

Set `hooks.transcript` to a command, and Hush runs it with `sh -c` on each transcript after polishing and before it types the text. Command Mode results and style rewrites also go through it. The text goes to stdin, and Hush types what the command prints. Hush removes one trailing newline from the output.

- Exit code 77 types nothing, and the take gets no history entry. After a style rewrite, exit code 77 leaves the selection as it is.
- When the command exits with a code other than 0 or 77, cannot start, prints nothing, or runs longer than 5 seconds, Hush types the original text and logs the reason.
- The command gets `HUSH_APP` (the window class, empty when unknown), `HUSH_PROFILE` (`terminal`, `editor`, `chat`, `mail`, `docs`, `browser`, or `other`), `HUSH_RAW_TEXT` (the text as Whisper returned it; in Command Mode, the spoken instruction; in a style rewrite, the selected text), and `HUSH_TRIGGER` (`hotkey`, `ipc` for `hush toggle` and `hush start`, `tray`, `command` for Command Mode, or `rewrite` for a rewrite key and `hush rewrite`).
- The command does not get the API keys that Hush reads from its .env files.

Remove every exclamation mark:

```toml
[hooks]
transcript = "/usr/bin/tr -d '!'"
```

Put a list marker before each transcript that you dictate into Obsidian:

```sh
#!/bin/sh
# ~/.local/bin/hush-hook (chmod +x), with transcript = "/home/you/.local/bin/hush-hook"
text=$(cat)
case "$HUSH_APP" in
  obsidian) printf '%s' "- $text" ;;
  *) printf '%s' "$text" ;;
esac
```

The daemon runs with the PATH of your login session, so use absolute paths. `hush doctor` shows whether the command resolves.

## Models

| Model | Download |
|---|---|
| tiny | 78 MB |
| **base** (default) | **147 MB** |
| small | 488 MB |
| medium | 1.5 GB |
| large, large-v2, large-v3 | 3.1 GB each |

Larger models are more accurate and slower. `hush setup init` picks one for your hardware: medium on a GPU with 16 GB or more of RAM, small on a smaller-RAM GPU, base on a CPU. Manage them with `hush models list`, `hush models download <size>`, and `hush models set <size>`. Downloads come from the whisper.cpp catalogue and are checksum-verified.

Hush also downloads two small helper models. The voice activity model comes with `hush models download`. The language-ID model comes only when `transcription.detect_languages` is set.

| Model | Source | License |
|---|---|---|
| Whisper (OpenAI) | ggml files from [ggerganov/whisper.cpp](https://huggingface.co/ggerganov/whisper.cpp) | MIT |
| Silero VAD v6.2 | [ggml-org/whisper-vad](https://huggingface.co/ggml-org/whisper-vad), from [snakers4/silero-vad](https://github.com/snakers4/silero-vad) | MIT |
| VoxLingua107 ECAPA-TDNN language ID | [collectivecontext/hush-models](https://huggingface.co/collectivecontext/hush-models), converted with transcribe.cpp from [speechbrain/lang-id-voxlingua107-ecapa](https://huggingface.co/speechbrain/lang-id-voxlingua107-ecapa) | Apache-2.0 |

## Troubleshooting

```bash
hush doctor                      # checks everything and prints the fix for each problem
hush status --full               # every component, with the device ggml selected
hush setup permissions --check   # keyboard and uinput access
hush status --devices            # microphones
hush test audio -d 3             # record and play back
hush -vv daemon start --foreground
```

If a notification says that the microphone did not start, Hush got no audio for five seconds. Hush opens the microphone again before the next recording. If the problem stays, run `hush test audio -d 3` to test the microphone, or set `audio.device` to a different one from `hush status --devices`.

If a notification says that the microphone stopped, the audio stopped for two seconds during a recording. Hush inserts what it heard until then, and opens the microphone again before the next recording.

If `hush setup permissions` cannot run where you are (a container, an SSH session), `hush setup permissions --print` prints the udev rule to apply as root.

In the first three minutes after boot, Hush waits up to five seconds for the graphics driver. If the driver is not ready then, Whisper runs on the CPU until you run `hush daemon restart`. When a take fails on the GPU, Hush runs it again on the CPU. Hush then keeps the CPU for that GPU until the driver version changes. `hush doctor` shows a `GPU failure` line with the command that tries the GPU again.

The prebuilt packages need an x86-64-v3 CPU: Intel Haswell (2013) and newer, or AMD Excavator, Zen, and newer. On an older CPU, `hush doctor` shows a failed `CPU` line. Build from source with `make release`, or use a cloud backend. See [INSTALL.md](INSTALL.md#cpu-requirement).

## Privacy and security

Everything runs locally by default. The only network traffic is the one-time model download, text-only LLM polishing if you turn it on (never audio), and the update check if you turn it on (one request a day to GitHub for the newest release number). If you set `transcription.backend` to `groq` or `openai`, Hush sends the audio of each dictation to that service, with the focused window's title when `transcription.context_prompt` is on. If you set it to `openai-compatible`, Hush sends the same data to the URL in `transcription.server_url`; `hush doctor` and `hush status` show whether that address is a local server. When that service fails, Hush reports the error and does not try another service. Hush writes audio to disk only when you set `history.keep_audio_days`, while a hands-free recording transcribes in parts (until Hush types the text), and when a transcription fails (for 7 days, for `hush recover`). Only you can read those files. Hush reads keyboard events and types through `/dev/input` and `/dev/uinput`; one udev rule grants the logged-in user access to those for the length of the session. Details in [SECURITY.md](SECURITY.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and the [Code of Conduct](CODE_OF_CONDUCT.md). Hush is MIT licensed and runs Whisper through [transcribe.cpp](https://github.com/handy-computer/transcribe.cpp) and its [transcribe-cpp](https://crates.io/crates/transcribe-cpp) bindings, on [ggml](https://github.com/ggml-org/ggml). Release notes live in [CHANGELOG.md](CHANGELOG.md).

## License

[MIT](LICENSE).
