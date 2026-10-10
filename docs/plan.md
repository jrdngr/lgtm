# Milestone plan

Assumes the Tauri v2 stack in `stack.md`. Each milestone ends with something Jordan can use on a real repo.

## M0: Skeleton and risk checks

- Tauri app, one window per repo, single-instance lock per repo.
- Typed IPC between Rust and the UI.
- Dock layout with placeholder tabs, saved per repo.
- One terminal tab (xterm.js + `portable-pty`).
- Spikes for the risks in `stack.md`: large-diff rendering, PTY throughput, keybinding capture.
- macOS build only.

## M1: Review core

- `Jj` adapter: reads with `--ignore-working-copy`, snapshot worker, one writer per workspace.
- Review target from a revset, Combined view, gap handling.
- Diff view: unified and split, tree-sitter highlighting and function ranges.
- Selection model (all units, mouse and keyboard) and the mark menu.
- Marks, anchoring, matching, and the SQLite store.
- Undo/redo for marks.
- Validate the window size and threshold against a real repo's history.

Added after M1: a flat Files panel with reviewed/changed counts (click to jump), Open Recent on the start window, and a Justfile.

## M2: Review workflow

- File checkboxes, "changed since reviewed", amber rows.
- Progress counts, line filter, gap bars, file tree with progress (grow the Files panel into it).
- `skip` globs.
- Per-change and Since-last-session views, with evolog mapping and `jj interdiff`.
- Live updates from `op_heads`, stale working copy notice.
- Debounced snapshot worker, driven by the working-copy watcher.
- Selections follow the code when the diff changes.
- Config file: `skip` globs, anchoring window and threshold.

Notes from building M2:
- The config lives at `.lgtm/config.toml`.
- Since last session, after a split, diffs the old commit against the last successor's tree. That includes base changes if the stack was rebased too.
- An interdiff's old side isn't a commit, so its removed rows can't be marked.

## M3: Tasks and terminals

- Tasks as jj workspaces, `.lgtm/setup`, `lgtm gc`.
- Task sidebar with review progress.
- Terminal splits, any number of panes.
- Send a selection or comment to a terminal pane's prompt.

## M4: Comments and GitHub

- Local comments with anchoring.
- PR checkout (same repo, fork, fallback ref).
- Pull PR comments, reply locally, publish.
- Workshop a reply with an agent inside the comment block.

## M5: Persona reviews

- Dispatch wee-bots personas against a review, headless via `claude -p` or into a terminal running `claude`.
- Per-persona reports, synthesized report, cross-review, minimal report.
- Findings link to code and can become comments.

## M6: Navigation

- Graph views of the change.
- Agent highlighting of the riskiest parts.
- MCP server exposing review state and actions to agents in terminals.

## M7: Editor

- File tree and editor tab with syntax highlighting and save.

M7 can move earlier if editing during review is needed sooner. It has no dependency past M1.
