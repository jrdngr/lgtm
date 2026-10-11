#!/usr/bin/env bash
# Setup for Claude Code cloud sessions (Linux): what `just test` needs.
# Paste into the cloud environment's setup script, or run it from the repo root.
set -euo pipefail

JJ_VERSION=0.46.0
SUDO=$(command -v sudo >/dev/null && echo sudo || true)

# Tauri's Linux build libraries.
$SUDO apt-get update -q
$SUDO apt-get install -yq --no-install-recommends \
  build-essential curl file pkg-config libssl-dev libxdo-dev \
  libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev

command -v cargo >/dev/null || {
  curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
  . "$HOME/.cargo/env"
}

# The Rust tests run jj.
if ! jj --version 2>/dev/null | grep -q "$JJ_VERSION"; then
  curl -sSfL "https://github.com/jj-vcs/jj/releases/download/v$JJ_VERSION/jj-v$JJ_VERSION-$(uname -m)-unknown-linux-musl.tar.gz" |
    $SUDO tar -xz -C /usr/local/bin ./jj
fi
jj config set --user user.name "Claude" 2>/dev/null || true
jj config set --user user.email "noreply@anthropic.com" 2>/dev/null || true

command -v just >/dev/null || $SUDO apt-get install -yq just ||
  curl -sSf https://just.systems/install.sh | $SUDO bash -s -- --to /usr/local/bin

node --version 2>/dev/null | grep -q '^v2[2-9]' || {
  curl -fsSL https://deb.nodesource.com/setup_22.x | $SUDO bash -
  $SUDO apt-get install -yq nodejs
}
command -v pnpm >/dev/null || $SUDO corepack enable

pnpm install --frozen-lockfile
