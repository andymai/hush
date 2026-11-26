# Commands

Everyday dictation:

| Command | Does |
|---|---|
| `hush toggle` | Start or stop recording over the daemon socket |
| `hush start` / `hush stop` / `hush cancel` | Start, stop, or discard a recording |
| `hush start --lang de` / `hush toggle --lang de` | Start a recording in that language: a Whisper code, or `auto` |
| `hush start --style casual` / `hush toggle --style casual` | Start a recording in that writing style: `verbatim`, or a style under `[styles]`. An unknown style exits with code 2 |
| `hush stop --print [--json]` | Stop, wait for the text, and print it instead of typing it |
| `hush listen` | Run a dictation session attached to the terminal |
| `hush record [-d secs] [-p]` | Record for N seconds (default 30), transcribe, and type the text. `-p` prints the text on stdout and types nothing |
| `hush learn [words…]` | Add the selection, or the given words, to the vocabulary |
| `hush rewrite <style>` | Rewrite the selected text with the prompt of a style. The style needs polish on and a prompt. Another style exits with code 2 |
| `hush paste-last` | Re-type the last transcript |
| `hush shortcuts [--configure]` | In portal mode, show the keys that the desktop binds to the shortcuts of Hush. `--configure` opens the shortcut settings of the desktop |
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
| `hush setup mic [--undo] [--yes]` | Keep the microphone awake between recordings with one WirePlumber rule. `--undo` removes the rule, and `--yes` skips the question before WirePlumber restarts |
| `hush setup init [--defaults] [-f]` | Guided config: model, GPU, hotkey |
| `hush setup wizard [--auto]` | Full interactive setup |
| `hush setup audio` / `hotkeys` / `uinput` / `diagnose-uinput` | Configure or diagnose one subsystem |
| `hush config export <file> [-f]` / `import <file> [--merge]` | Move the settings and the vocabulary to another machine. See [Move your setup](configuration.md#move-your-setup) |

Diagnostics and lifecycle:

| Command | Does |
|---|---|
| `hush status [--full]` | Daemon state, settings, and microphones. `--full` adds the `hush doctor` report |
| `hush status --devices` / `--config` | Only the microphones, or only the settings. Use both flags to get both. They need no daemon |
| `hush status --json [--follow]` | Daemon state and setup as JSON; `--follow` prints a Waybar line for each change |
| `hush test audio\|transcription\|text-insertion\|hotkeys\|window\|pipeline\|all` | Targeted self-tests |
| `hush install [--autostart] [--desktop] [--system]` | Set up autostart and the desktop entry |
| `hush uninstall [--autostart] [--desktop] [--system]` | Reverse the above |

The .deb, the .rpm, the AUR package, and the Nix flake ship the user unit `hush.service`. Hush uses the unit when it runs the same binary and a graphical session runs under systemd. Then `hush daemon start` and `restart` go through the unit, `hush install --autostart` enables it, and `hush uninstall --autostart` disables it. A start or restart with `--editing-mode`, `--no-processing`, or `--no-button` runs the daemon outside the unit, because the unit starts it with the default options. A daemon that runs outside the unit stops when the unit starts, after its recording ends.

Without a graphical session under systemd, as on Sway, Hyprland, or niri started from a TTY, Hush starts the daemon itself. The compositor's config must then start it at login:

- Sway: `exec hush daemon start`
- Hyprland: `exec-once = hush daemon start`
- niri: `spawn-at-startup "hush" "daemon" "start"`

Global flags: `-c, --config-file <path>` (or `HUSH_CONFIG`) points at another config file; `-v`, `-vv`, `-vvv` set INFO, DEBUG, and TRACE logging; `--no-notifications` silences desktop notifications. `hush --help` lists everything.

## Scripts

`hush toggle`, `hush start`, `hush stop`, `hush cancel`, `hush status`, `hush paste-last`, and `hush rewrite` use fixed exit codes. `hush status --devices` and `hush status --config` do not ask the daemon, so they do not exit with 3:

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
| `locked` | While a recording runs: `true` when it is hands-free, else `false`. `null` outside a recording |
| `command` | While a recording runs: `true` when it is a Command Mode instruction, else `false`. `null` outside a recording |
| `stops_in_ms` | Once the recording cap is near: the time until it stops the recording, else `null` |
| `style` | The style that the recording in progress asked for (`--style` or a style key), else `null` |
| `overlay` | `running`, `unavailable` (no layer-shell and no X11 display), `failed` (it crashed again and again, or Hush was updated under it), or `off` (`overlay.enabled = false`); `null` when no daemon runs |
| `daemon_version` | The version of the daemon that answered, else `null`. After an upgrade, it can differ from `hush --version` until the daemon restarts |
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
| `safe_mode` | Why the daemon runs in safe mode, else `null`. A daemon that started while the configuration did not load dictates on the defaults and writes and deletes nothing in the history and the kept audio |
| `start_delay` | How long the last 10 recordings waited for the first audio from the microphone: `{"last_ms": 40, "worst_ms": 650, "takes": 10}`. `null` when no daemon runs or before the first recording |

## Waybar

`hush status --follow --json` prints one line at start and one line for each state change, in the JSON format of a Waybar custom module. `alt` and `class` are the state name. The tooltip also says when a recording is hands-free or a Command Mode instruction, and names the style that the recording asked for. When the daemon stops, the module shows `stopped`, and it recovers when the daemon starts again.

```json
"custom/hush": {
    "return-type": "json",
    "exec": "hush status --follow --json",
    "on-click": "hush toggle"
}
```

Give each state a color in the Waybar style sheet, for example `#custom-hush.recording { color: #e5484d; }`.
