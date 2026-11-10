#!/usr/bin/env bash
# Subset Lucide's icon font to the icons the settings window draws.
#
#   scripts/icon-font.sh path/to/lucide-static
#
# Take lucide-static from npm (`npm pack lucide-static`, then unpack it). The
# script needs pyftsubset (pip install fonttools) and jq. It writes
# src/gui/fonts/lucide.ttf and prints each icon's code point for
# src/gui/icons.rs. To add an icon, add its Lucide name to ICONS and run the
# script again.

set -euo pipefail

ICONS=(
  app-window check chevron-down circle-alert circle-check
  circle-stop circle-x copy download file-input file-output file-text
  history house info keyboard languages mic minus monitor pen-line pencil
  play plus power refresh-cw rotate-ccw rotate-cw search shield-check square
  text-cursor-input trash-2 triangle-alert type whole-word x
)

[ "$#" -eq 1 ] || {
  echo "usage: $0 path/to/lucide-static" >&2
  exit 2
}
font="$1/font"
for tool in pyftsubset jq; do
  command -v "$tool" >/dev/null || {
    echo "$tool is missing" >&2
    exit 1
  }
done

out="$(git rev-parse --show-toplevel)/src/gui/fonts/lucide.ttf"
unicodes=()
for name in "${ICONS[@]}"; do
  point=$(jq -e --arg name "$name" '.[$name]' "$font/codepoints.json") || {
    echo "Lucide has no icon named $name" >&2
    exit 1
  }
  unicodes+=("$(printf 'U+%04X' "$point")")
  printf '%-20s \\u{%x}\n' "$name" "$point"
done

pyftsubset "$font/lucide.ttf" \
  --unicodes="$(IFS=,; echo "${unicodes[*]}")" \
  --no-hinting --desubroutinize \
  --name-IDs='*' --output-file="$out"
echo "Wrote $out ($(stat -c %s "$out") bytes)"
