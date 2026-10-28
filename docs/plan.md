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

Added after M1: a flat Files panel with reviewed/changed counts (click to jump), Open Recent on the start window, and a justfile.

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
- New workspaces go in `$XDG_DATA_HOME/lgtm/ws/<repo>-<hash>/<workspace>`, falling back to `~/.local/share`. `<hash>` comes from the repo's path, so same-named repos don't collide. Names are limited to letters, digits, `.`, `_` and `-`.
- `.lgtm/setup` runs in the new workspace's first terminal, so its output is visible, then the shell takes over. Terminals get `LGTM_ROOT` (the main workspace) and `LGTM_WORKSPACE`.
- Each workspace mounts its own dock and saves its own layout. Docks of unselected workspaces stay mounted but hidden, so their terminals keep running.
- Deleting a workspace forgets it and deletes its directory only when lgtm created it. Its commits stay.
- `lgtm gc [--force] [repo]` removes workspace directories whose workspace was forgotten, and forgets workspaces whose directory is gone. A workspace whose repo can't be found (moved, or on an unmounted volume) is kept unless `--force`.
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
- Personas are markdown files: `description` and `off_lens` front matter, and the profile as the body. The file name is the persona's name. They're in `$XDG_CONFIG_HOME/lgtm/personas` (falling back to `~/.config`), never in the repo, so a change under review can't rewrite its own reviewers. Files that can't be read are listed with why.
- Nothing is built in. The Reviewers view offers to install the seven defaults (the wee-bots personas), all picked, except ones whose file was edited, which say so and start unpicked. Installing again restores the originals.
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

Notes from building M6:
- The graph shows the review's changed symbols. A symbol is changed when a changed row sits directly in it, not in a nested symbol. A symbol changed on both sides is one node, matched by name, kind and enclosing symbol; one only on the old side is removed. Without type information, symbols link by name. A function needs a call (`name(`, `name!(`, `name::<`), a type any mention, and modules are never linked, since their names are often variables' too. Same-file symbols win; otherwise a name shared by more than 2 changed symbols links to none.
- Callers are unchanged symbols that use a changed symbol's name, found in the review's files and in the rest of the workspace's working copy (gitignored files skipped, at most 20,000 files of 512 KB or less). Names under 3 characters don't count, and each symbol shows at most 8 callers. Callers outside the review show but can't be jumped to until M7.
- The graph is a dock pane (`g`, or Graph in the review bar) laid out with ELK in a web worker, in boxes per file, callers to the left. M6's own `@` (276 changed symbols, 550 links) took 4 s to lay out on the main thread, which froze the window. Callers start hidden past 60, with a checkbox to show them. Links are drawn as one SVG path per style rather than React Flow edges: in the app's WebKit, an element per link cost time that grew faster than the link count (250 links blocked the window for 2 s, 554 for 5 s). Hovering a symbol highlights its links. Nodes show review progress and are outlined by their worst hotspot. The Symbols view (⇧⌘Y) lists the changed symbols with progress and caller counts.
- Highlight risks (`h`) runs `claude -p --json-schema` read-only on the review's diff and returns up to 15 ranges, each with a level (high, medium, low) and a reason. They're anchored like comments and stored per review label, and each pass replaces the last. They show as a gutter strip, in the Hotspots view (⇧⌘H), and on graph nodes. `]` and `[` jump between them.
- The MCP server runs in the app on a Unix socket in the app's data directory. Terminals get `LGTM_SOCKET`. `lgtm mcp`, registered with `claude mcp add -s user`, pipes stdio to the socket after a first line naming the terminal's repo and workspace. Outside an lgtm terminal, or with the app closed, it's a server with no tools, so other `claude` sessions don't show a failed server.
- The app offers to register it in a bar under the toolbar while `~/.claude.json` has no `lgtm` server, or one whose binary is gone. A registration pointing at another existing lgtm binary (a dev build) is left alone.
- The revision selector moved from the review bar to the center of the header, styled as a field rather than a button, so another revset can be picked even when the review is empty or didn't load. It acts on the workspace's active review pane, and opens one when there's none. `r` in a review still opens it.
- MCP tools act on the review in the workspace's last active review pane: get_review, list_hotspots, set_hotspots, get_symbol_graph, list_reports, get_report, publish_findings, show, set_review_revset, set_log_revset. Agents can't mark code. Published findings become a reviewer's section of a report, new or named by its id (`R<n>` or the name the user gave it).

## M7: Editor

- File tree and editor tab with syntax highlighting and save.

