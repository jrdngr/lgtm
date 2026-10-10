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

## M3: Workspaces and terminals

- Workspaces via `jj workspace add`, `.lgtm/setup`, `lgtm gc`.
- Workspace sidebar with review progress. Each workspace has its own layout.
- Terminal splits, any number of panes.
- Send a selection or comment to a terminal pane's prompt.

Notes from building M3:
- New workspaces go in `$XDG_DATA_HOME/lgtm/ws/<repo>/<workspace>`, falling back to `~/.local/share`. Names are limited to letters, digits, `.`, `_` and `-`.
- `.lgtm/setup` runs in the new workspace's first terminal, so its output is visible, then the shell takes over. Terminals get `LGTM_ROOT` (the main workspace) and `LGTM_WORKSPACE`.
- Each workspace mounts its own dock and saves its own layout. Docks of unselected workspaces stay mounted but hidden, so their terminals keep running.
- Deleting a workspace forgets it and deletes its directory only when lgtm created it. Its commits stay.
- `lgtm gc [repo]` removes workspace directories whose workspace was forgotten or whose repo is gone, and forgets workspaces whose directory is gone.
- Review sessions record their workspace. Marks made outside the main workspace record `<revset> in <workspace>` as their review.
- The sidebar's progress is for the workspace's latest reviewed revset, or `@`.
- Send to agent pastes into the workspace's last focused terminal, and only when its program has bracketed paste on, so a shell never runs the pasted lines. Comments come with M4.
- Panes move by dragging their tabs. ⌘[ and ⌘] move between panes.
- Workspaces sit in a left side panel and Files in a right one, outside the pane grid. Each side panel holds views behind a tab strip; repo files will join the right one in M7. ⌘B and ⌥⌘B show and hide them, ⇧⌘K and ⇧⌘E open and focus Workspaces and Files. Their visibility, widths and tabs are saved per repo, shared by its workspaces.

## M4: Comments

- Local comments with anchoring.
- Workshop a comment with an agent inside the comment block.

Notes from building M4:
- `c`, or `m` then `c`, opens a comment block under the selection's last changed row. ⌘↵ saves and Esc cancels. Commenting clears the selection, as marking does.
- A comment belongs to one file: the last file in the selection with changed rows. It's anchored like a mark on its first and last changed rows, and keeps the code as it was. When the code is gone it stays listed, marked "code changed".
- Comments aren't part of mark undo.
- The box under a comment runs `claude -p` headless in the review's workspace, with no MCP servers. **Ask** (↵) gets only Read, Grep and Glob. **Change** (⌘↵) also gets Edit and Write in `acceptEdits` mode, and edits the working copy in place; with the box empty it does what the comment says. Both share one conversation per comment, and follow-ups resume the same Claude session, so they show up in `claude --resume` there. Replies stream into the block. The agent puts a suggested comment in a code block tagged `comment`, shown with a Use this button that puts it in the editor. Asking about an unsaved comment saves it first.
- `m` then `g` (Change with agent) opens a comment block on the selection with the cursor in the ask box.
- Change edits `@`, so it's turned off, with a note, when the review's revset doesn't include `@`.
- A comment whose code is gone shows under its file's header with the code it was made on, so a Change that rewrites its lines doesn't lose the conversation.
- Send to agent on a comment pastes its code and text into a terminal, as sending a selection does. With two or more terminals in the workspace, a picker asks which, last focused first. Number keys pick directly. Terminals are listed by the title their program sets.
- A Comments view sits next to Files in the right side panel (⇧⌘M). It lists the review's comments, jumps to them, and can send them all to the agent.

## M4.5: jj in the review bar

- The revset box becomes a chip showing what's reviewed, `@ <change> <description>` in jj's colors. `r` or a click opens the revisions panel.
- The revisions panel: a revset box and the `jj log` graph. j/k move, J/K extend, Enter reviews. `e` edits, `n` and `N` make a new commit after and before, `s` squashes into the parent, `u` and `U` undo and redo. The footer shows the last action.
- The review follows `@`: edit X reviews `@`, new after X reviews `X::@`, new before X reviews `@::X`.

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

## M8: GitHub

Remote features come last, after everything local.

- PR checkout (same repo, fork, fallback ref).
- Pull PR comments, reply locally (workshopping with an agent as in M4), publish.
