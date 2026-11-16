#!/usr/bin/env bash
# Checks a repository that repo.sh built, as a store would use it. Its
# hush.flatpakrepo and .flatpakref must name URL/repo/. Then a copy of the
# .flatpakrepo with a file:// Url adds the remote, Hush installs from the
# repository on disk, and Hush must be in its AppStream data. The copy keeps the
# check off the published site: outside a release, the published summary has
# another key. A private FLATPAK_USER_DIR keeps the real installation out of it.
#
#   packaging/flatpak/repo-check.sh SITE URL
set -euo pipefail

usage="usage: repo-check.sh SITE URL"
site=$(realpath "${1:?$usage}")
url=${2:?$usage}
url=${url%/}
app=io.github.andymai.hush

for file in hush.flatpakrepo "$app.flatpakref"; do
  grep -qxF "Url=$url/repo/" "$site/$file" || { echo "$file doesn't name $url/repo/" >&2; exit 1; }
done

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
export FLATPAK_USER_DIR=$work/flatpak
mkdir "$FLATPAK_USER_DIR"

local_url=$(python3 -I -c 'import pathlib, sys; print(pathlib.Path(sys.argv[1]).as_uri() + "/")' "$site/repo")
{
  grep -v '^Url=' "$site/hush.flatpakrepo"
  echo "Url=$local_url"
} > "$work/check.flatpakrepo"

flatpak --user remote-add check "$work/check.flatpakrepo"
flatpak --user install -y --noninteractive --no-deps --no-related check "$app"
flatpak --user update -y --noninteractive --appstream check
flatpak --user search --columns=application "$app" | grep -qx "$app" ||
  { echo "$app is not in the repository's AppStream data" >&2; exit 1; }

# A store shows the screenshots and the icon that the AppStream data names. A
# screenshot URL on dl.flathub.org means a build that expected the Flathub
# pipeline to upload the media, and nothing serves it for this repository.
appstream=$FLATPAK_USER_DIR/appstream/check/x86_64/active/appstream.xml.gz
[[ -f $appstream ]] || { echo "no AppStream data at $appstream" >&2; exit 1; }
if zcat "$appstream" | grep -q 'https://dl.flathub.org/'; then
  echo "the AppStream data names dl.flathub.org, which has no media for this repository" >&2
  exit 1
fi
flatpak --user info "$app" | grep -E 'Version|Commit'
