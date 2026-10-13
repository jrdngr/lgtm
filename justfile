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
    pnpm check
    pnpm test

# Build the release binary and macOS app
build:
    pnpm tauri build --bundles app

# Run the release binary from `just build`, optionally on a repo
run *repo:
    src-tauri/target/release/lgtm {{ repo }}

# Clean up workspaces left behind (see `lgtm gc`)
gc *repo:
    src-tauri/target/release/lgtm gc {{ repo }}
