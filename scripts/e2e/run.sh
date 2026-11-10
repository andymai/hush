#!/usr/bin/env bash
# End-to-end dictation tests. A test daemon with its own home hears a clip
# through a PipeWire virtual microphone, and `hush stop --print` returns the
# text. See tests/e2e/README.md.
#
# Usage: scripts/e2e/run.sh [case ...]    (no argument runs every case)
#
# Environment:
#   HUSH_BIN                 the binary to test (default target/debug/hush, built when missing)
#   HUSH_E2E_MODEL           a ggml model file (default: tiny, downloaded once into the cache)
#   HUSH_E2E_LANGID_MODEL    a language-ID GGUF file (default: the daemon downloads it when a case needs it)
#   HUSH_E2E_PARAKEET_MODEL  a Parakeet GGUF file (default: parakeet-v3, downloaded once into the cache)
#   HUSH_E2E_KEEP=1          keep the temporary folder with the daemon logs
#   HUSH_E2E_ALLOW_UINPUT=1  run without the /dev/uinput guard (only on a machine without a desktop)

set -euo pipefail

REPO=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
SAMPLES=$REPO/tests/e2e/samples
DEFAULT_BIN=$REPO/target/debug/hush
HUSH_BIN=${HUSH_BIN:-$DEFAULT_BIN}
MODEL_CACHE=${XDG_CACHE_HOME:-$HOME/.cache}/hush-e2e
TAIL_SECS=${HUSH_E2E_TAIL_SECS:-1.5}
PW_RUNTIME=${PIPEWIRE_RUNTIME_DIR:-${XDG_RUNTIME_DIR:-/run/user/$(id -u)}}
export PIPEWIRE_RUNTIME_DIR=$PW_RUNTIME

SINK=hush-e2e-sink-$$
MIC=hush-e2e-mic-$$
# WirePlumber stores no volume or target for these nodes, and a stream never
# falls back to a default device when its target is missing.
NO_STATE='state.restore-props = false state.restore-target = false'
STREAM_PROPS="{ node.dont-fallback = true node.dont-reconnect = true node.dont-move = true $NO_STATE }"
RUN_DIR=
MIC_PID=
PLAYER_JOB=
MODEL=
DEFAULTS_BEFORE=
SANDBOX=()
HOME_ENV=()

# Set for each case.
CASE_PID=
CASE_STATUS=
CASE_DIR=
CASE_HOME=
DAEMON_PID=
TAKE_JSON=
TAKE_EXIT=
TAKE_TEXT=

log() { printf '[e2e] %s\n' "$*" >&2; }
die() {
  log "error: $*"
  exit 1
}
fail() {
  log "FAIL: $*"
  exit 1
}

# --- Assertions -------------------------------------------------------------

assert_exit() {
  local want=$1 got=$2 what=${3:-the command}
  [ "$got" = "$want" ] || fail "$what exited with $got, expected $want"
}

assert_eq() {
  local want=$1 got=$2 what=${3:-the value}
  [ "$got" = "$want" ] || fail "$what is '$got', expected '$want'"
}

# Each word must be in the text as a whole word, ignoring case and punctuation.
assert_words() {
  local text=$1 normalized word
  shift
  normalized=$(printf ' %s ' "$text" | tr '[:upper:]' '[:lower:]' | tr -c '[:alnum:]' ' ' | tr -s ' ')
  for word in "$@"; do
    word=$(printf '%s' "$word" | tr '[:upper:]' '[:lower:]')
    case "$normalized" in
      *" $word "*) ;;
      *) fail "'$word' is not in the text: $text" ;;
    esac
  done
}

# The file exists and, with a pattern, contains it (ignoring case).
assert_file() {
  local path=$1 pattern=${2:-}
  [ -f "$path" ] || fail "$path does not exist"
  if [ -n "$pattern" ]; then
    grep -qiF -- "$pattern" "$path" || fail "$path does not contain '$pattern'"
  fi
}

# The newest log file of the test daemon. Hush writes its log lines there, not
# to stdout.
hush_log_file() {
  local file
  file=$(find "$CASE_HOME/.local/share/hush/logs" -name '*.log' -printf '%T@ %p\n' 2>/dev/null |
    sort -n | tail -n 1 | cut -d' ' -f2-)
  printf '%s\n' "${file:-$CASE_HOME/.local/share/hush/logs/missing.log}"
}

# Wait until the hush log of the test daemon contains the text.
wait_for_log() {
  local text=$1 secs=${2:-60} _
  for _ in $(seq $((secs * 5))); do
    grep -qF -- "$text" "$(hush_log_file)" 2>/dev/null && return
    sleep 0.2
  done
  fail "the hush log did not show '$text' within $secs s"
}

