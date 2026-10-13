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
| ⌘, | lgtm › Settings…: opens the Settings tab |
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
| ⌘⇧M | Show the Notes tab |
| ⌘⇧P | Show the PR tab (PR workspaces) |
| ⌘⇧T | Open or focus the Threads tab (PR workspaces) |
| ⌘⇧H | Show the Walkthrough tab |
| ⌘⇧O | Show the Reviewers tab |
| ⌘⇧I | Show the Design tab |

In a review pane:

| Keys | Action |
|---|---|
| `j` / `k` | Move the cursor |
| `J` / `K`, or `v` then `j` / `k` | Extend the selection |
| `e` / `E` | Expand / shrink: line, function, hunk, file, review |
| `1`–`9` | Mark with that color (the selection then collapses to the cursor) |
| `0` | Clear the mark |
| `m` or right-click | Mark menu: the colors on their numbers, `0` clear, `c` comment, `g` change, `a` send to agent |
| ⌘A | Select the whole review |
| ⌘Z / ⇧⌘Z | Undo / redo mark changes |
| ⌘C | Copy selected code text, or the selected rows when no text is selected |
| `c` | Write a note on the selection |
| `C` | Write a draft for GitHub on the selection (PR workspaces) |
| `a` | Ask the agent about the note or thread at the cursor |
| `x` | Check or uncheck the file under the cursor |
| `za` | Collapse or expand the hunk, or the file on its header |
| `g` | Open the symbol graph |
| `h` | Walkthrough: an agent guides you through the change |
| `]` / `[` | Next / previous change (each run of changed rows) |
| `}` / `{` | Next / previous file |
| `n` / `N` | Next / previous note in the walkthrough's tour |
| `Esc` | Back to the cursor row |

The right panel has seven icon tabs, plus PR in a PR workspace. PR counts threads that need you (new or waiting on a decision), Walkthrough its notes, Reviewers its running reviews, and Design its open questions.

- **Files** (⌘⇧R) shows the whole repo.
- **Changes** (⌘⇧E) shows the review's files as a tree, with the rows done so far in its header. Each file has a ring filling as you review it, or a ✓ once done; hover it for the counts, or hover a folder to see its count. Done files dim, deleted ones are struck through, and a file changed since you checked it says **changed**. It follows the review's line filter. Click a file to jump to it.
- **Symbols** (⌘⇧Y) lists the changed symbols by file, with how many unchanged callers use each and how much of it is done. With no Review tab open, clicking a file or symbol opens one on the current revset.
- **Notes** (⌘⇧M) lists your notes by file, with done ones folded at the end. A row shows when the agent answered or changed the code. Its ⋯ menu sends the open notes to a terminal. Notes never leave your machine.
- **PR** (⌘⇧P, PR workspaces only) shows the PR: its state, a **Yours** pill on your own PR, its title, a bar of how far its threads are, branches, ↗ to open it on GitHub, and ⟳ Sync, which fetches the PR, pulls its threads, and reloads its conversation. The conversation folds to one line of each reviewer's verdict. Below, every thread on the PR (review threads, comments and review summaries on the PR itself, and your drafts) is grouped by stage: New, Decide, To fix, Reply, Ready, Done (folded). Click a thread to open it in place and write its reply; ⌘-click shows its code in the review. The box at the bottom sends what's Ready (see below).
- **Walkthrough** (⌘⇧H) lists the walkthrough's notes.
- **Reviewers** (⌘⇧O) runs review personas.
- **Design** (⌘⇧I) lists the workspace's designs with their step and open questions, and the current design's next question with **Go with 1** (see Design below).

Notes and threads show under their code as cards: blue for a note, purple for a GitHub thread or draft. Their actions show on hover: ✎ edit, ✓ done (notes), ✦ ask the agent (or `a`), ↗ send to a terminal, and in ⋯: Move to a stage (threads), Draft PR comment from this (notes, in PR workspaces), and Delete (notes and drafts). The agent box stays open while there's a conversation with the agent. Done notes, and done or resolved threads, fold to one line; click to open one.

Threads move through stages: **New** (synced, not triaged), **Decide** (needs your call), **To fix**, **Reply** (a reply is being written), **Ready** (the reply is final, or the fix needs none), and **Done**. Writing a reply moves a thread to Reply; **Final** moves it to Ready, and editing a final reply moves it back. A draft (`C`) is Ready once it has text. Sync keeps each thread's stage, brings one back to New when someone posts to it, and marks it Done when it's resolved. Comments on the PR from others come in as threads too, Done when you posted to the PR after them.

