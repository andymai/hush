#!/bin/sh
# Writes cargo-sources.json, the crates that the Flatpak build gets offline,
# from Cargo.lock. Run it after each change to Cargo.lock:
#
#   packaging/flatpak/update-cargo-sources.sh
#
# It uses flatpak-cargo-generator.py from flatpak-builder-tools at a pinned
# commit, checks its SHA-256, and runs it with uv, which installs the
# dependencies that the script declares.
set -eu

GENERATOR_COMMIT=41c20aa10819cdb2a4f3ca171758a96d1955c018
GENERATOR_SHA256=0a2db6be87d75910facef28ab46d4d6460802e8419ab850d0caa6a364d26b380
GENERATOR_URL="https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/$GENERATOR_COMMIT/cargo/flatpak-cargo-generator.py"

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
command -v uv >/dev/null 2>&1 || { echo "update-cargo-sources.sh needs uv: https://docs.astral.sh/uv/" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
curl -fsSL -o "$work/flatpak-cargo-generator.py" "$GENERATOR_URL"
echo "$GENERATOR_SHA256  $work/flatpak-cargo-generator.py" | sha256sum -c --quiet -
uv run --quiet --script "$work/flatpak-cargo-generator.py" "$repo/Cargo.lock" -o "$here/cargo-sources.json"
echo "Wrote $here/cargo-sources.json"
