#!/usr/bin/env bash
# Run a command with a private PipeWire and WirePlumber, on a machine without
# a desktop session, such as a CI runner. Give it its own session bus:
#
#   dbus-run-session -- scripts/e2e/headless.sh scripts/e2e/run.sh
#
# Do not use it on a desktop: the private WirePlumber can open the sound
# devices. On a desktop, run scripts/e2e/run.sh directly.

set -euo pipefail

[ "$#" -gt 0 ] || {
  echo "usage: $0 command [argument ...]" >&2
  exit 2
}
for tool in pipewire wireplumber pw-dump pw-metadata jq timeout; do
  command -v "$tool" >/dev/null || {
    echo "$tool is missing" >&2
    exit 1
  }
done
if [ -S "${XDG_RUNTIME_DIR:-/nonexistent}/pipewire-0" ] || [ -n "${WAYLAND_DISPLAY:-}${DISPLAY:-}" ]; then
  echo "A desktop session or a PipeWire server is present; run the command without $0" >&2
  exit 1
fi

XDG_RUNTIME_DIR=$(mktemp -d "${TMPDIR:-/tmp}/hush-pipewire.XXXXXX")
export XDG_RUNTIME_DIR
unset PIPEWIRE_RUNTIME_DIR PIPEWIRE_REMOTE
PIDS=()

cleanup() {
  local status=$?
  trap - EXIT
  kill "${PIDS[@]}" 2>/dev/null || true
  wait 2>/dev/null || true
  if [ "$status" -ne 0 ]; then
    tail -n 30 "$XDG_RUNTIME_DIR"/*.log >&2 || true
  fi
  rm -rf "$XDG_RUNTIME_DIR"
  exit "$status"
}
trap cleanup EXIT

pipewire >"$XDG_RUNTIME_DIR/pipewire.log" 2>&1 &
PIDS+=("$!")
wireplumber >"$XDG_RUNTIME_DIR/wireplumber.log" 2>&1 &
PIDS+=("$!")

ready=
for _ in $(seq 100); do
  if pw-dump 2>/dev/null |
    jq -s -e 'any(.[0][]; .type == "PipeWire:Interface:Client" and .info.props["application.name"] == "WirePlumber")' \
      >/dev/null &&
    timeout 2 pw-metadata -n default 2>/dev/null | grep -q 'Found "default" metadata'; then
    ready=1
    break
  fi
  sleep 0.1
done
[ -n "$ready" ] || {
  echo "WirePlumber did not start within 10 s" >&2
  exit 1
}

"$@"
