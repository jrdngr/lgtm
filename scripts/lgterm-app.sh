#!/usr/bin/env bash
# Build lgterm in release and install it as /Applications/lgterm.app.
set -euo pipefail
cd "$(dirname "$0")/../lgterm"

minor=$(rustc --version | sed -E 's/^rustc 1\.([0-9]+).*/\1/')
if [ "$minor" -lt 99 ]; then
  echo "lgterm needs Rust 1.99 or newer (GPUI uses newly stable APIs). Run: rustup update" >&2
  exit 1
fi
command -v zig >/dev/null || { echo "lgterm needs Zig 0.17 to build libghostty-vt" >&2; exit 1; }

cargo build --release --locked

app=target/release/bundle/lgterm.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp target/release/lgterm "$app/Contents/MacOS/lgterm"
version=$(sed -nE 's/^version = "(.*)"/\1/p' Cargo.toml | head -1)
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>lgterm</string>
  <key>CFBundleDisplayName</key><string>lgterm</string>
  <key>CFBundleIdentifier</key><string>dev.lgtm.lgterm</string>
  <key>CFBundleExecutable</key><string>lgterm</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
codesign --force --sign - "$app" >/dev/null 2>&1 || true

rm -rf /Applications/lgterm.app
ditto "$app" /Applications/lgterm.app
echo "Installed /Applications/lgterm.app"