The **Threads** tab (⌘⇧T, or the PR tab's header icon, or a thread card's icon) is where you work a PR's threads down. Each thread is one row (who spoke last, summary, location, the latest comment, the take) in stage sections, with Done folded at the end. The thread at the cursor opens in place: on the left what's unanswered since your last post (earlier posts fold) and the code; on the right the take, the reply, and an agent chat (↵ to ask, ⌘↵ to change the code). **✦ Triage N new** (or `t`) has a headless agent read each new thread's code and the change, then give it a summary, a take (Agree, Agree with changes, Push back, Needs you) and a draft reply, and sort it: Decide for a call only you can make, To fix, Reply, or Done for threads to skip (bots, ones you answered). It never runs on its own. The To fix section's **Fix N** has an agent change the code in `@` for those threads; a thread fixed as asked moves to Ready, and one fixed differently gets a reply saying so and moves to Reply. **Send to terminal** pastes them into a terminal's prompt instead. The agent chat on a thread rewrites its take and reply together when you change your mind, marked "updated take and reply". The ⋯ menu has Copy final replies (final replies, threads fixed as asked, and what's still open, as text) and Edit reply prompt…, which opens Settings › Prompts. The rules for takes and replies are the **Replies** prompt (`reply.md`). Agents never post or resolve anything; only Send does. Clicking a thread in the PR tab focuses it here while this tab is open. Keys:

| Key | Action |
| --- | --- |
| `j` / `k` | Next / previous thread |
| `f` | To fix |
| `r` | Reply, with the cursor in the agent chat |
| `e` | Edit the reply (Esc or ⌘↵ leaves the box) |
| `y` | Mark the reply final (Ready) |
| `d` | Done, without sending |
| `o` | Show the thread's code in the review |
| `t` | Triage the new threads |
| ⌘↵ | Send to GitHub (opens the list of what goes out) |

**Send to GitHub** in the PR and Threads tabs sends everything Ready at once: replies on code and new drafts go into one review, so reviewers get one notification, and replies to comments on the PR post as PR comments that open with a quote of what they answer. On someone else's PR the button carries your verdict (Comment, Approve, Request changes) and summary. Ready threads with no reply (fixed as asked) follow the **Fixed threads** choice, remembered per repo: leave as is, reply "Done", resolve, or both. A short list of what goes out shows before it sends; sent threads move to Done and the PR syncs. New drafts need the whole PR open in the Combined view, where GitHub's lines match.

In the header and review toolbar:

