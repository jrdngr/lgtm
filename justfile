# Fetch, start a new change on main@origin, and install the `lgtm` command and lgterm
go: fresh install-tui install-lgterm

[private]
fresh:
    jj git fetch
    jj new main@origin

# Check formatting, lint, and run all tests
test:
    scripts/case-check.sh
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

# Build the `lgtm` command
build-tui:
    cargo build --release --locked -p lgtm

# Build lgterm, the terminal multiplexer, and bundle lgterm.app. Needs Zig 0.17 and Rust 1.99
# or newer.
build-lgterm:
    scripts/lgterm-app.sh

# Build the `lgtm` command and put it in ~/.local/bin
install-tui: build-tui
    mkdir -p ~/.local/bin
    install -m 755 target/release/lgtm ~/.local/bin/lgtm

# Build lgterm, put lgterm.app in /Applications, replacing any copy there, and link the
# `lgterm` command into ~/.local/bin
install-lgterm: build-lgterm
    rm -rf /Applications/lgterm.app
    ditto target/release/bundle/lgterm.app /Applications/lgterm.app
    mkdir -p ~/.local/bin
    ln -sf /Applications/lgterm.app/Contents/MacOS/lgterm ~/.local/bin/lgterm
