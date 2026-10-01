#!/bin/sh
# Set sprid's version everywhere it is written: almide.toml (the package,
# the app bundle, release file names) and src/terminal.almd (what XTVERSION
# reports). Usage: scripts/bump-version.sh 0.3.0
set -eu
v=${1:?usage: scripts/bump-version.sh X.Y.Z}
case $v in [0-9]*.[0-9]*.[0-9]*) ;; *) echo "not a version: $v" >&2; exit 1 ;; esac
root=$(cd "$(dirname "$0")/.." && pwd)
sed -i '' -e "s/^version = \".*\"/version = \"$v\"/" "$root/almide.toml" 2>/dev/null \
  || sed -i -e "s/^version = \".*\"/version = \"$v\"/" "$root/almide.toml"
sed -i '' -e "s/^let VERSION = \".*\"/let VERSION = \"$v\"/" "$root/src/terminal.almd" 2>/dev/null \
  || sed -i -e "s/^let VERSION = \".*\"/let VERSION = \"$v\"/" "$root/src/terminal.almd"
grep -n '^version = ' "$root/almide.toml"
grep -n '^let VERSION = ' "$root/src/terminal.almd"