- **Revset:** the selector in the center of the header shows what's reviewed (`@` by default), with or without a Review tab open: one change as its id and description, a range as `oldest → newest` with its newest description and change count. Its border turns amber when the change has no description or the working copy is stale. New Review tabs open on it. Click it or press `r` to open `jj log` as you have it configured. Move with `j` / `k` or a click and extend a range with `J` / `K` or a shift-click, then press ↵ or the Review button; or type a revset (`/`); or click a preset: `@`, `@-`, or `main..@` (named for the bookmark `trunk()` resolves to). The cursor row's Edit, New ▾ and Squash buttons, or `e`, `n` / `N` and `s`, edit the commit, add one after or before it, or squash it into its parent. `u` / `U` undo / redo the last of these.
- **View:** Combined, Per change, or Since last session (what each change does differently from the last time this revset was reviewed). The second picker chooses the change.
- **Line filter:** All lines, Not done, then each mark color in use. Hidden rows become thin gap bars; click one to show its rows.
- **Mark colors:** marks are colors, each with an optional label: by default **reviewed** (violet), **flagged** (red) and **skipped** (grey) on `1`–`3`. The toolbar shows the colors in use with their keys, labeled ones as a box with the label and the rest as a square. Click one to mark the selection, or right-click it to filter to it. Its **+** opens the palette of all nine colors: rename a color, drag it to change its number key, set whether its rows count as done (flagged doesn't by default), remove it (its marks stay), or add one on the next key. The setup is the same in every repo and saves to `~/.config/lgtm/settings.toml`.
- **Layout:** the two icons pick Unified or Split; the lit one is current. In Split, each side scrolls sideways on its own, with a scroll bar under each. Scrolling sideways over the diff moves both, and the one with room left keeps going after the other hits its end.
- Progress for the whole review: a bar of marked rows by color, then rows done because their file is checked or skipped, with the share done. Only added rows count when a change both removes and adds lines; a change that only removes lines counts its removed rows. Hover it for the counts. **Reset review…** in the ⋯ menu at the end clears every mark in it after you confirm.

Bottom panel: a second pane grid docked under the panes, between the side panels, which pushes the panes up. Its tabs and splits work like the main grid's, and each workspace has its own. Drag tabs between it and the main grid; terminals keep running when they move, but an editor with unsaved changes stays put. It opens with a terminal when it would open empty. Hiding it with ⌃\` keeps its terminals running, and closing its last tab hides it too, while dragging its last tab out leaves it open. Drag its top edge to resize; the height and whether it's open are remembered.

Panes: the **+** tab after each pane's tabs opens a new Terminal or Graph tab in that pane. The split icons at the right of each pane's tab strip split it right or down into an empty pane, which offers the same kinds of tab.

Each file header has a checkbox. Checking marks the file done and collapses it, without touching its line marks; its unmarked rows show in the first color. If new rows show up later, the file unchecks itself and the new rows turn amber. Marking a whole-file selection with the first color (`e` up to file, then `1`) checks it too, when that color counts as done. The header counts the file's marks by color, and on hover its eraser clears every mark in the file and unchecks it (⌘Z brings them back).

Reviews update live: edits in the working copy are snapshotted after 300 ms of quiet, and any new jj operation reloads open reviews. Selections stay on the same code.

Reviewers: the Reviewers tab (⌘⇧O) runs review personas on the open review, each as its own headless `claude -p` that can only read files. Personas are markdown files in `~/.config/lgtm/personas` or the repo's `.lgtm/personas`. Each shows its description, a **repo** pill if it's the repo's, and ✎ on hover to edit it; a file that can't be read shows its error in red. The header's **+** starts a new persona, and its ⋯ menu installs the defaults. The installer marks a default you've edited, which it would replace, and one installed from an older lgtm, which it picks to update. Pick personas, then the report: **Raw** (each persona's findings), **Synthesis** (merged into one set), or **Minimal** (only what matters to the change's task), and optionally **Cross-review**, where personas challenge each other first, with **Adversarial** under it. Run says how many reviewers it starts and on what, or why it can't. Runs list newest first with their outcome (findings and how many are high or critical, progress, or Halted); hover one for Stop, Retry, or Delete in ⋯.

Findings open in a pane beside the review, titled with the revset and a status pill; its ⋯ menu adds a synthesis, cross-review, or minimal report to a finished run. Tabs switch between Minimal, Synthesis, By persona (with a filter chip per persona and a severity tally), and Fixes. A card shows its severity, title, locations, who raised it, and its id; click the title to open its evidence and fix. A location jumps to its code. On hover, ↗ pastes the finding into a terminal's prompt, **Keep as note** adds it as one of your notes at its first location, **Draft PR comment** (PR workspaces) drafts it for GitHub there, and **Dismiss** settles it. Check findings to act on several: a bar at the bottom sends them to a terminal or has a headless agent fix them (**Fix headless**), whose reply shows in Fixes. Findings a finished fix covered show **Fixed**, and dismissed ones **Dismissed** (with **Bring back**); both fold into a Settled section at the end of each list.

Navigation: **Graph** (`g`, or Graph in a pane's **+** menu) opens a pane with the review's changed symbols nested in folders, files, and their enclosing classes or functions, with links between those that use each other. It opens on files for a small review and on folders for a big one, and remembers what you opened per review. Click a folder, file, or class to open or close it, click a symbol to jump to its code, and ⌘-click anything to jump to it. A closed box shows one link per box it connects to, and a line along its bottom for how much of everything in it is done. A symbol's sign shows whether it was added (+), modified (~), or removed (−). Links leave an open box through a port on its right edge and come in through one on its left, one port per box at the other end, so two boxes are joined by a single link however much inside them connects. Links run at right angles and merge where they head into the same box, like a bus, with a dot where they join. A symbol's **N callers** badge brings in the unchanged code that uses it (dashed), and each caller's badge goes one hop further; the toolbar's **Callers** chip shows every changed symbol's callers. Links are dim until you hover or click a box, which lights its links. The toolbar also has Expand all, Collapse all, and a **?** legend; zoom in, zoom out, and fit sit at the bottom right. The Symbols tab (⌘⇧Y) lists the same symbols.

Languages: Rust, TypeScript, JavaScript, Python, Go, Swift, and shell get highlighting and symbols (functions, types). Markdown, JSON, YAML, TOML, CSS, SCSS, HTML, SQL, and Makefiles get highlighting and an outline as symbols (headings' sections, nested keys and tables, style rules, ids, `CREATE` statements, targets), which `e` expands through but the graph never links. Dockerfiles get highlighting only. Other files show as plain text.

**Walkthrough** (`h`, or **Write walkthrough** in the Walkthrough tab, or its ⋯ menu) runs a headless `claude -p` that reads the change and writes notes pinned to the code, in the order to read them. While it writes, the tab shows what it's doing, with a Stop button. Each note has a category, marked with a colored icon:

- **Important** (warning triangle, red, amber, or grey by level): needs a careful look, at a high, medium, or low level. These also show as a colored strip in the gutter (hover for why) and as a dot in the corner of graph boxes.
- **Flow step** (compass): one numbered step of a named flow through several places.
- **Context** (bulb): intent or background the diff doesn't show.
- **Question** (question circle): something for you or the author to settle.

Notes show as blocks under their code, and the Walkthrough tab (⌘⇧H) lists them. The tab's tour card picks the tour: **Reading order** covers every note, and each flow is its own tour. Its ‹ › stepper, like `n` and `N`, steps through the active tour, moving the cursor to each note, and each tour remembers where you are. The notes list as a path, with the ones behind you dimmed and the current one ringed; important notes show their level and flow steps their place in the flow. The category chips, with their note counts, hide or show a category (dashed when hidden), and the eye button turns the blocks under the code off. On hover, a walkthrough note's **Keep as note** adds it as one of your notes, with its category as an emoji, and **Draft PR comment** (PR workspaces) drafts it for GitHub instead. Notes are never published; each new walkthrough replaces the last.

The walkthrough prompt is a markdown file: the repo's `.lgtm/walkthrough.md`, else `~/.config/lgtm/walkthrough.md`, else the built-in one. Manage it in Settings › Prompts; the Walkthrough tab's ⋯ › **Edit prompt…** opens that section. The note format the agent returns is fixed.

Pull requests: the Workspaces sidebar's **PR** icon button toggles a field that takes a PR number or URL (Escape closes it) and checks the PR out into its own `pr-<N>` workspace. Below the workspaces, **On this stack** lists the open PRs of bookmarks pushed from the selected workspace's stack (`@`'s ancestors down to `trunk()`, and their descendants, so a new change on a stack commit still sees the commits above it), nearest `@` first. A PR counts when its head branch has a pushed bookmark's name in the GitHub repo that bookmark was pushed to, so PRs from your fork show up too. Click one to preview it in a **PR #N** tab, read straight from GitHub without checking anything out: the PR's conversation and review summaries on top, then its diff with review threads under their lines. Each click replaces the preview. Double-click a PR, or press Enter on it, to review it in its own workspace like the PR button does. The list loads when you switch workspaces and on its refresh button; it needs `gh` signed in. When it's empty, its note says whether no bookmarks were pushed from the stack or which ones have no open PR.

Agents in terminals: lgtm offers to register itself with Claude Code (`claude mcp add -s user lgtm -- <app> mcp`). Then `claude` in an lgtm terminal gets tools to read the open review (progress by mark label, ranges not yet done, comments, walkthrough, symbol graph, Reviewer findings), add comments and replies, list the PR's threads and set a thread's take and reply or mark it ready (never posting to GitHub), write a walkthrough, submit its own findings as a Reviewers run, and show code or change the revset in the review pane. It can't mark code. Outside an lgtm terminal the server has no tools.

At startup LGTM checks for `jj` 0.40.0+ and `claude` 2.1.22+, and says how to install or update whichever is missing.

Mouse: click a row, drag or shift-click for a range, double-click for the function, triple-click for the hunk. Click a file header, hunk header, or ƒ marker to select that unit.

**Agent runs** (a tab from any pane's **+** menu, in the main or bottom panel) lists every headless agent run in the workspace, newest first: Ask and Change, thread reply chats, Triage, Fix threads, Walkthrough, conflict Resolve, Design steps, and Reviewers. Each row shows its kind, what it's about, what it's doing or how it ended, and its age; hover for Stop while it runs, Retry after it failed or stopped, and ↗ to show what it worked on. **Clear finished** empties the list of everything but running runs. The list lasts until the window closes, except Reviewers runs, which are stored.

## Design

The **Design** tab (from a pane's **+** menu, or a design in the Design panel) takes a design from a brief through questions to docs, after the qqueue workflow. Each design is a folder in the repo, `docs/design/<name>/` by default, holding `brief.md`, `design.md`, `questions.json`, and what's made from them: `docs/` (markdown and Mermaid), `diagrams/`, and `html/`. Comments on the docs, the agent's session, and what you last saw stay in lgtm.

The header's ▾ picks the design or makes a new one, the steps show where it is (Brief › Questions › Revise › Docs › HTML › Refine), and the views are:

- **Brief:** what to design, and files the agent reads first. **Draft design** (⌘↵) has a headless `claude -p` write `design.md` and its open questions. **Send to terminal** pastes a prompt for Claude Code in a terminal, which asks you through lgtm's MCP tools instead.
- **Questions:** one question at a time in the qqueue template: the last decision, "Question N of M", the subject, context, options with the recommended one first in green, and the recommendation. The rail lists every question: ✓ decided, ✎ changed, struck through when dropped, subquestions indented. After each decision the agent updates the questions still open: until it's done the next question is greyed and can't be answered, under a blue bar. A question it rewrote shows an amber bar with what changed; `p` shows what it said before. One it dropped says why. When none are open, **Revise design** folds the new decisions into `design.md`; the agent may ask follow-ups.
- **Docs:** the design's markdown, rendered with Mermaid diagrams. Blocks changed since you last looked are marked amber (`]` / `[` move between them; **Mark seen** clears them). Select text, then `c` to comment (queued) or `a` to ask (answered beside it). **Apply comments** (⇧⌘↵) has the agent make the queued changes.
- **Files:** the docs, diagrams and HTML pages, with a preview. **Make** asks the agent for a doc, Mermaid diagram, or HTML page about what you describe; **Make HTML** turns a doc or diagram into a standalone page, and says when its source changed since. HTML previews can't run scripts; **Open in browser** shows the page as is.

In Questions:

| Key | Action |
| --- | --- |
| `1`–`9` | Select an option (only the question's own) |
| ↵ | Decide the selected option |
| `n` | Add a note to the selected option, then ↵ to decide |
| `o` | Answer with none of the options |
| `d` | Discuss the question with the agent; ⌘↵ there asks it again |
| `s` | Later: move it to the end |
| `u` | Undo the last decision |
| `r` | Change a decided question's answer |
| `p` | Show or hide a rewritten question's previous text |
| `j` / `k` | Browse the questions |

The tab's ⋯ copies the question as qq text or every decision, and opens the design prompts. In terminals, the MCP server's `list_designs`, `design_questions`, `design_ask` and `design_answers` let Claude Code ask you a design's questions in the tab and read your answers.

## Settings

lgtm › Settings… (⌘,) opens a Settings tab. Settings are saved to `~/.config/lgtm/settings.toml` and apply to every window.

- **Claude command:** what runs Claude Code for every agent (default `claude`). It can be a command line, or an alias or function from your login shell, like `cc`. An alias is expanded once and run directly; a function runs through `$SHELL -l -i -c` each time. The line under it shows what it resolved to, and ⟳ reads your shell again. Registering the MCP server still runs plain `claude`.
- **Models:** the `--model` for Ask and Change (also Fix headless and conflict resolution), Walkthrough, Reviewers, Replies, and Design. Empty uses Claude Code's default.
- **Designs:** the folder designs go in, relative to the repo root (default `docs/design`).
- **Terminal font size**, applied to open terminals right away.
- **Tools:** where jj, claude and gh were found and their versions, and the MCP server's registration with a Register button.
- **Prompts:** each agent prompt's global and repo file, and which one is in use. Create copies the built-in prompt into the file; Delete removes it.

## Config

Optional, in the repo at `.lgtm/config.toml`:

```toml
# Files to skip: they start collapsed and count as done.
skip = ["*.lock", "**/generated/**"]

[anchor]
window = 2       # context lines above and below a marked line
threshold = 0.5  # minimum context score for a duplicate line to match

[design]
dir = "docs/design"  # where designs go, relative to the repo root
```

## Anchoring check

`cargo run --release --example anchor_eval -- <repo> [commits] [gap]` replays a repo's history: it marks every line each commit added, then matches those marks `gap` commits later and reports how many land on the right line for each window and threshold.
