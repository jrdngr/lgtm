# LGTM spec

LGTM is a desktop app for the code review part of agentic development. Most features are about review. Basic agentic workflows (workspaces, terminals, sending work to an agent) are also supported.

Inspirations, not parity targets: Orca ADE, tuicr, Crit.

## 1. Product

- v1 targets macOS. Keep the code portable to Linux and Windows: no macOS-only APIs outside a platform module.
- jj only. git is not supported in v1.
- One app instance per repo. One instance holds many jj workspaces.
- Every major view is a tab that can be docked into custom splits. Each workspace has its own layout, remembered per repo.
- Workspaces and the file tree are views in side panels left and right of the pane grid. Keys show and hide each side panel and jump to each view.
- Keybinds cover navigation and marking.
- A basic file tree and file editor with syntax highlighting.
- Full terminal support with arbitrary splits. The user runs their agent here.

## 2. Comments

- Comments attach to a selection and persist with the same anchoring as marks.
- A comment can be sent to an agent or published to a GitHub PR.
- Comments can be pulled down from a GitHub PR and replied to locally.
- Core workflow: pull a GitHub comment, work out a reply (with triage or an agent in a terminal), then send the final reply back to GitHub.
- A comment block shows under the last changed row it covers. A comment covers changed rows in one file and keeps the code it was made on, for when that code is gone.
- The Comments view lists a review's comments, including ones whose code is gone.
- Publishing adds a comment to the user's pending review on the PR, as a reply on its thread or a new thread on its code. The Comments view submits the review with a verdict and summary. The PR's conversation (comments not on code, and review summaries) shows in a PR view, with a box that posts to it right away.

## 3. Agent reviews

Reports of findings that agents in terminals publish through the MCP server.

- The app owns the finding format. Agents publish structured findings, and the report is UI built from them, not text.
- An agent can add the fixes the change's task needs: ordered actions, each closing findings, with off-task critical and high findings and questions.
- Findings link to code, can become comments, and can be sent to an agent in a terminal.

## 4. VCS layer

### Scope

- jj only. For a git-only repo, offer to run `jj git init --colocate` and refuse to continue otherwise.
- A single `Jj` adapter in the backend wraps CLI calls.
- Every read uses `--ignore-working-copy`. One debounced (about 300 ms) `jj util snapshot` worker runs per workspace. Never run two writing jj commands in one workspace at once.
- Use jj for revsets, diffs, and undo wherever it already does the job.

### Workspaces

- New workspaces (`jj workspace add`) go under `~/.local/share/lgtm/ws/<repo>-<hash>/<workspace>`. The path has no spaces, so Makefiles and shell tools work. The repo's main workspace is listed too.
- "New workspace" runs `jj workspace add -r trunk()`, then opens a terminal there that runs the repo's `.lgtm/setup` script if present.
- The sidebar switches between workspaces. Each has its own panes and layout. Switching hides the other workspaces' panes; their terminals keep running.
- The default review target is `@`. The review bar shows the target as jj colors it. The revisions panel shows `jj log` as the user has it configured. Picking one commit or a range, or typing a revset, sets the review. It also edits a commit, adds a new one before or after it, squashes it into its parent, and undoes or redoes, and the review follows `@`.
- Reviewing someone else's PR creates its own workspace on the PR head, so agent edits never touch other work.
- Deleting a workspace forgets it and removes its directory. `lgtm gc` cleans up any left behind.

### Diff output

- Hunks come from `jj diff --git`.
- File metadata (status, old path, file types, exec bit, conflict) comes from a hand-built `-T` JSON template, one object per line. Joined with the hunks on path.
- Line counts come from `self.diff().stat().files()`.
- `json(self)` only covers commit_id, parents, change_id, description, author, committer. Other fields are built by hand.

### Review targets

- A review is a revset R.
- Combined view:
  1. `jj diff -r R`.
  2. If jj rejects R for having gaps (for example, a PR that merged trunk), and `heads(R)` and `heads(::R ~ R)` are each a single commit, use `--from 'heads(::R ~ R)' --to 'heads(R)'`. That matches GitHub's PR diff.
  3. Otherwise reject the revset with jj's hint.
- Per-change view: step through each change in the stack with its own diff.
- The toolbar's view menu switches between Combined, Per change (with a change picker), and Since last session.
- Marks attach to code, not to a view.

### Re-review

- Each review session snapshots the commit ID it saw per change.
- Old commits map to current ones through evolog predecessors, not by change ID, because split and squash break change IDs. jj has no successors revset, so scan current commits' evolog for the predecessor.
- "Since last session" view: `jj interdiff` from each old commit to its successors. After a split, compare the old commit against the combined successor range.
- Hidden commits stay readable until `jj util gc`. A snapshot that no longer resolves means "no previous session".
- New and removed changes in the stack are listed separately.
- PRs work the same way, with the fetched head commit as the snapshot.

### Live updates

