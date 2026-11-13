#!/usr/bin/env bash
# Build lgterm in release and bundle it as target/release/bundle/lgterm.app.
set -euo pipefail
cd "$(dirname "$0")/.."

minor=$(rustc --version | sed -E 's/^rustc 1\.([0-9]+).*/\1/')
if [ "$minor" -lt 99 ]; then
  echo "lgterm needs Rust 1.99 or newer (GPUI uses newly stable APIs). Run: rustup update" >&2
  exit 1
fi
command -v zig >/dev/null || { echo "lgterm needs Zig 0.17 to build libghostty-vt" >&2; exit 1; }

cargo build --release --locked -p lgterm

app=target/release/bundle/lgterm.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/lgterm "$app/Contents/MacOS/lgterm"
cp crates/lgterm/icons/lgterm.icns "$app/Contents/Resources/lgterm.icns"
version=$(sed -nE 's/^version = "(.*)"/\1/p' crates/lgterm/Cargo.toml | head -1)
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
  <key>CFBundleIconFile</key><string>lgterm</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
# A stable signer keeps macOS from asking for folder access again after every build.
identity=-
if command -v security >/dev/null && scripts/lgterm-cert.sh; then
  identity="lgterm local signing"
fi
if ! codesign --force --sign "$identity" "$app" >/dev/null 2>&1; then
  echo "Couldn't sign lgterm.app as \"$identity\"; using a throwaway signature." >&2
  codesign --force --sign - "$app" >/dev/null 2>&1 || true
fi
