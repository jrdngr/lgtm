#!/usr/bin/env bash
# What lgterm's build needs in Claude Code cloud sessions (Linux): GPUI's libraries, Zig 0.17
# for libghostty-vt, and Ghostty's Zig packages. `zig fetch` can't get through the cloud's
# proxy, so the packages are downloaded with curl and git and placed in Zig's cache.
set -euo pipefail

ZIG_VERSION=0.17.1
SUDO=$(command -v sudo >/dev/null && echo sudo || true)

$SUDO apt-get install -yq --no-install-recommends \
  clang libclang-dev libxcb1-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev \
  libvulkan-dev mesa-vulkan-drivers libfontconfig-dev libfreetype-dev xvfb xdotool imagemagick

if [ "$(zig version 2>/dev/null)" != "$ZIG_VERSION" ]; then
  $SUDO pip install -q --break-system-packages --target /opt/ziglang "ziglang==$ZIG_VERSION"
  $SUDO ln -sf /opt/ziglang/ziglang/zig /usr/local/bin/zig
fi

CACHE="${ZIG_GLOBAL_CACHE_DIR:-$HOME/.cache/zig}/p"
mkdir -p "$CACHE"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# Repack the one folder in "$WORK/src" as Zig's cache entry for package hash $1.
store() {
  local root
  root=$(find "$WORK/src" -mindepth 1 -maxdepth 1 -type d | head -1)
  mv "$root" "$WORK/$1"
  tar -czf "$CACHE/$1.tar.gz" -C "$WORK" "$1"
  rm -rf "$WORK/src" "${WORK:?}/$1"
}

tarball() {
  [ -f "$CACHE/$1.tar.gz" ] && return
  mkdir -p "$WORK/src"
  curl -sSfL "$2" | tar -xz -C "$WORK/src"
  store "$1"
}

git_commit() {
  [ -f "$CACHE/$1.tar.gz" ] && return
  mkdir -p "$WORK/src/repo"
  git -C "$WORK/src/repo" init -q
  git -C "$WORK/src/repo" fetch -q --depth 1 "$2" "$3"
  git -C "$WORK/src/repo" checkout -q FETCH_HEAD
  rm -rf "$WORK/src/repo/.git"
  store "$1"
}

# The packages libghostty-vt needs at the commit ghostty-vt-sys pins.
tarball uucode-0.2.0-ZZjBPoGFVgCiflJNFc6Io8xzC67B6PA5bUkk4LT0kCxg \
  https://deps.files.ghostty.org/uucode-ea62149739404a73c202b48a33bf6dd2af4bd9b0.tar.gz
tarball translate_c-2.0.0-Q_BUWltOBwBYABF84EUL208pIFeUIwbBs29FmVBaafN5 \
  https://deps.files.ghostty.org/translate_c-02ff0c523fb92a38939b04d0145c4a9602a74d56.tar.gz
git_commit aro-0.0.0-JSD1QtuBNwCASyBtNF3pqTl_W3oAJQGEVyFAtrBSE_Pa \
  https://github.com/Vexu/arocc d0c8c4d9c55daa7ef6e40cf0f630a5b5e900989b
# Lazy packages zig asks for when building from a Ghostty checkout (GHOSTTY_SOURCE_DIR).
tarball N-V-__8AAB0eQwD-0MdOEBmz7intriBReIsIDNlukNVoNu6o \
  https://deps.files.ghostty.org/zlib-1220fed0c74e1019b3ee29edae2051788b080cd96e90d56836eea857b0b966742efb.tar.gz
tarball N-V-__8AAGmZhABbsPJLfbqrh6JTHsXhY6qCaLAQyx25e0XE \
  https://deps.files.ghostty.org/highway-66486a10623fa0d72fe91260f96c892e41aceb06.tar.gz
tarball N-V-__8AAP5JWgCGP_AD0teWpa4krRvE9VPZzvviGdbmN4jI \
  https://deps.files.ghostty.org/wuffs-7411f488fe2e2c205c3d3b3d28638b7356522930.tar.gz
tarball N-V-__8AADYiAAB_80AWnH1AxXC0tql9thT-R-DYO1gBqTLc \
  https://deps.files.ghostty.org/pixels-12207ff340169c7d40c570b4b6a97db614fe47e0d83b5801a932dcd44917424c8806.tar.gz