assert_log_lacks() {
  if grep -qF -- "$1" "$CASE_DIR/daemon.log"; then
    fail "the daemon log contains '$1'"
  fi
}

# A field of the last `stop --print --json` reply; null prints as "null".
take_field() {
  jq -r --arg key "$1" '.[$key] | if . == null then "null" else tostring end' <<<"$TAKE_JSON"
}

# --- PipeWire ---------------------------------------------------------------

# pw-dump 1.0 can print a second array with the changes that came during the
# dump, so only the first array, a whole snapshot, counts.
pw_json() { pw-dump 2>/dev/null | jq -s '.[0]'; }

node_id() {
  pw_json | jq -r --arg name "$1" \
    'first(.[] | select(.type == "PipeWire:Interface:Node" and .info.props["node.name"] == $name) | .id) // empty'
}

# Print the graph, for a failure message.
pw_summary() {
  pw_json | jq -r '
    (.[] | select(.type == "PipeWire:Interface:Client")
      | "client \(.id) pid=\(.info.props["pipewire.sec.pid"]) \(.info.props["application.name"])"),
    (.[] | select(.type == "PipeWire:Interface:Node")
      | "node \(.id) client=\(.info.props["client.id"]) \(.info.props["media.class"]) \(.info.props["node.name"])"),
    (.[] | select(.type == "PipeWire:Interface:Link")
      | "link \(.info["output-node-id"]) -> \(.info["input-node-id"]) \(.info.state)")' >&2 || true
}

audio_defaults() {
  pw-metadata -n default 2>/dev/null |
    grep -oE "key:'default\.(configured\.)?audio\.(sink|source)' value:'[^']*'" | sort || true
}

