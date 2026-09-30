#!/bin/sh
# Install sprid for this user from an unpacked release: the binary into
# ~/.local/bin, its desktop entry and icon into ~/.local/share, so a
# launcher finds it. PREFIX=/usr/local (as root) installs it for everyone.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
prefix=${PREFIX:-$HOME/.local}
install -Dm755 "$here/sprid" "$prefix/bin/sprid"
install -Dm644 "$here/sprid.desktop" "$prefix/share/applications/sprid.desktop"
install -Dm644 "$here/sprid.png" "$prefix/share/icons/hicolor/1024x1024/apps/sprid.png"
echo "installed $prefix/bin/sprid"
case ":$PATH:" in *":$prefix/bin:"*) ;; *) echo "note: $prefix/bin is not on PATH" ;; esac
