#!/bin/sh
# Installs the latest Hush release for this machine.
#
#   curl -fsSL https://raw.githubusercontent.com/andymai/hush/main/install.sh | sh
#
# Debian and Ubuntu get the .deb, Fedora and openSUSE the .rpm (both through
# the package manager, so removal is `apt remove hush` or `dnf remove hush`),
# and everything else, Arch included, gets the tarball under
# ~/.local. Set HUSH_VERSION to install a specific tag.
set -eu

REPO="andymai/hush"
VERSION="${HUSH_VERSION:-latest}"
ARCH="$(uname -m)"

say() { printf '%s\n' "$*"; }
die() { printf 'install.sh: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "$1 is required"; }

need curl
[ "$(uname -s)" = "Linux" ] || die "Hush runs on Linux only"
[ "$ARCH" = "x86_64" ] || die "no prebuilt package for $ARCH yet; build from source (see INSTALL.md)"

if [ "$VERSION" = "latest" ]; then
  api="https://api.github.com/repos/$REPO/releases/latest"
else
  api="https://api.github.com/repos/$REPO/releases/tags/$VERSION"
fi
tag="$(curl -fsSL "$api" | sed -n 's/^ *"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)"
[ -n "$tag" ] || die "could not find a release ($api)"
version="${tag#v}"
base="https://github.com/$REPO/releases/download/$tag"

. /etc/os-release 2>/dev/null || true
id="${ID:-}"
like="${ID_LIKE:-}"
case " $id $like " in
  *" debian "*|*" ubuntu "*) kind=deb ;;
  *" fedora "*|*" rhel "*|*" centos "*|*" suse "*|*" opensuse "*) kind=rpm ;;
  # Arch takes the tarball: there is no AUR package yet.
  *" arch "*) kind=tar ;;
  *) kind=tar ;;
esac

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

fetch() {
  say "Downloading $1"
  curl -fsSL -o "$tmp/$1" "$base/$1"
  curl -fsSL -o "$tmp/SHA256SUMS" "$base/SHA256SUMS"
  (cd "$tmp" && grep " $1\$" SHA256SUMS | sha256sum -c --quiet -) || die "checksum mismatch for $1"
}

sudo_cmd() {
  if [ "$(id -u)" -eq 0 ]; then "$@"; else need sudo; sudo "$@"; fi
}

case "$kind" in
  deb)
    file="hush_${version}-1_amd64.deb"
    fetch "$file"
    sudo_cmd apt-get install -y "$tmp/$file"
    ;;
  rpm)
    file="hush-${version}-1.x86_64.rpm"
    fetch "$file"
    if command -v dnf >/dev/null 2>&1; then sudo_cmd dnf install -y "$tmp/$file"
    elif command -v zypper >/dev/null 2>&1; then sudo_cmd zypper --non-interactive install "$tmp/$file"
    else sudo_cmd rpm -i "$tmp/$file"; fi
    ;;
  tar)
    file="hush-${version}-x86_64-linux.tar.gz"
    fetch "$file"
    bin="${XDG_BIN_HOME:-$HOME/.local/bin}"
    data="${XDG_DATA_HOME:-$HOME/.local/share}"
    mkdir -p "$bin" "$data/applications" "$data/icons/hicolor/scalable/apps"
    tar -xzf "$tmp/$file" -C "$tmp"
    dir="$tmp/hush-${version}-x86_64-linux"
    install -m 755 "$dir/hush" "$bin/hush"
    sed "s|^Exec=hush|Exec=$bin/hush|" "$dir/io.github.andymai.hush.desktop" > "$data/applications/io.github.andymai.hush.desktop"
    install -m 644 "$dir/io.github.andymai.hush.svg" "$data/icons/hicolor/scalable/apps/"
    say "Installed $bin/hush"
    case ":$PATH:" in *":$bin:"*) ;; *) say "Add $bin to your PATH." ;; esac
    ;;
esac

say ""
say "Hush $version is installed. Next:"
say "  hush setup permissions      # one polkit prompt for keyboard and uinput access"
say "  hush models download base   # or: hush setup init"
say "  hush daemon start           # then hold Right Alt in any window; double-tap to keep recording"
