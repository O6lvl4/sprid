#!/bin/sh
# Build sprid on Linux, run its tests, and draw a screen on a virtual X
# display with the CPU's Vulkan (lavapipe), in the image scripts/linux/
# Dockerfile describes. From the repository's root:
#
#   docker build -t sprid-linux scripts/linux
#   scripts/linux/check.sh              # writes out/linux/sprid, out/linux/screen.png
#
# Inside the container (`--inside`) it does the work on a copy of the
# source, so the host's build directories stay the host's.
set -eu

if [ "${1:-}" != "--inside" ]; then
  root=$(cd "$(dirname "$0")/../.." && pwd)
  mkdir -p "$root/out/linux"
  exec docker run --rm -v "$root:/host:ro" -v "$root/out/linux:/out" -v sprid-almide-cache:/root/.almide \
    sprid-linux sh /host/scripts/linux/check.sh --inside
fi

mkdir -p /src
(cd /host && tar --exclude=./target --exclude=./.almide --exclude=./dist --exclude=./out --exclude=./sprid -cf - .) | tar -xf - -C /src
cd /src
almide build --release src/main.almd -o /out/sprid
for f in src/terminal_test.almd src/resize_test.almd src/updates_test.almd src/view.almd; do
  echo "$f: $(almide test "$f" 2>&1 | grep -E 'passed|failed' | tail -1)"
done
Xvfb :99 -screen 0 1920x1200x24 >/dev/null 2>&1 &
sleep 1
DISPLAY=:99 SPRID_CAPTURE_MS=4000 \
  SPRID_CAPTURE_CMD="sh -c 'uname -srm; printf \"日本語 ✔ ⏺ ✻ ─┼─\\n\"; stty size; sleep 5'" \
  /out/sprid capture /out/screen.png 2>/dev/null
