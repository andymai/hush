#!/usr/bin/env bash
# Draw the 1280x640 social preview that GitHub shows on link cards:
#
#   scripts/social-preview.sh path/to/overview-light.png docs/brand/hush-social-preview.png
#
# The screenshot is a framed settings window, as screenshots.sh draws it.
# GitHub takes the image only through the repository settings page.
set -euo pipefail

[ "$#" -eq 2 ] || { echo "usage: $0 screenshot.png out.png" >&2; exit 2; }
shot=$1
out=$2
root=$(cd "$(dirname "$0")/.." && pwd)
fonts=$root/src/gui/fonts
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

rsvg-convert -h 96 "$root/docs/brand/hush-lockup-light.svg" -o "$work/lockup.png"
magick -size 1280x640 gradient:'#ffe6a8-#ffd17a' "$work/ground.png"
magick -background none -fill '#1d1b22' -font "$fonts/Geist-SemiBold.ttf" -pointsize 44 \
  -gravity center label:"Hold a key, talk, and your words appear\nwherever you're typing." "$work/tagline.png"
magick -background none -fill '#4a3a14' -font "$fonts/Geist-Medium.ttf" -pointsize 28 \
  label:"Local dictation for Linux" "$work/sub.png"
magick "$shot" -resize 960x "$work/shot.png"
magick "$work/ground.png" \
  "$work/lockup.png" -gravity north -geometry +0+44 -composite \
  "$work/tagline.png" -gravity north -geometry +0+168 -composite \
  "$work/sub.png" -gravity north -geometry +0+290 -composite \
  "$work/shot.png" -gravity north -geometry +0+352 -composite \
  -strip "$out"
echo "$out"
