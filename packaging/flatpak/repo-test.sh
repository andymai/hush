#!/usr/bin/env bash
# Runs two builds through repo.sh as the release workflow does, over a local
# HTTP server: OLD is published and installed, then NEW goes on top of it and
# the install updates. NEW's commit must have OLD's as its parent, and OLD's
# commit must stay in the repository. A throwaway key and private folders keep
# the test apart from the real keyring and installation.
#
#   packaging/flatpak/repo-test.sh OLD.flatpak NEW.flatpak
set -euo pipefail

usage="usage: repo-test.sh OLD.flatpak NEW.flatpak"
old=$(realpath "${1:?$usage}")
new=$(realpath "${2:?$usage}")
here=$(dirname "$(realpath "$0")")
app=io.github.andymai.hush
ref=app/$app/x86_64/master

work=$(mktemp -d)
server=
trap '[[ -z $server ]] || kill "$server"; rm -rf "$work"' EXIT
export GNUPGHOME=$work/gnupg FLATPAK_USER_DIR=$work/flatpak PUBLISHED_KEY=$work/key.asc
mkdir -m 700 "$GNUPGHOME"
gpg --batch --passphrase '' --quick-gen-key "Hush test key" rsa2048 sign never 2> /dev/null
gpg --batch --armor --export > "$PUBLISHED_KEY"

# The server serves $work, and the symlink `live` says which site is published.
port=$(python3 -I -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')
python3 -I -m http.server "$port" --bind 127.0.0.1 --directory "$work" > /dev/null 2>&1 &
server=$!
url=http://127.0.0.1:$port/live
for _ in {1..50}; do
  curl -s -o /dev/null "http://127.0.0.1:$port/" && break
  sleep 0.1
done

"$here/repo.sh" "$old" "$work/one" "$url"
ln -sfn one "$work/live"
flatpak --user remote-add test "$work/one/hush.flatpakrepo"
flatpak --user install -y --noninteractive --no-deps --no-related test "$app"
first=$(ostree rev-parse --repo="$work/one/repo" "$ref")

"$here/repo.sh" "$new" "$work/two" "$url"
ln -sfn two "$work/live"
flatpak --user update -y --noninteractive --no-deps --no-related "$app"
second=$(ostree rev-parse --repo="$work/two/repo" "$ref")

fail() { echo "$1" >&2; exit 1; }
[[ $second != "$first" ]] || fail "NEW made no new commit: give two different builds"
[[ $(ostree rev-parse --repo="$work/two/repo" "$second^") == "$first" ]] || fail "NEW's commit doesn't have OLD's as its parent"
ostree show --repo="$work/two/repo" "$first" > /dev/null || fail "OLD's commit is gone from the repository"
[[ $(flatpak --user info --show-commit "$app") == "$second" ]] || fail "the install didn't update to NEW"
echo "updated $first -> $second"
