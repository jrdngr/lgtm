# lgtm

Code review on jj, in the terminal. Rust crates in `crates/`, one Cargo workspace at the root: `crates/lgtm` is the `lgtm` TUI and CLI (`lgtm mcp`, `lgtm hook`, `lgtm issues`), `crates/lgterm` the terminal multiplexer, and `lgtm-core`, `lgtm-tui-kit`, `lgtm-issues` and `lgterm-proto` what they share. The README covers features and shortcuts.

## Workflow

- Sessions run in the cloud (Linux). `scripts/cloud-setup.sh` installs the toolchain if it's missing.
- Before every push, run `just test` (rustfmt, clippy, cargo test) and make sure it passes. Fix every warning; `cargo fmt` fixes formatting.
- Push straight to `main`. No PRs. One commit per task with a clear message.
- Cloud sessions can't run lgterm. Jordan builds `main` on a Mac with `just go` and tests it there. Say what to check by hand when a change touches the UI.
- v1 is macOS only. Don't add Linux-specific code paths. Keep the Linux build working, since tests run there.

## Conventions

- Keep files small and code simple and modular. Split a module before it grows past a few hundred lines.
- Match the surrounding code's comment density and naming.
- All jj calls go through `crates/lgtm-core/src/jj/`.
- Update the README when a feature or shortcut changes.
- No explanatory text in the UI: no hints, subtitles or tooltips saying what something does or why. Labels name things; that's all.
