# lgtm

Desktop code review for agentic development, built on jj. See `docs/` for the spec, stack, and plan.

## Develop

Needs Rust, Node 22, pnpm, [just](https://github.com/casey/just), and jj 0.46 or newer (for `jj util snapshot`).

```sh
pnpm install
just dev         # hot reload; shows the repo picker
just go          # jj git fetch, jj new main@origin, then just dev
just test        # tsc + biome, vitest, cargo test (the Rust tests run jj)
just build       # release binary and .app
just run [repo]  # run the release build
```

Debug builds regenerate `src/bindings.ts` from the Rust commands. `cargo test` fails if it's stale; rerun with `UPDATE_BINDINGS=1` to refresh it.

A built app takes a repo path as its argument: `lgtm /path/to/repo`. A second launch for the same repo focuses its window. Each repo opens in its own window, and a repo is never open in two: picking one that's already open focuses its window. Closing an editor tab or a window, ⌘O, quitting (⌘Q, the Dock, or logging out), or deleting a workspace asks to save first when an editor has unsaved changes.

## Shortcuts

| Keys | Action |
|---|---|
| ⌘N | New window, opening on the repo picker |
| ⌘O | Open a different repo: swaps this window for the repo picker |
| ⌥Z | View > Word Wrap: wrap long lines in reviews and editors instead of scrolling sideways (off by default, remembered) |
| ⌃\` | Bottom panel: show it and focus it (with a new terminal when it's empty), or hide it when it has focus |
| ⌃⇧\` | New terminal in the bottom panel |
| ⌘T | New terminal tab, in the pane grid or bottom panel, whichever has focus |
| ⌘D | Split terminal right |
| ⌘⇧D | Split terminal down |
| ⌘⇧E | Show the Changes tab (reopens the right panel if closed) |
| ⌘⇧Y | Show the Symbols tab |
| ⌘⇧R | Show the Files tab |
| ⌘⇧M | Show the Comments tab |
| ⌘⇧P | Show the Comments tab with the PR open |
| ⌘⇧H | Show the Walkthrough tab |
| ⌘⇧O | Show the Reviewers tab |

In a review pane:

| Keys | Action |
|---|---|
| `j` / `k` | Move the cursor |
| `J` / `K`, or `v` then `j` / `k` | Extend the selection |
| `e` / `E` | Expand / shrink: line, function, hunk, file, review |
| `space` | Mark reviewed (the selection then collapses to the cursor) |
| `m` or right-click | Mark menu: `r` reviewed, `s` skip, `f` flag, `u` clear |
| ⌘A | Select the whole review |
| ⌘Z / ⇧⌘Z | Undo / redo mark changes |
| ⌘C | Copy selected code text, or the selected rows when no text is selected |
| `c` | Comment on the selection |
| `a` | Ask the agent about the comment at the cursor |
| `x` | Check or uncheck the file under the cursor |
| `za` | Collapse or expand the hunk, or the file on its header |
| `g` | Open the symbol graph |
| `h` | Walkthrough: an agent guides you through the change |
| `]` / `[` | Next / previous change (each run of changed rows) |
| `}` / `{` | Next / previous file |
| `n` / `N` | Next / previous note in the walkthrough's tour |
| `Esc` | Back to the cursor row |

The right panel has six icon tabs. Comments counts pending GitHub posts, Walkthrough its notes, and Reviewers its running reviews.

- **Files** (⌘⇧R) shows the whole repo.
- **Changes** (⌘⇧E) shows the review's files as a tree, with the rows done so far in its header. Each file has a ring filling as you review it, or a ✓ once done; hover it for the counts, or hover a folder to see its count. Done files dim, deleted ones are struck through, and a file changed since you checked it says **changed**. It follows the review's line filter. Click a file to jump to it.
- **Symbols** (⌘⇧Y) lists the changed symbols by file, with how many unchanged callers use each and how much of it is reviewed. With no Review tab open, clicking a file or symbol opens one on the current revset.
- **Comments** (⌘⇧M) lists the review's comments by file, with Open, Pending, Resolved and All filters once some are GitHub threads; its ⋯ menu sends them all to a terminal. In a PR workspace the PR sits on top: its state, branches, ↗ to open it on GitHub, and ⟳ Sync, which fetches the PR, pulls its threads, and reloads its conversation. The conversation folds to one line of each reviewer's verdict. The review submit box (Comment, Approve, Request changes) stays at the bottom.
- **Walkthrough** (⌘⇧H) lists the walkthrough's notes.
- **Reviewers** (⌘⇧O) runs review personas.

Comments show under their code as cards: purple for a GitHub thread, blue for your own. Their actions show on hover: ✎ edit, ✦ ask the agent (or `a`), ↗ send to a terminal, and Publish and Delete in ⋯. The agent box stays open while the comment has a conversation with the agent. Resolved threads fold to one line; click to open one.

In the header and review toolbar:

- **Revset:** the selector in the center of the header shows what's reviewed (`@` by default), with or without a Review tab open: one change as its id and description, a range as `oldest → newest` with its newest description and change count. Its border turns amber when the change has no description or the working copy is stale. New Review tabs open on it. Click it or press `r` to open `jj log` as you have it configured. Move with `j` / `k` or a click and extend a range with `J` / `K` or a shift-click, then press ↵ or the Review button; or type a revset (`/`); or click a preset: `@`, `@-`, or `main..@` (named for the bookmark `trunk()` resolves to). The cursor row's Edit, New ▾ and Squash buttons, or `e`, `n` / `N` and `s`, edit the commit, add one after or before it, or squash it into its parent. `u` / `U` undo / redo the last of these.
- **View:** Combined, Per change, or Since last session (what each change does differently from the last time this revset was reviewed). The second picker chooses the change.
- **Line filter:** All lines, Unreviewed, Reviewed, Skipped, Flagged. Hidden rows become thin gap bars; click one to show its rows.
- **Layout:** the two icons pick Unified or Split; the lit one is current. In Split, each side scrolls sideways on its own, with a scroll bar under each. Scrolling sideways over the diff moves both, and the one with room left keeps going after the other hits its end.
- Progress for the whole review: a bar of reviewed (green), skipped (grey), and flagged (red) rows with the reviewed percentage. Only added rows count when a change both removes and adds lines; a change that only removes lines counts its removed rows. Hover it for the counts. **Reset review…** in the ⋯ menu at the end clears every mark in it after you confirm.

Bottom panel: a second pane grid docked under the panes, between the side panels, which pushes the panes up. Its tabs and splits work like the main grid's, and each workspace has its own. Drag tabs between it and the main grid; terminals keep running when they move, but an editor with unsaved changes stays put. It opens with a terminal when it would open empty. Hiding it with ⌃\` keeps its terminals running, and closing its last tab hides it too, while dragging its last tab out leaves it open. Drag its top edge to resize; the height and whether it's open are remembered.

Panes: the **+** tab after each pane's tabs opens a new Terminal or Graph tab in that pane. The split icons at the right of each pane's tab strip split it right or down into an empty pane, which offers the same kinds of tab.

Each file header has a checkbox. Checking marks the file reviewed and collapses it, without touching its line marks. If new rows show up later, the file unchecks itself and the new rows turn amber. Marking a whole-file selection reviewed (`e` up to file, then `space`) checks it too.

Reviews update live: edits in the working copy are snapshotted after 300 ms of quiet, and any new jj operation reloads open reviews. Selections stay on the same code.

Reviewers: the Reviewers tab (⌘⇧O) runs review personas on the open review, each as its own headless `claude -p` that can only read files. Personas are markdown files in `~/.config/lgtm/personas` or the repo's `.lgtm/personas`. Each shows its description, a **repo** pill if it's the repo's, and ✎ on hover to edit it; a file that can't be read shows its error in red. The header's **+** starts a new persona, and its ⋯ menu installs the defaults. The installer marks a default you've edited, which it would replace, and one installed from an older lgtm, which it picks to update. Pick personas, then the report: **Raw** (each persona's findings), **Synthesis** (merged into one set), or **Minimal** (only what matters to the change's task), and optionally **Cross-review**, where personas challenge each other first, with **Adversarial** under it. Run says how many reviewers it starts and on what, or why it can't. Runs list newest first with their outcome (findings and how many are high or critical, progress, or Halted); hover one for Stop, Retry, or Delete in ⋯.

Findings open in a pane beside the review, titled with the revset and a status pill; its ⋯ menu adds a synthesis, cross-review, or minimal report to a finished run. Tabs switch between Minimal, Synthesis, By persona (with a filter chip per persona and a severity tally), and Fixes. A card shows its severity, title, locations, who raised it, and its id; click the title to open its evidence and fix. A location jumps to its code, and on hover ↗ pastes the finding into a terminal's prompt and the comment icon adds it as a comment there. Check findings to act on several: a bar at the bottom sends them to a terminal or has a headless agent fix them (**Fix headless**), whose reply shows in Fixes.

Navigation: **Graph** (`g`, or Graph in a pane's **+** menu) opens a pane with the review's changed symbols nested in folders, files, and their enclosing classes or functions, with links between those that use each other. It opens on files for a small review and on folders for a big one, and remembers what you opened per review. Click a folder, file, or class to open or close it, click a symbol to jump to its code, and ⌘-click anything to jump to it. A closed box shows one link per box it connects to, and a line along its bottom for how much of everything in it is reviewed. A symbol's sign shows whether it was added (+), modified (~), or removed (−). Links leave an open box through a port on its right edge and come in through one on its left, one port per box at the other end, so two boxes are joined by a single link however much inside them connects. Links run at right angles and merge where they head into the same box, like a bus, with a dot where they join. A symbol's **N callers** badge brings in the unchanged code that uses it (dashed), and each caller's badge goes one hop further; the toolbar's **Callers** chip shows every changed symbol's callers. Links are dim until you hover or click a box, which lights its links. The toolbar also has Expand all, Collapse all, and a **?** legend; zoom in, zoom out, and fit sit at the bottom right. The Symbols tab (⌘⇧Y) lists the same symbols.

Languages: Rust, TypeScript, JavaScript, Python, Go, Swift, and shell get highlighting and symbols (functions, types). Markdown, JSON, YAML, TOML, CSS, SCSS, HTML, SQL, and Makefiles get highlighting and an outline as symbols (headings' sections, nested keys and tables, style rules, ids, `CREATE` statements, targets), which `e` expands through but the graph never links. Dockerfiles get highlighting only. Other files show as plain text.

**Walkthrough** (`h`, or **Write walkthrough** in the Walkthrough tab, or its ⋯ menu) runs a headless `claude -p` that reads the change and writes notes pinned to the code, in the order to read them. While it writes, the tab shows what it's doing, with a Stop button. Each note has a category, marked with a colored icon:

- **Important** (warning triangle, red, amber, or grey by level): needs a careful look, at a high, medium, or low level. These also show as a colored strip in the gutter (hover for why) and as a dot in the corner of graph boxes.
- **Flow step** (compass): one numbered step of a named flow through several places.
- **Context** (bulb): intent or background the diff doesn't show.
- **Question** (question circle): something for you or the author to settle.

Notes show as blocks under their code, and the Walkthrough tab (⌘⇧H) lists them. The tab's tour card picks the tour: **Reading order** covers every note, and each flow is its own tour. Its ‹ › stepper, like `n` and `N`, steps through the active tour, moving the cursor to each note, and each tour remembers where you are. The notes list as a path, with the ones behind you dimmed and the current one ringed; important notes show their level and flow steps their place in the flow. The category chips, with their note counts, hide or show a category (dashed when hidden), and the eye button turns the blocks under the code off. A note's **Comment** button adds it as your own comment, with its category as an emoji. Notes are never published; each new walkthrough replaces the last.

The walkthrough prompt is a markdown file: the repo's `.lgtm/walkthrough.md`, else `~/.config/lgtm/walkthrough.md`, else the built-in one. The Walkthrough tab's ⋯ menu shows which prompt is in use, and its **Edit global prompt** and **Edit repo prompt** open that file, copying the built-in prompt there first. Delete the file to go back to the built-in prompt. The note format the agent returns is fixed.

Pull requests: the Workspaces sidebar's **PR** icon button toggles a field that takes a PR number or URL (Escape closes it) and checks the PR out into its own `pr-<N>` workspace. Below the workspaces, **On this stack** lists the open PRs of bookmarks pushed from the selected workspace's stack (`@`'s ancestors down to `trunk()`, and their descendants, so a new change on a stack commit still sees the commits above it), nearest `@` first. A PR counts when its head branch has a pushed bookmark's name in the GitHub repo that bookmark was pushed to, so PRs from your fork show up too. Click one to preview it in a **PR #N** tab, read straight from GitHub without checking anything out: the PR's conversation and review summaries on top, then its diff with review threads under their lines. Each click replaces the preview. Double-click a PR, or press Enter on it, to review it in its own workspace like the PR button does. The list loads when you switch workspaces and on its refresh button; it needs `gh` signed in. When it's empty, its note says whether no bookmarks were pushed from the stack or which ones have no open PR.

Agents in terminals: lgtm offers to register itself with Claude Code (`claude mcp add -s user lgtm -- <app> mcp`). Then `claude` in an lgtm terminal gets tools to read the open review (progress, unreviewed ranges, comments, walkthrough, symbol graph, Reviewer findings), add comments and replies, write a walkthrough, submit its own findings as a Reviewers run, and show code or change the revset in the review pane. It can't mark code reviewed. Outside an lgtm terminal the server has no tools.

At startup LGTM checks for `jj` 0.40.0+ and `claude` 2.1.22+, and says how to install or update whichever is missing.

Mouse: click a row, drag or shift-click for a range, double-click for the function, triple-click for the hunk. Click a file header, hunk header, or ƒ marker to select that unit.

## Config

Optional, in the repo at `.lgtm/config.toml`:

```toml
# Files to skip: they start collapsed and count as skipped.
skip = ["*.lock", "**/generated/**"]

[anchor]
window = 2       # context lines above and below a marked line
threshold = 0.5  # minimum context score for a duplicate line to match
```

## Anchoring check

`cargo run --release --example anchor_eval -- <repo> [commits] [gap]` replays a repo's history: it marks every line each commit added, then matches those marks `gap` commits later and reports how many land on the right line for each window and threshold.
