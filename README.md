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
| ⌘⇧H | Show the Hotspots panel |

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
| `h` | Highlight risks: an agent finds the parts that most need a careful look |
| `]` / `[` | Next / previous hunk |
| `n` / `N` | Next / previous hotspot |
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

Navigation: **Graph** (`g`) opens a pane with the review's changed symbols grouped by file, links between those that use each other, and unchanged callers (dashed). Each node has a review progress bar. Hover one to highlight its links, click it to jump to its code. The Symbols panel (⌘⇧Y) lists the same symbols. **Highlight risks** (`h`) runs a headless `claude -p` that marks the parts most in need of a careful look: a colored strip in the gutter (hover for why), the Hotspots panel (⌘⇧H), and outlines on graph nodes.

Agents in terminals: lgtm offers to register itself with Claude Code (`claude mcp add -s user lgtm -- <app> mcp`). Then `claude` in an lgtm terminal gets tools to read the open review (progress, unreviewed ranges, comments, hotspots, symbol graph, Reviewer findings), add comments and replies, set hotspots, submit its own findings as a Reviewers run, and show code or change the revset in the review pane. It can't mark code reviewed. Outside an lgtm terminal the server has no tools.

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
