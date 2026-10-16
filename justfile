# Run the app with hot reload
dev:
    pnpm tauri dev

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
    pnpm tauri build --bundles app

# Build the app and put it in /Applications, replacing any copy there
install: build
    rm -rf /Applications/lgtm.app
    ditto src-tauri/target/release/bundle/macos/lgtm.app /Applications/lgtm.app

# Run the app bundle from `just build`, optionally on a repo. Launching the
# binary inside the .app gives it the bundle's Dock icon.
run *repo:
    src-tauri/target/release/bundle/macos/lgtm.app/Contents/MacOS/lgtm {{ repo }}

# Clean up workspaces left behind (see `lgtm gc`)
gc *repo:
    src-tauri/target/release/lgtm gc {{ repo }}

# Set the app version (Cargo.toml, package.json, Cargo.lock) before a release
bump version:
    sed -i.bak 's/^version = ".*"/version = "{{ version }}"/' src-tauri/Cargo.toml && rm src-tauri/Cargo.toml.bak
    sed -i.bak 's/^  "version": ".*"/  "version": "{{ version }}"/' package.json && rm package.json.bak
    cd src-tauri && cargo metadata --format-version 1 > /dev/null
