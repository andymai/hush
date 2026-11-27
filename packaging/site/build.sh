#!/usr/bin/env bash
# Copy the landing page and the images it uses into a site folder that
# repo.sh already wrote, so the page and the Flatpak remote deploy together:
#
#   packaging/site/build.sh SITE
#
# The images come from docs/, so the page shows the same lockup and
# screenshots as the README.
set -euo pipefail

site=${1:?usage: build.sh SITE}
[ -d "$site" ] || { echo "$site is not a folder" >&2; exit 1; }
root=$(cd "$(dirname "$0")/../.." && pwd)

install -m644 "$root/packaging/site/index.html" "$site/index.html"
install -m644 "$root/packaging/io.github.andymai.hush.svg" "$site/icon.svg"
install -m644 "$root/docs/brand/hush-lockup-light.svg" "$site/lockup-light.svg"
install -m644 "$root/docs/brand/hush-lockup-dark.svg" "$site/lockup-dark.svg"
install -m644 "$root/docs/brand/hush-social-preview.png" "$site/social-preview.png"
install -m644 "$root/docs/screenshots/settings/overview.png" "$site/overview.png"
install -m644 "$root/docs/screenshots/settings/overview-light.png" "$site/overview-light.png"

# Each local file that the page names must be in the folder.
missing=0
while read -r ref; do
  [ -e "$site/$ref" ] || { echo "index.html names $ref, which is not in $site" >&2; missing=1; }
done < <(grep -oE '(src|srcset|href)="[^":#]+"' "$site/index.html" | sed -E 's/^[a-z]+="([^"]+)"$/\1/' | sort -u)
exit "$missing"
