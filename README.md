# lgtm

Desktop code review for agentic development, built on jj. See `docs/` for the spec, stack, and plan.

## Develop

Needs Rust, Node 22, pnpm, [just](https://github.com/casey/just), and jj 0.46 or newer (for `jj util snapshot`).

```sh
pnpm install
just dev         # hot reload; shows the repo picker
just test        # tsc + biome, vitest, cargo test (the Rust tests run jj)
just build       # release binary and .app
just run [repo]  # run the release build
```

Debug builds regenerate `src/bindings.ts` from the Rust commands. `cargo test` fails if it's stale; rerun with `UPDATE_BINDINGS=1` to refresh it.

A built app takes a repo path as its argument: `lgtm /path/to/repo`. A second launch for the same repo focuses its window.

## Shortcuts

| Keys | Action |
|---|---|
| ⌘T | New terminal tab |
| ⌘D | Split terminal right |
| ⌘⇧D | Split terminal down |
| ⌘⇧E | Show the Files panel (reopens it if closed) |
| ⌘⇧O | Show the Reviewers panel |
| ⌘⇧Y | Show the Symbols panel |
| ⌘⇧H | Show the Walkthrough panel |

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
| `x` | Check or uncheck the file under the cursor |
| `za` | Collapse or expand the hunk, or the file on its header |
| `g` | Open the symbol graph |
| `h` | Walkthrough: an agent guides you through the change |
| `]` / `[` | Next / previous hunk |
| `}` / `{` | Next / previous file |
| `n` / `N` | Next / previous note in the walkthrough's tour |
| `Esc` | Back to the cursor row |

The Files panel (⌘⇧E or the toolbar's Files button) shows the review's files as a tree with reviewed/changed counts per file and folder. It follows the review's line filter. Click a file to jump to it.

In the header and review toolbar:

- **Revset:** the selector in the center of the header shows what's reviewed (`@` by default). Click it or press `r` to open `jj log` as you have it configured, then pick a commit or range, or type a revset.
- **View:** Combined, Per change, or Since last session (what each change does differently from the last time this revset was reviewed). The second picker chooses the change.
- **Line filter:** All, Unreviewed, Reviewed, Skipped, Flagged. Hidden rows become thin gap bars; click one to show its rows.
- Progress counts for the whole review.

Each file header has a checkbox. Checking marks the file reviewed and collapses it, without touching its line marks. If new rows show up later, the file unchecks itself and the new rows turn amber. Marking a whole-file selection reviewed (`e` up to file, then `space`) checks it too.

Reviews update live: edits in the working copy are snapshotted after 300 ms of quiet, and any new jj operation reloads open reviews. Selections stay on the same code.

Reviewers: the Reviewers panel (⌘⇧O) runs review personas on the open review, each as its own headless `claude -p` that can only read files. Personas are markdown files in `~/.config/lgtm/personas` or the repo's `.lgtm/personas`. The panel can install the defaults, open a persona to edit, or start a new one. Pick personas and reports (synthesis, cross-review, minimal), then Run. Findings open as cards in a pane beside the review. A card's locations jump to its code, Comment adds it as a comment there, and Send to terminal pastes it into a terminal's prompt.

Navigation: **Graph** (`g`) opens a pane with the review's changed symbols nested in folders, files, and their enclosing classes or functions, with links between those that use each other. It opens on files for a small review and on folders for a big one, and remembers what you opened per review. Click a folder, file, or class to open or close it, click a symbol to jump to its code, and ⌘-click anything to jump to it. A closed box shows one link per box it connects to, and a progress bar for everything in it. Links leave an open box through a port on its right edge and come in through one on its left, one port per box at the other end, so two boxes are joined by a single link however much inside them connects. Links run at right angles and merge where they head into the same box, like a bus, with a dot where they join. A symbol's **+N callers** badge brings in the unchanged code that uses it (dashed), and each caller's badge goes one hop further; the toolbar checkbox shows every changed symbol's callers. Hover a box to highlight its links. The Symbols panel (⌘⇧Y) lists the same symbols.

**Walkthrough** (`h`, or Walkthrough in the review bar) runs a headless `claude -p` that reads the change and writes notes pinned to the code, in the order to read them. Each note has a category:

- ⚠️ **Important**: needs a careful look, at a high, medium, or low level. These also show as a colored strip in the gutter (hover for why) and outline graph boxes.
- 🧭 **Flow step**: one numbered step of a named flow through several places.
- 💡 **Context**: intent or background the diff doesn't show.
- ❓ **Question**: something for you or the author to settle.

Notes show as blocks under their code, and the Walkthrough panel (⌘⇧H) lists them. Pick a tour there: **Reading order** covers every note, and each flow is its own tour. `n` and `N` step through the active tour, moving the cursor to each note, and each tour remembers where you are. The panel's emoji buttons hide or show a category, and **In review** turns the blocks under the code off. A note's **Comment** button adds it as your own comment. Notes are never published; each new walkthrough replaces the last.

The walkthrough prompt is a markdown file: the repo's `.lgtm/walkthrough.md`, else `~/.config/lgtm/walkthrough.md`, else the built-in one. **Edit global** and **Edit repo** in the Walkthrough panel open that file, copying the built-in prompt there first. Delete the file to go back to the built-in prompt. The note format the agent returns is fixed.

Pull requests: the Workspaces sidebar's **PR** button takes a PR number or URL and checks the PR out into its own `pr-<N>` workspace. Below the workspaces, **Pull requests** lists the open PRs of bookmarks pushed from the selected workspace's stack (`@`'s ancestors down to `trunk()`, and its descendants), nearest `@` first. A PR counts when its head branch has a pushed bookmark's name in the GitHub repo that bookmark was pushed to, so PRs from your fork show up too. Click one to review it like the PR button does. The list loads when you switch workspaces and on **Refresh**; it needs `gh` signed in.

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
