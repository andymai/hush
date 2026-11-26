# Configuration

`~/.config/hush/config.toml` only needs the keys you change; every default lives in [`config/default.toml`](../config/default.toml).

Hotkey chords name keys in two ways. Ctrl, Shift, Alt, and Super in a chord mean the keys that type these modifiers with the XKB options of your layout. With `ctrl:swap_lwin_lctl`, `Ctrl+RightAlt` fires with the Win key. A key alone, such as `RightAlt` or `RightCtrl`, is the physical key. When Hush cannot read the options, as on Sway, the modifier names also mean the physical keys.

| Key | Meaning | Default |
|---|---|---|
| `transcription.model_size` | tiny, base, small, medium, large, large-v2, large-v3, or the opt-in parakeet-v3 | base |
| `transcription.language` | A Whisper language code such as `en` or `de`, or `auto` to detect what you speak | en |
| `transcription.detect_languages` | With `language = "auto"`, a small language-ID model picks the most likely of these languages, such as `["en", "de"]`, and local Whisper writes in it. Hush downloads the model (27 MB) the first time, and tries again later when the download fails. Until the model is there, Whisper detects the language, and a take that it hears in another language runs again in the last language detected within the list, else in the first one. `[]` allows every language, and Whisper detects it. Cloud and server backends detect the language on their own | `[]` |
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
| `hotkey.mask_menu` | When a key or chord whose keys type only Alt and Super goes down, such as `RightAlt` or `Super+LeftAlt`, Hush taps Ctrl through its own virtual keyboard. Applications then open no menu bar, and the desktop opens no launcher. Hush does not tap with `hotkey.exclusive` or the `x11` backend, because they hide the key. Hush also does not tap when it cannot read the XKB options of the session, as on Sway, because an option can move Ctrl. There, `hotkey.exclusive` hides the key | true |
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
| `insertion.submit_apps` | Window classes where Hush presses Enter after it inserts a dictation, for example to send a chat message: 150 ms after typed text, 400 ms after a paste. A rule names the whole class or its last part after a dot. The rule matches the window class, so a browser rule presses Enter in every tab. Hush does not press Enter after a failed or empty insertion, when the transcript hook types nothing, after Command Mode, for `hush stop --print`, or while Shift Lock is on | none |
| `history.enabled`, `history.max_entries` | Keep delivered transcripts in `~/.local/share/hush/history.jsonl`, and how many | true, 500 |
| `history.keep_audio_days` | Keep the audio of each take as FLAC in `~/.local/share/hush/audio` for this many days. 0 keeps none and deletes the audio that Hush kept. The audio of a failed take stays in `~/.local/share/hush/recover` for 7 days for `hush recover`, whatever this says. When the two folders hold more than 500 MB, the oldest files go | 0 |
| `overlay.enabled` | The pill at the bottom of the screen while a take records and is transcribed | true |
| `tray.enabled` | Status icon on the desktop panel | true |
| `updates.check` | Ask GitHub once a day whether a newer release exists. AppImage, tarball, and source installs check. A package manager updates its own installs, so those never check | false |
| `gui.appearance` | The colors of the settings window: `system` follows the light or dark preference of the desktop and changes with it, and `light` or `dark` keeps one. When the desktop has no preference, the window is light | system |
| `input.backend` | `auto` uses the desktop portals in a Flatpak sandbox, and evdev and uinput everywhere else. `portal` uses the portals also outside a sandbox. `evdev` uses evdev and uinput also in a sandbox. See [Flatpak and desktop portals](portals.md) | auto |

Point at a different file with `hush -c other.toml …` or `HUSH_CONFIG=other.toml`.

## Move your setup

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

## Transcript hook

Set `hooks.transcript` to a command, and Hush runs it with `sh -c` on each transcript after polishing and before it types the text. Command Mode results and style rewrites also go through it. The text goes to stdin, and Hush types what the command prints. Hush removes one trailing newline from the output.

- Exit code 77 types nothing, and the take gets no history entry. After a style rewrite, exit code 77 leaves the selection as it is.
- When the command fails, prints nothing, or runs longer than 5 seconds, Hush types the original text and logs the reason. A command fails when it exits with a code other than 0 or 77, or cannot start.
- The command gets four environment variables:
  - `HUSH_APP`: the window class, empty when unknown.
  - `HUSH_PROFILE`: `terminal`, `editor`, `chat`, `mail`, `docs`, `browser`, or `other`.
  - `HUSH_RAW_TEXT`: the text as Whisper returned it. In Command Mode, the spoken instruction. In a style rewrite, the selected text.
  - `HUSH_TRIGGER`: `hotkey`, `ipc` for `hush toggle` and `hush start`, `tray`, `command` for Command Mode, or `rewrite` for a rewrite key and `hush rewrite`.
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