Notes from building M7:
- The Repo view (⇧⌘R) sits next to Files in the right side panel. It lists the workspace's working copy a directory at a time, without what `.gitignore` files ignore, and colors files the current review changes with VS Code's dark git colors: green for added (and copied), lighter green for renamed, yellow for modified, red for conflicts, with the status letter. A folder takes its most urgent file's color with a dot: conflict, then added, renamed, modified. VS Code has no fixed order here (a folder takes whichever child it finds first). Removed files don't color folders, as in VS Code. It lists open directories again when the repo changes.
- Files open in editor tabs, all in one pane, opened right of the active pane the first time. Opening a file that's open focuses its tab and moves the cursor.
- The editor is CodeMirror 6, colored by the backend's tree-sitter highlighting, the same `hl-*` classes as the diff view. Edits keep the old colors in place until the next pass lands, about 120 ms after typing stops. Files over 4 MB or not UTF-8 don't open. CRLF files keep their line endings.
- Markdown highlights with `tree-sitter-md`, in the editor and the diff view. Inline text is injected into the block grammar (it needs `include-children`, or the inline grammar highlights nothing), and fenced code is highlighted by its info string's language.
- Markdown files have a Rendered/Editor toggle in the editor's path bar (⇧⌘V). The rendered view is read-only, shows unsaved edits, and follows reloads from disk. It renders GitHub's extensions (tables, task lists), colors fenced code with the same tree-sitter highlighting, shows links as text (following one would navigate the window away), and doesn't render raw HTML. Opening the file at a line switches back to the editor.
- ⌘S saves, then snapshots the workspace's working copy, so reviews of `@` update right away. Only files that exist can be saved.
- When the file changes on disk, an editor with no unsaved edits loads it in place, keeping the cursor. With unsaved edits a bar offers Load theirs or Keep mine. A dirty tab's title starts with ●. Closing a tab drops unsaved edits.
- Edit and New in the Reviewers view open persona files in the editor: the editor opens absolute paths only for persona files. Saving one rereads the personas.
- Opening at a line: `o` in a review opens the cursor's file at its new-side line. Graph and Symbols callers outside the review, and finding locations not in the open review, open in the editor. The MCP `show` tool opens files the review doesn't change in the editor, and so does any `show` while the workspace has no review pane.

## M8: GitHub

Remote features come last, after everything local.

- PR checkout (same repo, fork, fallback ref).
- Pull PR comments, reply locally (workshopping with an agent as in M4), publish.
- jj conflict resolution: one click hands a conflicted commit to an agent to resolve.

Notes from building M8:
- GitHub goes through the `gh` CLI (`gh api graphql`), not octocrab: `gh` already has the user's auth and hosts, so the app never holds a token. It's an optional tool in the startup check (2.40.0 or newer); without it the PR button and Publish are off.
- PR in the Workspaces view takes a number, `#N`, or a PR URL (anchors and queries are ignored), and checks the PR out into workspace `pr-<N>` on its head, or switches to it if it's there. A number means the repo behind the `upstream` remote, then `origin`, then the first GitHub remote that isn't a `gh-*` fork remote; a URL picks the remote for its repo. The base branch and same-repo heads come from that remote, fork heads from a `gh-<owner>` remote added on demand (SSH or HTTPS like the base remote), and when the head branch is gone, from `pull/N/head` into an `lgtm/pr-N` bookmark (git fetch into jj's git store, then `jj git import`, so it works without colocation). The PR is stored per workspace.
- A PR workspace reviews `"<base>"@"<remote>"..@`, which matches GitHub's diff and includes agent edits in `@`. Its review pane opens on that instead of `@`, and the sidebar shows the PR's number and title.
- Opening a review in a PR workspace pulls its threads and checks the head on GitHub. When it moved, a note under the review bar says how (new commits, or force-pushed) with Refetch. Refetch fetches from the stored base remote, and when the head moved, ends the review's session and rebases the workspace's own commits (the ones above the old head that the new head doesn't already include, as when they were pushed) onto the new head; a workspace editing the old head gets a new commit on the new one. Then the whole PR opens again, so "Since last session" shows the push.
- Sessions match a change with no evolog link to the last session (a fetched replacement) by change id, which survives a force-push when the author uses jj. Other force-pushed commits show as removed and new.
- Inline threads become comment blocks labeled GitHub thread, with their posts above and a reply below. Threads are pulled, and comments published, only from the whole PR in the Combined view, where GitHub's lines match the review's rows. Threads on a head newer than the fetched one wait unplaced until Refetch, as do threads whose line `@` changed. A range from a removed line to an added one keeps only its end. Pulling again updates known threads by thread id and keeps the reply, workshop and anchors, and places threads that had no rows when their code shows up. A thread anchors on the review's changed rows at its line (head lines remapped to the review's new side through line hashes, since `@` can differ from the head), or the nearest changed row for a context line. Outdated threads have no rows and keep GitHub's diff hunk as their snippet. The workshop prompt includes the thread.
- Publish (in the block) adds the block's text to the viewer's pending review: a reply on a thread, or a new thread on the comment's rows mapped back to head lines. A line the head doesn't have can't be published, nor can anything while the pending review is on an older commit. The reply then clears (unless it was edited meanwhile) and the threads are pulled again. The Comments view shows the pending count, a verdict (Comment, Approve, Request changes), a summary and Submit; with no pending review, Submit posts the verdict alone.
- The PR view (⇧⌘P) shows the PR, its conversation comments and review summaries (bare "commented" reviews are left out, since their comments show on the code), Refetch, and a box that posts to the conversation right away.
- Conflicted changes in a review get a note with Resolve with agent and Send to terminal. Resolve runs `jj new` on the change, has `claude -p` with edit tools fix the markers (the prompt explains jj's diff-style sides), snapshots, squashes into the change, and puts `@` back. `jj new` abandons an empty `@`, so then a new one goes where it was. A failed or stopped run abandons the attempt, and `@` goes back however the run ends. Resolve, Refetch, finding fixes and comment Change each claim the workspace, so only one moves `@` or edits the working copy at a time.
- Deleting a PR workspace deletes its `lgtm/pr-N` bookmark and the fork remote lgtm added for it, once no other PR workspace uses that; a `gh-*` remote that was already there stays. `lgtm gc` deletes `lgtm/pr-N` bookmarks without a `pr-N` workspace.
