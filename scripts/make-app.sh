#!/bin/sh
# Build Sprid.app: the release binary, its icon and an Info.plist, signed
# ad hoc so macOS runs it as an app of its own.
#
#   scripts/make-app.sh               build dist/Sprid.app
#   scripts/make-app.sh --install     ... and copy it to ~/Applications
#   scripts/make-app.sh --zip         ... and pack it for a release,
#                                     dist/Sprid-<version>-<arch>.zip
#
# ALMIDE names the compiler (default: almide on PATH).
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
almide=${ALMIDE:-almide}
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/almide.toml" | head -1)
app="$root/dist/Sprid.app"

"$almide" build --release "$root/src/main.almd" -o "$root/sprid"

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$root/sprid" "$app/Contents/MacOS/sprid"
cp "$root/assets/Sprid.icns" "$app/Contents/Resources/Sprid.icns"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Sprid</string>
  <key>CFBundleDisplayName</key><string>Sprid</string>
  <key>CFBundleIdentifier</key><string>io.github.o6lvl4.sprid</string>
  <key>CFBundleExecutable</key><string>sprid</string>
  <key>CFBundleIconFile</key><string>Sprid</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
codesign --force --sign - "$app" >/dev/null 2>&1 || echo "make-app: codesign failed; the app is unsigned" >&2
echo "built $app"

if [ "${1:-}" = "--zip" ]; then
  # ditto keeps the bundle as Finder would: its signature and permissions.
  zip="$root/dist/Sprid-$version-$(uname -m).zip"
  rm -f "$zip"
  ditto -c -k --keepParent "$app" "$zip"
  echo "packed $zip"
fi

if [ "${1:-}" = "--install" ]; then
  dest="$HOME/Applications/Sprid.app"
  mkdir -p "$HOME/Applications"
  rm -rf "$dest"
  cp -R "$app" "$dest"
  echo "installed $dest"
fi
