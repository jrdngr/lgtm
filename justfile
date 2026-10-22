# Log levels for the app, as RUST_LOG takes them. `lgtm_lib` is the backend and
# `ui` the frontend: `just log=debug dev`, `just log=lgtm_lib::watch=trace,info run`
log := "lgtm_lib=info,ui=info,warn"

# Hourly log files, the last 24 kept, at most 50 MB each
export LGTM_LOG_DIR := justfile_directory() / "logs"

# Run the app with hot reload
dev:
    pnpm install --frozen-lockfile
    RUST_LOG={{ log }} pnpm tauri dev

# Fetch, start a new change on main@origin, and run the app
go:
    jj git fetch
    jj new main@origin
    just dev

# Typecheck, lint, and run all tests
test:
    scripts/case-check.sh
    pnpm check
    pnpm test

# Build the release binary and macOS app
build:
    pnpm install --frozen-lockfile
    pnpm tauri build --bundles app

# Build the app and put it in /Applications, replacing any copy there
install: build
    rm -rf /Applications/lgtm.app
    ditto src-tauri/target/release/bundle/macos/lgtm.app /Applications/lgtm.app

# Run the app bundle from `just build`, optionally on a repo. Launching the
# binary inside the .app gives it the bundle's Dock icon.
run *repo:
    RUST_LOG={{ log }} src-tauri/target/release/bundle/macos/lgtm.app/Contents/MacOS/lgtm {{ repo }}

# Clean up workspaces left behind (see `lgtm gc`)
gc *repo:
    RUST_LOG={{ log }} src-tauri/target/release/lgtm gc {{ repo }}

