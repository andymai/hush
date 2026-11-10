#!/usr/bin/env bash
# Draw every page of the settings window in both schemes, under a headless
# Weston, with sample settings, and save the frames as PNG files:
#
#   scripts/screenshots.sh target/debug/hush docs/screenshots/settings
#
# The window gets its own runtime folder, configuration, data, and session
# bus, so it reads nothing of the desktop session and opens no microphone. A
# stand-in answers the window's status question as an idle daemon. The
# models folder of the user is linked in, so the Overview page shows an
# installed model.

set -euo pipefail

[ "$#" -eq 2 ] || {
  echo "usage: $0 path/to/hush output-folder" >&2
  exit 2
}
hush=$(realpath "$1")
out=$(realpath -m "$2")
for tool in weston python3; do
  command -v "$tool" >/dev/null || {
    echo "$tool is missing" >&2
    exit 1
  }
done
mkdir -p "$out"

models="${XDG_DATA_HOME:-$HOME/.local/share}/hush/models"
work=$(mktemp -d "${TMPDIR:-/tmp}/hush-shots.XXXXXX")
PIDS=()
cleanup() {
  local status=$?
  trap - EXIT
  kill "${PIDS[@]}" 2>/dev/null || true
  wait 2>/dev/null || true
  [ "$status" -eq 0 ] || tail -n 20 "$work"/*.log >&2 || true
  rm -rf "$work"
  exit "$status"
}
trap cleanup EXIT

export HOME="$work/home"
export XDG_RUNTIME_DIR="$work/run" XDG_CONFIG_HOME="$HOME/.config" XDG_DATA_HOME="$HOME/.local/share"
export DBUS_SESSION_BUS_ADDRESS="unix:path=$work/no-bus"
unset DISPLAY WAYLAND_DISPLAY HUSH_CONFIG
mkdir -p -m 700 "$XDG_RUNTIME_DIR/hush"
mkdir -p "$XDG_CONFIG_HOME/hush" "$XDG_DATA_HOME/hush"
[ -d "$models" ] && ln -s "$models" "$XDG_DATA_HOME/hush/models"

cat >"$XDG_CONFIG_HOME/hush/config.toml" <<'TOML'
[hotkey]
languages = { "Shift+RightAlt" = "de" }

[insertion]
clipboard_apps = ["org.wezfurlong.wezterm"]

[styles.casual]
prompt = "Casual and brief, like a chat message to a friend."
TOML
cat >"$XDG_CONFIG_HOME/hush/vocabulary.json" <<'JSON'
{"technical_terms": {"kubernetes": "Kubernetes", "postgresql": "PostgreSQL", "neovim": "Neovim"}}
JSON
cat >"$XDG_DATA_HOME/hush/history.jsonl" <<'JSONL'
{"at":"2026-11-10T09:12:41Z","text":"Ship the release notes after the review, and tag the build once CI is green.","app":"org.kde.konsole"}
{"at":"2026-11-10T10:03:05Z","text":"Can we move the design review to Thursday afternoon?","app":"Slack"}
{"at":"2026-11-10T11:27:58Z","text":"The migration keeps the old column until every reader moves to the new one.","app":"code"}
JSONL

python3 -I - "$XDG_RUNTIME_DIR/hush/hush.sock" >"$work/daemon.log" 2>&1 <<'PY' &
import socket, sys
server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
server.bind(sys.argv[1])
server.listen()
while True:
    connection, _ = server.accept()
    with connection:
        connection.recv(4096)
        connection.sendall(b'{"ok":true,"state":{"state":"idle"}}\n')
PY
PIDS+=("$!")

weston --backend=headless --renderer=pixman --width=1280 --height=900 \
  --socket=hush-shots --idle-time=0 >"$work/weston.log" 2>&1 &
PIDS+=("$!")
for _ in $(seq 100); do
  [ -S "$XDG_RUNTIME_DIR/hush-shots" ] && [ -S "$XDG_RUNTIME_DIR/hush/hush.sock" ] && break
  sleep 0.1
done
[ -S "$XDG_RUNTIME_DIR/hush-shots" ] || {
  echo "Weston did not start" >&2
  exit 1
}
export WAYLAND_DISPLAY=hush-shots

# shot NAME SCHEME [VARIABLE=VALUE...]: save one frame as NAME.png, or as
# NAME-light.png in the light scheme.
shot() {
  local name=$1 scheme=$2
  shift 2
  [ "$scheme" = light ] && name="$name-light"
  env "$@" HUSH_SETTINGS_APPEARANCE="$scheme" HUSH_SETTINGS_SCREENSHOT="$out/$name.png" \
    timeout 60 "$hush" settings >>"$work/hush.log" 2>&1
  [ -s "$out/$name.png" ] || {
    echo "No frame for $name" >&2
    exit 1
  }
  echo "$out/$name.png"
}

for page in overview hotkeys speech text output desktop history; do
  for scheme in dark light; do
    shot "$page" "$scheme" HUSH_SETTINGS_PAGE="$page"
  done
done
for scheme in dark light; do
  shot palette "$scheme" HUSH_SETTINGS_PAGE=speech HUSH_SETTINGS_PALETTE=
  shot palette-search "$scheme" HUSH_SETTINGS_PAGE=speech HUSH_SETTINGS_PALETTE=audio
done
