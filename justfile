# Log levels for the app, as RUST_LOG takes them. `lgtm_lib` and `lgtm_core` are the backend and
# `ui` the frontend: `just log=debug dev`, `just log=lgtm_lib::watch=trace,info run`
log := "lgtm_lib=info,lgtm_core=info,ui=info,warn"

# Hourly log files, the last 24 kept, at most 50 MB each
export LGTM_LOG_DIR := justfile_directory() / "logs"

# Run the app with hot reload
dev:
    pnpm install --frozen-lockfile
    RUST_LOG={{ log }} pnpm tauri dev

# Fetch, start a new change on main@origin, and run the app
app: fresh dev

# Fetch, start a new change on main@origin, and install the `lgtm` command
tui: fresh build-tui install-tui

# Fetch, start a new change on main@origin, and install the app and the `lgtm` command
go: fresh install

[private]
fresh:
    jj git fetch
    jj new main@origin

# Check formatting, typecheck, lint, and run all tests
test:
    scripts/case-check.sh
    pnpm check
    cd src-tauri && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
    pnpm test

# Build the macOS app and the `lgtm` command in one cargo build, so they share every
# dependency, then bundle the app. `tauri build` would compile the app on its own: the frontend
# goes into the binary, and custom-protocol is the feature it adds for release builds.
build:
    pnpm install --frozen-lockfile
    pnpm build
    cd src-tauri && cargo build --release --locked -p lgtm-app -p lgtm --features tauri/custom-protocol
    pnpm tauri bundle --bundles app

[private]
build-tui:
    cd src-tauri && cargo build --release --locked -p lgtm

# Put the `lgtm` command from `just build` in ~/.local/bin
[private]
install-tui:
    mkdir -p ~/.local/bin
    install -m 755 src-tauri/target/release/lgtm ~/.local/bin/lgtm

# Build the app and the `lgtm` command, put the app in /Applications, replacing any copy
# there, and install the command
install: build install-tui
    rm -rf /Applications/lgtm.app
    ditto src-tauri/target/release/bundle/macos/lgtm.app /Applications/lgtm.app

# Run the app bundle from `just build`, optionally on a repo. Launching the
# binary inside the .app gives it the bundle's Dock icon.
run *repo:
    RUST_LOG={{ log }} src-tauri/target/release/bundle/macos/lgtm.app/Contents/MacOS/lgtm-app {{ repo }}

# Clean up workspaces left behind (see `lgtm gc`)
gc *repo:
    RUST_LOG={{ log }} src-tauri/target/release/lgtm-app gc {{ repo }}

