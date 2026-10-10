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
| ⌘C | Copy the selected rows |
| `za` | Collapse or expand the hunk, or the file on its header |
| `Esc` | Back to the cursor row |

The Files panel (⌘⇧E or the toolbar's Files button) shows the review's files as a tree with reviewed/changed counts per file and folder. It follows the review's line filter. Click a file to jump to it.

In the review toolbar:

- **Revset:** defaults to `@`. **Pick…** opens `jj log` as you have it configured (default revset, template, colors). Click a commit, shift-click another for a range, then Review. Double-click reviews one commit.
- **View:** Combined, Per change, or Since last session (what each change does differently from the last time this revset was reviewed). The second picker chooses the change.
- **Line filter:** All, Unreviewed, Reviewed, Skipped, Flagged. Hidden rows become thin gap bars; click one to show its rows.
- Progress counts for the whole review.

Each file header has a checkbox. Checking marks the file reviewed and collapses it, without touching its line marks. If new rows show up later, the file unchecks itself and the new rows turn amber. Marking a whole-file selection reviewed (`e` up to file, then `space`) checks it too.

Reviews update live: edits in the working copy are snapshotted after 300 ms of quiet, and any new jj operation reloads open reviews. Selections stay on the same code.

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