- Watch `.jj/repo/op_heads/heads/` and emit `repo-changed`. Working-copy edits reach it after the snapshot worker runs.
- Surface a stale working copy. Don't auto-run `update-stale`.

### PR code

- Same-repo PR: `jj git fetch -b <head-branch>`.
- Fork PR: add a remote named `gh-<owner>` on demand, then fetch the head branch.
- Fallback when the branch is gone: `git fetch origin pull/N/head:refs/heads/lgtm/pr-N` into jj's git store, then `jj git import`. Clean these bookmarks up when the workspace is deleted.
- GitHub calls go through the `gh` CLI, which holds the user's auth.
- A conflicted change can go to an agent to resolve, pasted into a terminal.

## 5. Marking and anchoring

### Marking

Marking is two steps: select code, then choose a mark. The same selection is the target for every action (mark, comment, send to agent, copy).

#### Selection

- There is always a selection. With nothing chosen it is the cursor row.
- Units: line, range, function/class, hunk, file, whole review. Only changed rows are marked. Context rows in a selection are ignored, and the menu shows how many rows an action will touch.
- Mouse: click selects a row, drag or shift-click selects a range, double-click on code selects the word and on the gutter selects the enclosing function, triple-click selects the hunk. Clicking a hunk header, or file header selects that unit. ⌘A selects the whole review. Clicking empty space clears.
- Keyboard: `j`/`k` move the cursor, `J`/`K` or `v` extend, `e` expands one level (line, function, hunk, file, review, skipping levels that don't apply) and `E` shrinks. The status bar shows the current level, e.g. "function: `get_user`". `Esc` clears.
- In split view a selection covers both sides of its row pairs.
- Functions come from tree-sitter (Section 6). A Python symbol includes its decorators. When the parse has errors in the range, `e` skips the function level.
- The selection has its own color, distinct from every mark color.
- Selections follow the code. When the diff changes, a selection stays on the same rows. If some are gone it shrinks to what's left; if all are gone it clears.

#### Choosing the mark

- `1`–`9` mark the selection with the color in use on that key. `0` clears its marks.
- `c` comments on the selection (Section 2).
- `m` and right-click open the same menu: the colors in use on their numbers, `0` clear mark, `c` comment, `a` send to agent, plus Copy. Right-clicking outside the selection first selects the row under the pointer.
- Marks never toggle. Applying a mark the code already has changes nothing. Clear mark removes it.
- Mark keys ignore key repeat.
- The cursor stays put after marking.
- ⌘Z / ⇧⌘Z undo and redo mark changes, including file checkboxes.
- Code text can be selected by dragging in the code column. Line numbers and diff signs can't. Dragging in the gutter selects rows only. Marking clears any text selection. ⌘C copies selected text, or the selected rows' code when there's none.

#### Labels and states

Marks are colors: a gutter strip plus a faint tint. There are nine (cyan, red, grey, orange, violet, pink, yellow, blue, tan), none green or amber so they stand out on added rows and from amber rows. Each color has:

- An optional label. A labeled color shows as a box with the label, an unlabeled one as a square.
- Whether its rows count as done, for progress.
- Whether it's in use. The colors in use sit in the review toolbar in order, and the order sets their number keys. A + opens the palette of all nine to add, remove, rename, and drag to reorder. Removing a color keeps its marks.

The defaults, on `1`–`3`:

- **reviewed**: cyan, done.
- **flagged**: red, "come back to this", not done.
- **skipped**: grey, done.

The setup is global, in `settings.toml`, the same in every repo.

One derived state:

- **Changed since reviewed**: amber. An unmarked row that replaced a line whose mark was lost, found by the same line diff used for remapping. Needs the previous session's match state, since marks are repo-scoped and most unmatched marks are unrelated.

Line marks never collapse anything. Hunks collapse only by hand (`za` or the hunk's collapse button).

#### File reviewed status

- Each file header has a checkbox. `x` toggles the checkbox of the file under the cursor. Checking it marks the file done and collapses it; its unmarked rows show in the first color. A whole-file selection marked with the first color does the same, when that color counts as done.
- The header counts the file's marks by color. An eraser, on hover, clears the file's marks and unchecks it in one undo step.
- File status is separate from line marks. Checking never changes line marks, and unchecking restores them exactly.
- Other colors on a whole-file selection apply to its lines. Files have no status but checked, and other colors never collapse a file.
- When every changed line in a file is done, its checkbox gets an accent ring. It is never checked automatically.
- Checking stores the anchors of the file's changed rows at that moment. It also stores the file's status, rename source, exec bits, and binary contents. If later changes bring in rows outside that set or change any of those, the file unchecks itself, expands, and its header shows "changed since reviewed". New rows are amber. A rebase that doesn't change the code keeps it checked.
- Files matching `skip` globs in `.lgtm/config.toml` (e.g. `*.lock`, `**/generated/**`) are skipped and start collapsed. Files skipped by hand don't collapse.
- Any file can be collapsed or expanded with its collapse button regardless of status.

#### Scope

- Marks are repo-scoped. A line reviewed in one review shows as reviewed in any other review in that repo with the same content, such as stacked PRs or a re-targeted revset.
- Each mark records its source review and timestamp, shown on hover ("reviewed in PR #123").
- "Reset marks for this review" removes the marks that review created.

#### Progress

- Per file and per review: line counts by mark color, and how many are done.
- A checked file counts as fully done in every progress figure, whatever its line marks say.
- The file tree shows per-file progress. The review header shows overall progress. The workspace sidebar shows each workspace's review progress.
- A line filter sits at the top of the review: All, Not done, then each color in use (single choice).
  - Only changed rows in that state show. Amber rows count as not done, and rows in a checked file count as done.
  - Hidden runs render as thin gap bars. Files and hunks with no matching rows are hidden. The file tree follows the same filter.
  - Each review pane keeps its own filter.
  - Clicking a gap bar shows the rows it hides until the filter changes.
  - A row marked out of the filter disappears and the cursor moves to the next matching row.

### Anchoring

Marks and comments are content-addressed diff lines, independent of commit IDs, change IDs, and line numbers. They survive new commits and rebases.

- Each marked line stores:
  - `path`
  - `side`: added or removed. Context rows are never keyed.
  - `lineHash`: hash of the normalized line text
  - `above` and `below`: hashes of the nearest 2 non-trivial lines above and below that are unique in the file, taken from the full file rather than the diff
- Each batch of marks stores the commit ID it was made on. Line hashes per (commit, path) are cached at mark time.
- Normalization depends on the language: collapse all whitespace for brace languages, strip only trailing whitespace for indentation-sensitive ones (Python, YAML, Makefile, Haskell).
- Matching runs in the backend:
  1. A `(path, side, lineHash)` that is unique among candidate lines matches outright.
  2. Duplicates are scored by `(2*above + below) / (3*window)` and matched one-to-one at >= 0.5. Above counts double because it usually holds the enclosing scope.
  3. Ties are broken by remapping: line-diff the file at the batch's commit against the current file. An exact remap wins unless another copy scores higher.
- Removed lines use the old side of the file the same way.
- A comment whose line was edited (no hash match) lands on a line the remap's line diff replaced it with, the one whose context scores highest, when at least one context hash still matches. Marks don't: their lines turn amber.
- Renames: `jj diff --git` reports them up to about 50% changed. Above that, match unmarked added lines against marks on deleted paths in the same review.
- Window size and threshold live in config. Defaults are window 2 and threshold 0.5. Replaying 150 commits of jj and Flask history (`anchor_eval`), 99.3% of marks landed on the right line and 0.02% on a wrong one.

## 6. Code structure

Draft. Tree-sitter supplies everything that needs to know what the code is.

- Grammars are bundled per language and loaded on demand. v1 set: Rust, TypeScript/JavaScript, Python, Go, Swift, shell, Markdown, JSON, YAML, TOML, CSS, SCSS, HTML, SQL, Makefile, Dockerfile. Markup, config, and style languages have outline symbols (kind `section`: headings, nested keys, tables, rules) that the graph never links.
- Each file is parsed on both sides of the diff (old and new), from full file contents, not hunks.
- Parsing runs in the backend. Results are cached by (commit, path) and reused across views.
- Highlighting uses tree-sitter highlight queries. Files with no grammar render as plain text and lose only the function selection level.
- Symbols come from per-language queries (`tags.scm` style): functions, methods, classes, impls, modules. Each symbol has a name, kind, and byte range.
  - A symbol's range includes its leading decorators, attributes, and doc comments (Python decorators, Rust `#[...]`, TS decorators).
  - Nested symbols nest. `e` expands from the innermost symbol around the cursor to its parent before reaching hunk level.
  - When the range has a parse error, the function level is skipped (Section 5).
- Selected code text highlights its other matches across the review: case-sensitive, whole words for a one-word selection, single-line selections of 2 or more characters.
- Hunk headers show the enclosing symbol name, like `git diff`'s function context.
- The language's normalization class for anchoring (brace or indentation-sensitive, Section 5) is declared alongside its grammar.
- Symbol data feeds navigation (Section 7): changed-symbol lists, jump to definition within the review, and the graph views.
- Out of scope for v1: LSP, cross-file type resolution, semantic diff.

## 7. Navigation

- Graph-based views and navigation of the change.
- Agent highlighting of the parts of the change most likely to need human review.
- Possibly an MCP server so an agent in a terminal pane can drive these features.

## 8. Terminals and agents

- Full terminal emulation in panes with arbitrary custom splits.
- A comment sent to the agent goes to a terminal pane and is pasted into the prompt by default. More delivery options later.
- v1 agent is Claude Code. Two ways to run it:
  - Headless: the app spawns `claude -p` with stream-JSON output for highlighting and triage.
  - Terminal: the app pastes a prompt into a pane already running `claude`. This covers users on a subscription plan.
- Other agents (Codex, pi, etc.) are planned. Agent-specific code stays behind one adapter.

