# End-to-end tests

`scripts/e2e/run.sh` proves that dictation works from the microphone to the
text. It starts a test daemon with its own config and folders, plays a
recorded clip into a PipeWire virtual microphone in real time, and reads the
text with `hush stop --print --json`. The test daemon types nothing.

## Safety

The harness can run next to your desktop session and your own Hush daemon. It
does not change them:

- The virtual microphone is a `pw-loopback` pair: a sink for the clip and a
  source for the test daemon. No node of the pair outputs to a speaker.
- `PIPEWIRE_NODE` points only the capture of the test daemon at the virtual
  source. The default sink and source stay the same, and no other stream
  moves. The harness fails when the defaults change during a run.
- Each test stream has `node.dont-fallback`, so a missing target plays or
  records nothing. WirePlumber keeps no state for the test nodes.
- The harness uses `pw-dump` to make sure that the test daemon records only
  from the virtual source, that `pw-cat` plays only into the virtual sink, and
  that no other process links to them. It checks the route of `pw-cat` with
  silence before any speech plays.
- The test daemon gets a clean environment: a temporary `HOME`, XDG folders,
  and `XDG_RUNTIME_DIR`, so its socket is never the socket of your daemon. It
  has no display and no session bus, so it shows no overlay and no
  notification. Its config turns off the hotkey, the tray, the sound cues, the
  LLM, the media pause, and the length cap.
- The test daemon runs in a user namespace where `/dev/uinput` is `/dev/null`,
  so it cannot type. When the harness cannot make this namespace and the test
  daemon can write `/dev/uinput`, the harness stops. Set
  `HUSH_E2E_ALLOW_UINPUT=1` only on a machine without a desktop session.
- A `trap` stops the daemon, removes the virtual nodes, and deletes the
  temporary folder, also when a case fails or you press Ctrl+C. If the
  harness is killed, the kernel sends `SIGTERM` to the virtual microphone at
  once and to the test daemon when its case ends (`setpriv --pdeathsig`).
  Only the temporary folder stays.
- The harness never uses `~/.config/hush`, `~/.local/share/hush`,
  `~/.local/state/hush`, or `~/.cache/hush`.

## Run it

You need PipeWire with WirePlumber, the PipeWire ALSA plugin
(`pipewire-alsa`), the PipeWire tools (`pw-cat`, `pw-loopback`, `pw-dump`,
`pw-metadata`), `jq`, and util-linux. Run the harness where the binary can
reach the PipeWire socket of your session.

```sh
scripts/e2e/run.sh                 # all cases
scripts/e2e/run.sh english_clip    # one case
```

The first run downloads the `tiny` model and the voice activity model into
`~/.cache/hush-e2e/`. Later runs use that copy.

Two cases use the language-ID model:

- `detect_limited` needs the model before its take. With
  `HUSH_E2E_LANGID_MODEL`, the harness links that copy. Without it, the test
  daemon downloads the model (27 MB) from `collectivecontext/hush-models` at
  start, each run, and the case waits up to 60 s for it.
- `detect_limited_late_model` starts without the model and links
  `HUSH_E2E_LANGID_MODEL` after the first take. Without the variable, it
  checks nothing.

| Variable | Use |
| --- | --- |
| `HUSH_BIN` | The binary to test. The default is `target/debug/hush`, which the harness builds when it is missing. |
| `HUSH_E2E_MODEL` | A ggml model file to use instead of `tiny`. |
| `HUSH_E2E_LANGID_MODEL` | A copy of `lang-id-voxlingua107-ecapa-Q8_0.gguf` for the language-ID cases. It must match the SHA-256 in `models.rs`. |
| `HUSH_E2E_KEEP=1` | Keep the temporary folder with the daemon logs. |
| `HUSH_E2E_TAIL_SECS` | The time from the end of the clip to the stop. The default is 1.5. |

The nightly workflow `.github/workflows/e2e.yml` runs the harness on Ubuntu.
There, `scripts/e2e/headless.sh` starts a private PipeWire and WirePlumber.
Do not use `headless.sh` on a desktop.

## Add a case

Write a function `case_<name>` in `scripts/e2e/run.sh`. The runner finds it.
Each case runs in a subshell with its own `CASE_DIR` and `CASE_HOME`, and a
failed command or assertion stops the case.

```bash
case_example() {
  start_daemon 'transcription.language = "auto"'
  record_clip "$SAMPLES/jfk.wav"
  stop_print_json
  assert_exit 0 "$TAKE_EXIT" "hush stop --print --json"
  assert_words "$TAKE_TEXT" ask country
}
```

- `start_daemon [toml]` writes the config and starts the test daemon. Write
  the TOML with dotted keys. A key replaces the base value of the same key.
- `record_clip file` starts a take, plays the WAV file in real time, and
  waits for a short tail.
- `stop_print_json` runs `hush stop --print --json` and sets `TAKE_JSON`,
  `TAKE_EXIT`, and `TAKE_TEXT`. `take_field key` reads one field.
- `hush_cli args` runs the binary in the test home.
- The assertions are `assert_exit`, `assert_eq`, `assert_words`,
  `assert_file`, and `assert_log_lacks`.

Put a new clip in `tests/e2e/samples/` as 16 kHz mono 16-bit WAV, and add
its source, license, and trim to `tests/e2e/samples/SOURCES.md`.