# The jq definition of the node ids that belong to the processes in $pids.
# shellcheck disable=SC2016 # jq variables, not shell variables
JQ_NODES_OF_PIDS='
  def nodes_of($pids):
    [.[] | select(.type == "PipeWire:Interface:Client"
                  and ((.info.props["pipewire.sec.pid"] | tonumber? // -1) as $p | $pids | index($p)))
         | .id] as $clients
    | [.[] | select(.type == "PipeWire:Interface:Node"
                    and ((.info.props["client.id"] | tonumber? // -1) as $c | $clients | index($c)))
           | .id];
  def links: [.[] | select(.type == "PipeWire:Interface:Link") | .info];
'

# Every link into the test daemon comes from the virtual microphone, and there is one.
capture_on_mic() {
  pw_json | jq -e --argjson pid "$DAEMON_PID" --argjson mic "$(node_id "$MIC")" "$JQ_NODES_OF_PIDS"'
    nodes_of([$pid]) as $daemon
    | [links[] | select(.["input-node-id"] as $n | $daemon | index($n))]
    | length > 0 and all(.["output-node-id"] == $mic)' >/dev/null
}

# Each link that touches the virtual nodes goes to a node of one of these PIDs.
assert_mic_isolated() {
  local pids=$1
  pw_json | jq -e --argjson pids "[$pids]" --argjson ours "[$(node_id "$SINK"),$(node_id "$MIC")]" \
    "$JQ_NODES_OF_PIDS"'
    nodes_of($pids) as $allowed
    | [links[]
       | if (.["output-node-id"] as $o | $ours | index($o)) then .["input-node-id"]
         elif (.["input-node-id"] as $i | $ours | index($i)) then .["output-node-id"]
         else empty end]
    | all(. as $n | $allowed | index($n))' >/dev/null ||
    fail "a process other than the test daemon and pw-cat links to the virtual microphone"
}

# Links from the player go only to the virtual input, and there is one.
player_on_sink() {
  pw_json | jq -e --argjson pid "$1" --argjson sink "$(node_id "$SINK")" "$JQ_NODES_OF_PIDS"'
    nodes_of([$pid]) as $player
    | [links[] | select(.["output-node-id"] as $n | $player | index($n))]
    | length > 0 and all(.["input-node-id"] == $sink)' >/dev/null
}

# WirePlumber configured both nodes: they have ports, so a stream can link to them.
mic_ready() {
  pw_json | jq -e --arg sink "$SINK" --arg mic "$MIC" '
    any(.[]; .type == "PipeWire:Interface:Node" and .info.props["node.name"] == $sink and .info["n-input-ports"] > 0)
    and any(.[]; .type == "PipeWire:Interface:Node" and .info.props["node.name"] == $mic and .info["n-output-ports"] > 0)' \
    >/dev/null
}

create_mic() {
  # Short options: pw-loopback 1.0 does not know --channel-map.
  setpriv --pdeathsig TERM -- pw-loopback -n "hush-e2e-$$" -c 1 -m '[ MONO ]' \
    --capture-props "{ media.class = Audio/Sink node.name = $SINK node.description = \"Hush e2e input\" priority.session = 0 priority.driver = 0 $NO_STATE }" \
    --playback-props "{ media.class = Audio/Source node.name = $MIC node.description = \"Hush e2e microphone\" priority.session = 0 priority.driver = 0 $NO_STATE }" \
    >"$RUN_DIR/mic.log" 2>&1 &
  MIC_PID=$!
  local _
  for _ in $(seq 50); do
    if mic_ready; then
      log "The virtual microphone $MIC is ready"
      return
    fi
    kill -0 "$MIC_PID" 2>/dev/null || die "pw-loopback exited: $(cat "$RUN_DIR/mic.log")"
    sleep 0.2
  done
  die "the virtual microphone did not appear"
}

remove_mic() {
  [ -n "$MIC_PID" ] || return 0
  kill "$MIC_PID" 2>/dev/null || true
  wait "$MIC_PID" 2>/dev/null || true
  MIC_PID=
  local _
  for _ in $(seq 25); do
    [ -z "$(node_id "$SINK")$(node_id "$MIC")" ] && return 0
    sleep 0.2
  done
  log "warning: the virtual nodes $SINK and $MIC are still present"
}

# Start to play a WAV file into the virtual microphone in real time, and set
# PLAYER_JOB.
start_player() {
  setpriv --pdeathsig TERM -- timeout --kill-after=5 120 setpriv --pdeathsig TERM -- \
    pw-cat --playback --target "$SINK" \
    --properties "$STREAM_PROPS" \
    "$1" &
  PLAYER_JOB=$!
}

# The PID of the pw-cat process that plays into the virtual input.
player_pid() {
  local _ pid
  for _ in $(seq 30); do
    pid=$(pgrep -f "^pw-cat --playback --target $SINK " | head -n 1 || true)
    [ -n "$pid" ] && { echo "$pid"; return; }
    sleep 0.1
  done
  return 1
}

# Play silence once and make sure that the player links only to the virtual
# input before any speech plays.
probe_route() {
  local probe=$RUN_DIR/probe.wav player ok='' _
  make_silence_wav 4 "$probe"
  start_player "$probe"
  if player=$(player_pid); then
    for _ in $(seq 30); do
      if player_on_sink "$player"; then
        ok=1
        break
      fi
      sleep 0.1
    done
  fi
  [ -n "$ok" ] || pw_summary
  wait "$PLAYER_JOB" || true
  [ -n "$ok" ] || die "pw-cat (PID ${player:-none}) did not link to $SINK only"
  log "pw-cat plays into $SINK only"
}

# --- Files ------------------------------------------------------------------

le16() { printf '%b' "\\x$(printf %02x $(($1 & 255)))\\x$(printf %02x $((($1 >> 8) & 255)))"; }
le32() {
  le16 $(($1 & 65535))
  le16 $((($1 >> 16) & 65535))
}

# A 16 kHz mono 16-bit WAV file of digital silence.
make_silence_wav() {
  local secs=$1 out=$2 bytes
  bytes=$((secs * 16000 * 2))
  {
    printf 'RIFF'
    le32 $((36 + bytes))
    printf 'WAVEfmt '
    le32 16
    le16 1
    le16 1
    le32 16000
    le32 32000
    le16 2
    le16 16
    printf 'data'
    le32 "$bytes"
    head -c "$bytes" /dev/zero
  } >"$out"
}

ensure_model() {
  if [ -n "${HUSH_E2E_LANGID_MODEL:-}" ]; then
    [ -f "$HUSH_E2E_LANGID_MODEL" ] || die "HUSH_E2E_LANGID_MODEL=$HUSH_E2E_LANGID_MODEL is not a file"
    HUSH_E2E_LANGID_MODEL=$(realpath "$HUSH_E2E_LANGID_MODEL")
  fi
  if [ -n "${HUSH_E2E_MODEL:-}" ]; then
    [ -f "$HUSH_E2E_MODEL" ] || die "HUSH_E2E_MODEL=$HUSH_E2E_MODEL is not a file"
    MODEL=$HUSH_E2E_MODEL
    return
  fi
  MODEL=$MODEL_CACHE/ggml-tiny.bin
  [ -s "$MODEL" ] && return
  log "Downloading the tiny model once into $MODEL_CACHE"
  local home=$RUN_DIR/download file name
  make_home "$home"
  in_home "$home" "$HUSH_BIN" models download tiny >&2
  mkdir -p "$MODEL_CACHE"
  for file in "$home"/.local/share/hush/models/*.bin; do
    name=$(basename "$file")
    cp "$file" "$MODEL_CACHE/.$name.part"
    mv "$MODEL_CACHE/.$name.part" "$MODEL_CACHE/$name"
  done
  [ -s "$MODEL" ] || die "the download gave no $MODEL"
}

# The Parakeet model for the parakeet case: the override, or the catalogue
# file, downloaded once into the cache through `hush models download`.
ensure_parakeet() {
  if [ -n "${HUSH_E2E_PARAKEET_MODEL:-}" ]; then
    [ -f "$HUSH_E2E_PARAKEET_MODEL" ] || fail "HUSH_E2E_PARAKEET_MODEL=$HUSH_E2E_PARAKEET_MODEL is not a file"
    PARAKEET=$(realpath "$HUSH_E2E_PARAKEET_MODEL")
    return
  fi
  PARAKEET=$MODEL_CACHE/parakeet-tdt-0.6b-v3-Q8_0.gguf
  [ -s "$PARAKEET" ] && return
  log "Downloading the Parakeet model (740 MB) once into $MODEL_CACHE"
  local home=$CASE_DIR/download
  make_home "$home"
  in_home "$home" "$HUSH_BIN" models download parakeet-v3 >&2
  mkdir -p "$MODEL_CACHE"
  cp "$home/.local/share/hush/models/parakeet-tdt-0.6b-v3-Q8_0.gguf" "$PARAKEET.part"
  mv "$PARAKEET.part" "$PARAKEET"
}

# --- Test home and daemon ---------------------------------------------------

make_home() {
  mkdir -p "$1/.config/hush" "$1/.local/share/hush/models" "$1/.local/state" "$1/.cache" "$1/run"
  chmod 700 "$1/run"
}

# The whole environment of a test process: the test home, the PipeWire socket,
# and PATH. Nothing else from the caller's session: no display, no session bus.
home_env() {
  local home=$1
  HOME_ENV=(
    PATH="$PATH" LANG=C.UTF-8 HOME="$home"
    XDG_CONFIG_HOME="$home/.config" XDG_DATA_HOME="$home/.local/share"
    XDG_STATE_HOME="$home/.local/state" XDG_CACHE_HOME="$home/.cache"
    XDG_RUNTIME_DIR="$home/run"
    PIPEWIRE_RUNTIME_DIR="$PW_RUNTIME" PIPEWIRE_NODE="$MIC"
    PIPEWIRE_PROPS="$STREAM_PROPS"
  )
}

in_home() {
  home_env "$1"
  shift
  env -i "${HOME_ENV[@]}" "$@"
}

hush_cli() { in_home "$CASE_HOME" "$HUSH_BIN" "$@"; }

# Hide /dev/uinput from the test daemon, so that it cannot type into a window
# even when a take ends in a way that the harness did not plan.
setup_sandbox() {
  [ -e /dev/uinput ] || return 0
  # shellcheck disable=SC2016 # $@ expands in the inner shell
  local wrapper=(unshare --user --map-root-user --mount sh -c 'mount --bind /dev/null /dev/uinput && exec "$@"' sh)
  if "${wrapper[@]}" true 2>/dev/null; then
    SANDBOX=("${wrapper[@]}")
    log "The test daemon runs with /dev/uinput hidden"
  elif [ ! -w /dev/uinput ]; then
    log "The test daemon cannot write /dev/uinput"
  elif [ "${HUSH_E2E_ALLOW_UINPUT:-}" = 1 ]; then
    log "warning: the test daemon can write /dev/uinput (HUSH_E2E_ALLOW_UINPUT=1)"
  else
    die "cannot hide /dev/uinput from the test daemon, because unprivileged user namespaces are off." \
      "Set HUSH_E2E_ALLOW_UINPUT=1 only on a machine without a desktop session."
  fi
}

# A case's TOML (dotted keys, such as `transcription.language = "de"`) replaces
# the base value of the same key.
write_config() {
  local extra=$1 line key
  {
    [ -z "$extra" ] || printf '%s\n' "$extra"
    while IFS= read -r line; do
      key=${line%% =*}
      grep -qE "^[[:space:]]*${key//./\\.}[[:space:]]*=" <<<"$extra" || printf '%s\n' "$line"
    done <<EOF
hotkey.enabled = false
tray.enabled = false
feedback.audio_enabled = false
llm.provider = "none"
audio.pause_media = false
audio.silence_stop_secs = 0
audio.max_recording_secs = 0
transcription.backend = "local"
transcription.model_path = "$MODEL"
transcription.context_prompt = false
updates.check = false
EOF
  } >"$CASE_HOME/.config/hush/config.toml"
}

link_langid_model() {
  ln -sf "$HUSH_E2E_LANGID_MODEL" "$CASE_HOME/.local/share/hush/models/lang-id-voxlingua107-ecapa-Q8_0.gguf"
}

# Start the test daemon with the base config and the case's TOML, and wait for
# its socket.
start_daemon() {
  write_config "${1:-}"
  local vad _
  for vad in "$MODEL_CACHE"/ggml-silero-*.bin; do
    [ -f "$vad" ] && ln -sf "$vad" "$CASE_HOME/.local/share/hush/models/"
  done
  if [ -n "${HUSH_E2E_LANGID_MODEL:-}" ] && [ -z "${SKIP_LANGID_LINK:-}" ]; then
    link_langid_model
  fi
  home_env "$CASE_HOME"
  "${SANDBOX[@]}" setpriv --pdeathsig TERM -- env -i "${HOME_ENV[@]}" \
    "$HUSH_BIN" -v daemon start --foreground >"$CASE_DIR/daemon.log" 2>&1 &
  DAEMON_PID=$!
  echo "$DAEMON_PID" >"$CASE_DIR/daemon.pid"
  for _ in $(seq 300); do
    kill -0 "$DAEMON_PID" 2>/dev/null || fail "the test daemon exited during startup"
    if hush_cli daemon status 2>/dev/null | grep -q "is running"; then
      [ "$(cat "$CASE_HOME/run/hush/hush.pid" 2>/dev/null)" = "$DAEMON_PID" ] && return
      kill -0 "$DAEMON_PID" 2>/dev/null || fail "the test daemon exited during startup"
      fail "the socket in $CASE_HOME/run belongs to another process"
    fi
    sleep 0.2
  done
  fail "the test daemon did not open its socket within 60 s"
}

# Stop the daemon of a case folder, but only a process that has that case's
# runtime folder, so that a reused PID never gets the signal.
stop_daemon() {
  local dir=$1 pid _
  [ -f "$dir/daemon.pid" ] || return 0
  pid=$(cat "$dir/daemon.pid")
  rm -f "$dir/daemon.pid"
  grep -qzF "XDG_RUNTIME_DIR=$dir/home/run" "/proc/$pid/environ" 2>/dev/null || return 0
  kill -TERM "$pid" 2>/dev/null || return 0
  for _ in $(seq 50); do
    kill -0 "$pid" 2>/dev/null || return 0
    sleep 0.2
  done
  log "warning: the test daemon $pid ignored SIGTERM, so it gets SIGKILL"
  kill -KILL "$pid" 2>/dev/null || true
}

# --- Takes ------------------------------------------------------------------

wait_recording() {
  local _
  for _ in $(seq 100); do
    case "$(hush_cli daemon status 2>/dev/null)" in
      *": recording"*) return ;;
    esac
    sleep 0.1
  done
  fail "the take did not start to record within 10 s"
}

# Start a take, play the clip into the virtual microphone in real time, and
# wait a short tail. The case then stops the take. Arguments after the clip go
# to `hush start`.
record_clip() {
  local clip=$1 player _
  shift
  hush_cli start "$@" >/dev/null || fail "hush start $* failed"
  wait_recording
  for _ in $(seq 20); do
    capture_on_mic && break
    sleep 0.1
  done
  capture_on_mic || {
    pw_summary
    fail "the test daemon (PID $DAEMON_PID) does not capture from $MIC only"
  }
  assert_mic_isolated "$DAEMON_PID"
  start_player "$clip"
  player=$(player_pid) || fail "pw-cat did not start"
  assert_mic_isolated "$DAEMON_PID,$player"
  wait "$PLAYER_JOB" || fail "pw-cat could not play $clip"
  sleep "$TAIL_SECS"
}

# `hush stop --print --json`: sets TAKE_JSON, TAKE_EXIT, and TAKE_TEXT.
stop_print_json() {
  TAKE_EXIT=0
  TAKE_JSON=$(hush_cli stop --print --json) || TAKE_EXIT=$?
  log "stop --print --json: exit $TAKE_EXIT, $TAKE_JSON"
  TAKE_TEXT=$(jq -r '.text // ""' <<<"$TAKE_JSON")
}

# --- Cases ------------------------------------------------------------------
# A case is a function named case_<name>. It runs in a subshell with its own
# CASE_DIR and CASE_HOME, and a failed assertion ends it.

case_english_clip() {
  start_daemon
  record_clip "$SAMPLES/jfk.wav"
  stop_print_json
  assert_exit 0 "$TAKE_EXIT" "hush stop --print --json"
  assert_eq ok "$(take_field status)" "the status"
  assert_words "$TAKE_TEXT" ask country
  assert_file "$CASE_HOME/.local/share/hush/history.jsonl" country
}

case_silence() {
  start_daemon
  make_silence_wav 3 "$CASE_DIR/silence.wav"
  record_clip "$CASE_DIR/silence.wav"
  stop_print_json
  assert_exit 4 "$TAKE_EXIT" "hush stop --print --json"
  assert_eq no_speech "$(take_field status)" "the status"
  assert_eq null "$(take_field text)" "the text"
}

# The daemon drops a silent take before Whisper, also without VAD, so Whisper
# cannot invent a word such as "you" for it.
case_silence_without_vad() {
  start_daemon 'transcription.vad = false'
  make_silence_wav 3 "$CASE_DIR/silence.wav"
  record_clip "$CASE_DIR/silence.wav"
  stop_print_json
  assert_exit 4 "$TAKE_EXIT" "hush stop --print --json"
  assert_eq no_speech "$(take_field status)" "the status"
  assert_eq null "$(take_field text)" "the text"
}

# Parakeet hears both clips with no language set: it tells English and German
# apart on its own, and it does not use detect_languages.
case_parakeet() {
  ensure_parakeet
  start_daemon "transcription.model_size = \"parakeet-v3\"
transcription.model_path = \"$PARAKEET\"
transcription.language = \"auto\"
transcription.detect_languages = [\"en\", \"de\"]"
  wait_for_log "Parakeet model loaded on"
  wait_for_log "Parakeet tells its own languages apart"
  record_clip "$SAMPLES/jfk.wav"
  stop_print_json
  assert_exit 0 "$TAKE_EXIT" "hush stop --print --json"
  assert_words "$TAKE_TEXT" fellow americans ask country
  record_clip "$SAMPLES/verwandlung_de.wav"
  stop_print_json
  assert_exit 0 "$TAKE_EXIT" "hush stop --print --json"
  assert_words "$TAKE_TEXT" gregor samsa morgens bett verwandelt
}

case_german_lang() {
  start_daemon
  record_clip "$SAMPLES/verwandlung_de.wav" --lang de
  stop_print_json
  assert_exit 0 "$TAKE_EXIT" "hush stop --print --json"
  assert_words "$TAKE_TEXT" samsa morgens bett verwandelt
}

# With llm.provider = "none", a style does not change the text, so the case
# checks the path of the style: the flag, the socket, and the take. A take
# through `hush start` is in parts, and the style must reach the joined text.
# Two copies of the JFK clip with a pause last about 24 s.
case_style_flag() {
  local clip=$CASE_DIR/long.wav pause=$CASE_DIR/pause.wav code=0 out parts
  command -v sox >/dev/null || fail "sox is missing; this case joins clips with it"
  make_silence_wav 2 "$pause"
  sox "$SAMPLES/jfk.wav" "$pause" "$SAMPLES/jfk.wav" "$clip"
  start_daemon
  out=$(hush_cli start --style nosuch 2>&1) || code=$?
  log "start --style nosuch: exit $code, $out"
  assert_exit 2 "$code" "hush start --style nosuch"
  case "$(hush_cli daemon status)" in
    *": idle"*) ;;
    *) fail "hush start --style nosuch started a take" ;;
  esac
  record_clip "$clip" --style verbatim
  stop_print_json
  assert_exit 0 "$TAKE_EXIT" "hush stop --print --json"
  assert_words "$TAKE_TEXT" ask country
  parts=$(grep -o 'ends with [0-9]* parts' "$(hush_log_file)" | grep -o '[0-9]*' | tail -n 1)
  [ "${parts:-0}" -ge 2 ] || fail "the take was not cut into parts (parts: ${parts:-none})"
  assert_file "$(hush_log_file)" "The take uses the style verbatim"
}

# The language-ID model must pick the language. Whisper's own detection, the
# fallback without the model, writes a different line.
case_detect_limited() {
  start_daemon 'transcription.language = "auto"
transcription.detect_languages = ["en", "de"]'
  wait_for_log "Language identification is ready"
  record_clip "$SAMPLES/verwandlung_de.wav"
  stop_print_json
  assert_exit 0 "$TAKE_EXIT" "hush stop --print --json"
  assert_words "$TAKE_TEXT" samsa morgens bett verwandelt
  assert_file "$(hush_log_file)" "Language identification is ready"
  assert_file "$(hush_log_file)" "Language ID picked de among en, de"
}

# A daemon that starts without the language-ID model, for example before the
# network is up, uses the model from the first take after the file appears.
case_detect_limited_late_model() {
  if [ -z "${HUSH_E2E_LANGID_MODEL:-}" ]; then
    log "detect_limited_late_model needs HUSH_E2E_LANGID_MODEL, so it checks nothing"
    return 0
  fi
  SKIP_LANGID_LINK=1 start_daemon 'transcription.language = "auto"
transcription.detect_languages = ["en", "de"]'
  record_clip "$SAMPLES/verwandlung_de.wav"
  stop_print_json
  assert_exit 0 "$TAKE_EXIT" "hush stop --print --json"
  assert_words "$TAKE_TEXT" samsa
  link_langid_model
  record_clip "$SAMPLES/verwandlung_de.wav"
  stop_print_json
  assert_exit 0 "$TAKE_EXIT" "hush stop --print --json"
  assert_words "$TAKE_TEXT" samsa
  assert_file "$(hush_log_file)" "Language ID picked de among en, de"
}

case_keep_audio() {
  start_daemon 'history.keep_audio_days = 1'
  record_clip "$SAMPLES/jfk.wav"
  stop_print_json
  assert_exit 0 "$TAKE_EXIT" "hush stop --print --json"
  assert_words "$TAKE_TEXT" ask country
  local flac text code=0
  flac=$(find "$CASE_HOME/.local/share/hush/audio" -name '*.flac' 2>/dev/null | head -n 1)
  [ -n "$flac" ] || fail "the audio folder has no FLAC file"
  text=$(hush_cli transcribe "$flac") || code=$?
  log "transcribe: exit $code, $text"
  assert_exit 0 "$code" "hush transcribe"
  assert_words "$text" ask country
}

case_no_audio_by_default() {
  start_daemon
  record_clip "$SAMPLES/jfk.wav"
  stop_print_json
  assert_exit 0 "$TAKE_EXIT" "hush stop --print --json"
  [ -z "$(find "$CASE_HOME/.local/share/hush/audio" -name '*.flac' 2>/dev/null)" ] ||
    fail "Hush saved take audio with keep_audio_days = 0"
}

# A take through `hush start` is hands-free, so Hush transcribes it in parts
# while it records. Four copies of the JFK clip with pauses last about 50 s.
case_chunked_take() {
  local clip=$CASE_DIR/long.wav pause=$CASE_DIR/pause.wav parts
  command -v sox >/dev/null || fail "sox is missing; this case joins clips with it"
  make_silence_wav 2 "$pause"
  sox "$SAMPLES/jfk.wav" "$pause" "$SAMPLES/jfk.wav" "$pause" \
    "$SAMPLES/jfk.wav" "$pause" "$SAMPLES/jfk.wav" "$clip"
  start_daemon
  record_clip "$clip"
  stop_print_json
  assert_exit 0 "$TAKE_EXIT" "hush stop --print --json"
  assert_words "$TAKE_TEXT" ask country fellow americans
  [ "$(grep -o -i country <<<"$TAKE_TEXT" | wc -l)" -ge 6 ] ||
    fail "the joined text lost parts: $TAKE_TEXT"
  parts=$(grep -o 'ends with [0-9]* parts' "$(hush_log_file)" | grep -o '[0-9]*' | tail -n 1)
  [ "${parts:-0}" -ge 2 ] || fail "the take was not cut into parts (parts: ${parts:-none})"
  [ -z "$(find "$CASE_HOME/.local/share/hush/parts" -type f 2>/dev/null)" ] ||
    fail "part files stay after the text went out"
}

# Every part of a hands-free take is in the language of the take, not only
# the first. Two copies of the German clip with a pause last about 21 s.
case_chunked_german() {
  local clip=$CASE_DIR/long_de.wav pause=$CASE_DIR/pause.wav parts
  command -v sox >/dev/null || fail "sox is missing; this case joins clips with it"
  make_silence_wav 2 "$pause"
  sox "$SAMPLES/verwandlung_de.wav" "$pause" "$SAMPLES/verwandlung_de.wav" "$clip"
  start_daemon
  record_clip "$clip" --lang de
  stop_print_json
  assert_exit 0 "$TAKE_EXIT" "hush stop --print --json"
  assert_words "$TAKE_TEXT" samsa morgens bett verwandelt
  [ "$(grep -o -i samsa <<<"$TAKE_TEXT" | wc -l)" -ge 2 ] ||
    fail "a part is not in German: $TAKE_TEXT"
  parts=$(grep -o 'ends with [0-9]* parts' "$(hush_log_file)" | grep -o '[0-9]*' | tail -n 1)
  [ "${parts:-0}" -ge 2 ] || fail "the take was not cut into parts (parts: ${parts:-none})"
}

case_print_types_nothing() {
  start_daemon
  record_clip "$SAMPLES/jfk.wav"
  local text code=0
  text=$(hush_cli stop --print) || code=$?
  log "stop --print: exit $code, $text"
  assert_exit 0 "$code" "hush stop --print"
  assert_words "$text" ask country
  assert_log_lacks "Text inserted"
  assert_log_lacks "Pressed Enter"
}

# --- Runner -----------------------------------------------------------------

all_cases() { compgen -A function case_ | sed 's/^case_//'; }

preflight() {
  local tool
  for tool in pw-cat pw-loopback pw-dump pw-metadata jq setpriv timeout pgrep; do
    command -v "$tool" >/dev/null || die "$tool is missing (PipeWire tools, jq, util-linux, procps)"
  done
  [ -S "$PW_RUNTIME/pipewire-0" ] || die "no PipeWire socket at $PW_RUNTIME/pipewire-0"
  if [ ! -x "$HUSH_BIN" ]; then
    [ "$HUSH_BIN" = "$DEFAULT_BIN" ] || die "HUSH_BIN=$HUSH_BIN is not an executable"
    log "Building $DEFAULT_BIN"
    (cd "$REPO" && cargo build)
  fi
}

# Sets CASE_STATUS. The caller must not call it in a condition (`if`, `||`):
# bash then ignores `set -e` in the case.
run_case() {
  local name=$1 status
  CASE_DIR=$RUN_DIR/$name
  CASE_HOME=$CASE_DIR/home
  make_home "$CASE_HOME"
  log "--- $name"
  # In the background, so that a signal reaches the trap at once.
  (
    set -e
    "case_$name"
  ) &
  CASE_PID=$!
  status=0
  wait "$CASE_PID" || status=$?
  CASE_PID=
  stop_daemon "$CASE_DIR"
  if [ "$status" -eq 0 ]; then
    log "PASS: $name"
  else
    log "Last lines of the daemon log of $name:"
    tail -n 40 "$CASE_DIR/daemon.log" >&2 2>/dev/null || true
  fi
  CASE_STATUS=$status
}

cleanup() {
  local status=$? dir
  trap - EXIT INT TERM HUP
  set +e
  if [ -n "$CASE_PID" ]; then
    kill -TERM "$CASE_PID" 2>/dev/null
    wait "$CASE_PID" 2>/dev/null
  fi
  if [ -n "$RUN_DIR" ]; then
    for dir in "$RUN_DIR"/*/; do
      stop_daemon "${dir%/}"
    done
  fi
  remove_mic
  if [ -n "$DEFAULTS_BEFORE" ] && [ "$(audio_defaults)" != "$DEFAULTS_BEFORE" ]; then
    log "error: the default sink or source changed during the run"
    status=1
  fi
  if [ -n "$RUN_DIR" ]; then
    if [ "${HUSH_E2E_KEEP:-}" = 1 ]; then
      log "The logs are in $RUN_DIR"
    else
      rm -rf "$RUN_DIR"
    fi
  fi
  exit "$status"
}

main() {
  local cases=("$@") name failed=()
  [ "${#cases[@]}" -gt 0 ] || mapfile -t cases < <(all_cases)
  for name in "${cases[@]}"; do
    declare -F "case_$name" >/dev/null || die "no case '$name'; the cases are: $(all_cases | paste -sd' ' -)"
  done
  preflight
  RUN_DIR=$(mktemp -d "${TMPDIR:-/tmp}/hush-e2e.XXXXXX")
  trap cleanup EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM HUP
  setup_sandbox
  ensure_model
  DEFAULTS_BEFORE=$(audio_defaults)
  create_mic
  probe_route
  for name in "${cases[@]}"; do
    run_case "$name"
    [ "$CASE_STATUS" -eq 0 ] || failed+=("$name")
  done
  if [ "${#failed[@]}" -gt 0 ]; then
    log "${#failed[@]} of ${#cases[@]} cases failed: ${failed[*]}"
    exit 1
  fi
  log "All ${#cases[@]} cases passed"
}

main "$@"
