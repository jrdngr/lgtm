# Log levels for the app, as RUST_LOG takes them. `lgtm_lib` and `lgtm_core` are the backend and
# `ui` the frontend: `just log=debug dev-app`, `just log=lgtm_lib::watch=trace,info run-app`
log := "lgtm_lib=info,lgtm_core=info,ui=info,warn"

# Hourly log files, the last 24 kept, at most 50 MB each
export LGTM_LOG_DIR := justfile_directory() / "logs"

# Fetch, start a new change on main@origin, and install the `lgtm` command, lgtm-issues and
# lgterm
go: fresh install-tui install-issues install-lgterm

[private]
fresh:
    jj git fetch
    jj new main@origin

# Check formatting, typecheck, lint, and run all tests
test:
    scripts/case-check.sh
    pnpm check
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    pnpm test

# Build and bundle the macOS app. `tauri build` would compile the app on its own: the frontend
# goes into the binary, and custom-protocol is the feature it adds for release builds.
build-app:
    pnpm install --frozen-lockfile
    pnpm build
    cargo build --release --locked -p lgtm-app --features lgtm-app/custom-protocol
    pnpm tauri bundle --bundles app

# Build the `lgtm` command
build-tui:
    cargo build --release --locked -p lgtm

# Build the `lgtm-issues` command
build-issues:
    cargo build --release --locked -p lgtm-issues

# Build lgterm, the terminal multiplexer, and bundle lgterm.app. Needs Zig 0.17 and Rust 1.99
# or newer.
build-lgterm:
    scripts/lgterm-app.sh

# Build the app and put it in /Applications, replacing any copy there
install-app: build-app
    rm -rf /Applications/lgtm.app
    ditto target/release/bundle/macos/lgtm.app /Applications/lgtm.app

# Build the `lgtm` command and put it in ~/.local/bin
install-tui: build-tui
    mkdir -p ~/.local/bin
    install -m 755 target/release/lgtm ~/.local/bin/lgtm

# Build the `lgtm-issues` command and put it in ~/.local/bin
install-issues: build-issues
    mkdir -p ~/.local/bin
    install -m 755 target/release/lgtm-issues ~/.local/bin/lgtm-issues

# Build lgterm, put lgterm.app in /Applications, replacing any copy there, and link the
# `lgterm` command into ~/.local/bin
install-lgterm: build-lgterm
    rm -rf /Applications/lgterm.app
    ditto target/release/bundle/lgterm.app /Applications/lgterm.app
    mkdir -p ~/.local/bin
    ln -sf /Applications/lgterm.app/Contents/MacOS/lgterm ~/.local/bin/lgterm

# Run the app with hot reload
dev-app:
    pnpm install --frozen-lockfile
    RUST_LOG={{ log }} pnpm tauri dev

# Run the app bundle from `just build-app`, optionally on a repo. Launching the binary inside
# the .app gives it the bundle's Dock icon.
run-app *repo:
    RUST_LOG={{ log }} target/release/bundle/macos/lgtm.app/Contents/MacOS/lgtm-app {{ repo }}

# Clean up workspaces left behind (see `lgtm gc`)
gc *repo:
    RUST_LOG={{ log }} target/release/lgtm-app gc {{ repo }}
