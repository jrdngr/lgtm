# lgtm

Desktop code review app on jj: Tauri v2, Rust crates in `crates/` (the app is `crates/lgtm-app`, the TUI `crates/lgtm`, the terminal multiplexer `crates/lgterm`), one Cargo workspace at the root, React UI in `src/`. The spec, stack, and milestone plan are in `docs/`. The README covers features and shortcuts.

## Workflow

- Sessions run in the cloud (Linux). `scripts/cloud-setup.sh` installs the toolchain if it's missing.
- Before every push, run `just test` (tsc, biome lint and format, rustfmt, clippy, vitest, cargo test) and make sure it passes. Fix every warning; `pnpm biome check --write` and `cargo fmt` fix formatting.
- Push straight to `main`. No PRs. One commit per task with a clear message.
- Cloud sessions can't run the app. Jordan builds `main` on a Mac with `just go` and tests it there. Say what to check by hand when a change touches the UI.
- v1 is macOS only. Don't add Linux-specific code paths. Keep the Linux build working, since tests run there.

## Conventions

- Keep files small and code simple and modular. Split a module before it grows past a few hundred lines.
- Match the surrounding code's comment density and naming.
- Rust commands are typed into `src/bindings.ts` by tauri-specta on debug builds. After changing a command, run `UPDATE_BINDINGS=1 cargo test -p lgtm-app` and commit the new bindings.
- All jj calls go through `crates/lgtm-core/src/jj/`.
- New Tauri plugin APIs need a permission in `crates/lgtm-app/capabilities/default.json`.
- Update the README when a feature or shortcut changes.
- No explanatory text in the UI: no hints, subtitles or tooltips saying what something does or why. Labels name things; that's all.
