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

Notes from building M4.5:
- Squash keeps the parent's description (`--use-destination-message`), since a headless jj can't open an editor to combine them.
- Squashing `@` then runs `jj edit` on the parent, so `@` lands on it rather than on a new empty commit. That's two jj operations, so the panel's undo and redo take back both.
- The panel builds revsets from change ids at jj's shortest unique length, at least 8.
- The chip colors ids as jj does, with the unique prefix in pink. One commit shows up to 5 characters of its id, fewer when the bar is narrow, but never less than the unique prefix. A range shows only the unique prefixes, like `wk::q`.
- After squash, undo or redo the review keeps its revset, and falls back to `@` when that no longer resolves.
- Review panes always open on `@`. The last revset isn't saved with the layout, since it may not exist next time.

## M5: Persona reviews

- Dispatch wee-bots personas against a review, headless via `claude -p` or into a terminal running `claude`.
- Per-persona reports, synthesized report, cross-review, minimal report.
- Findings link to code and can become comments.

Notes from building M5:
- Personas are markdown files: `description` and `off_lens` front matter, and the profile as the body. The file name is the persona's name. Global ones are in `$XDG_CONFIG_HOME/lgtm/personas` (falling back to `~/.config`), a repo's in `.lgtm/personas`, and a repo persona hides a global one of the same name. Files that can't be read are listed with why.
- Nothing is built in. The Reviewers view offers to install the six defaults (the wee-bots personas) globally or into the repo, all picked, except ones whose file was edited, which say so and start unpicked. Installing again restores the originals. A repo install adds files to `@`.
- The view rereads persona files each time it's shown. Edit opens a persona's file in the default editor, and New writes a starter file and opens it.
- A run copies its personas when it starts, so retries and later steps use them as they were.
- The rubric, the prompts and the steps belong to the app. Every step runs `claude -p --json-schema` with its own schema, and the result is stored as structured data: a persona's findings, challenges, rebuttals, the synthesis, and the minimal pick. A reply that doesn't match fails the part.
- The app numbers a persona's findings `<persona>#1…` by severity. Later steps refer to findings by id. The synthesis numbers its merged findings and lists the persona findings each absorbs, and the minimal pick lists ids.
- Each persona is its own run with Read, Grep and Glob, in the review's workspace. The prompt carries the diff (cut at 200 KB), the stat and the commit descriptions. The diff in the prompt is frozen at the commits the revset resolved to when the run started, but agents read files from the working copy, which the prompt says. The report bar warns when the open review's commit isn't the run's.
- Steps run in order: reviews, then for cross-review a challenge pass (fresh runs, each persona on the others' medium and higher findings, skipped when there are none) and rebuttals (each challenged persona's own session, resumed), then synthesis, then the minimal pick. Cross-review needs two personas, and turns synthesis on, since the synthesis rules on disputes.
- A step whose parts didn't all finish halts the run. Retry reruns only the parts that didn't finish. A finished run can add synthesis, cross-review or a minimal pick; adding cross-review redoes synthesis and minimal.
- While a part runs, its chip shows the agent's latest tool call, like "Read src/a.rs".
- Runs are stored per workspace in the repo's SQLite store, and listed in the Reviewers side view (⇧⌘O). The UI calls the feature Reviewers; the code calls it bots. Runs an older build stored in another shape are skipped.
- The report pane opens to the right of the review. It shows findings as cards, with Minimal, Synthesis and By persona views. A card opens to show facts, explanation, fix, any contested note, and the challenges, rebuttals and rulings behind it. Fix first, quick wins and questions follow the synthesis, linking to cards.
- Persona chips, with each persona's state and latest tool call, show only in the By persona view, where clicking one filters to it. The bar shows the run's state in every view.
- Each card has a checkbox, and Pick all picks every card shown. Picked findings, from any view, can go together to a terminal's agent, or to a headless agent (`claude -p` with Read, Grep, Glob, Edit and Write in `acceptEdits` mode, as comment Change does). That edits the working copy, so it needs the run's revset to include `@`. Its reply streams into a Fixes view, which lists each fix with links to its findings and can stop it. The working copy is snapshotted when it's done.
- Sending to a terminal says in the report pane where the text went, or why it couldn't go.
- A card's locations jump to the code in the open review. Comment anchors the finding on the changed rows in its first location's range, or the nearest changed row when the range has none. Send to terminal pastes the finding into a terminal's prompt.
- Reviews only run headless, since a terminal agent can't hand structured findings back. M6's MCP server is the way for one to do that.
- At startup the app checks `jj` (0.40.0 or newer, for `WorkspaceRef.root()`) and `claude` (2.1.22 or newer, for `-p --json-schema`). A missing or old tool gets a dialog with the command that fixes it. A `jj` problem blocks the app. A `claude` problem can be skipped, which turns off persona reviews and comment workshopping.

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
