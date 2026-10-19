# lgtm

Desktop code review for agentic development, built on `jj`.

This software is made for my personal use. If you don't want to install `jj`, `gh`, or `claude`, fork it.

You're welcome to file issues and feature requests, but I may or may not ever look at them. 

You're not welcome to file pull requests. I already spend too much time reading them. Turns out it's really fun to make your own editor and I don't want to make it unfun. Just fork it and make it your own.

This program may or may not be safe to run. Use it at your own risk.

## Screenshots

**Mark reviewed lines with customizable colored labels**

![Mark reviewed lines with customizable colored labels](docs/screenshots/01-review-marks-notes.png)

**jj stuff**

![jj stuff](docs/screenshots/03-revisions-list.png)

**Agentic code review reports**

![Agentic code review reports](docs/screenshots/10-reviewers-report.png)

**Structured Q&A**

![Structured Q&A](docs/screenshots/13-qa-question.png)

**Ask your agent to help you reply to PR comments about code you didn't write**

![Ask your agent to help you reply to PR comments about code you didn't write](docs/screenshots/18-pr-thread-agent.png)

**jj workspaces with their own terminals and layout**

![jj workspaces with their own terminals and layout](docs/screenshots/20-terminals.png)

The right panel's tabs are Files; Changes, Comments, Architecture and Reports; then Q&A. A tab's icon turns amber when something there waits on you: threads to triage or decide, open high Look items, or open questions. Comments (⌘⇧M or ⌘⇧P) holds a PR workspace's PR, its threads by stage, then your notes by file, with All, PR and Notes filters. Without a PR it holds your notes. Architecture has Generate, and with a review open, the score, Look items to check off, the layers, a map of the containers and the components the review changes, and History (click a commit for its change, shift-click a second to compare). Without a review it lists the system's tours, people, external systems, and containers with their components (stale ones marked). A row shows its node in the Architecture tab.

The header shows what the workspace reviews and where `@` is. A PR's review is blue with its number; any other review is neutral with its commit count. A dot per reviewed commit has `@` ringed on its commit, or past a dashed line when `@` builds on the review, and the tag after `@` says which commit it is, or that it's on top. When `@` is outside the review, the header, `@` and a banner (Review @, Edit Review Head) turn red, and so does a dot on the shown editor tab. Picking a PR reviews only its commits. Moving `@` elsewhere points the review at the PR's commits, or without a PR at `@` unless `@` is still in the review or on top of it. Blue marks review and PRs throughout. ⌘J opens `jj log` with the reviewed commits tinted blue and bracketed, `@` green: `r` reviews the commit (or a ⇧J/⇧K range), ⇧r adds it to the review, ↵ or `e` edits it. Right-click the header to review the PR, `@`, `@-`, trunk to `@` or any revset, or act on `@`; right-click a commit for the same and its jj actions.

Right-click a workspace to pin it to the top of the list or give it a color. Drag pinned workspaces to reorder them. The PR button in the Workspaces header shows or hides the stack's PR list; hiding it clears the selected PRs.

Everything sent to a terminal (⌘>, violet buttons, Send Path to Terminal) goes to the send target: the last terminal you focused, whose tab title is violet. Click the ›_ before the focused terminal tab's title (or pick Pin as Send Target from its menu) to keep sends going there; it turns violet while pinned. Rename on a terminal tab's menu edits its title in place. Violet buttons also press Enter when an agent runs in the target, so it starts right away; ⌘> and Send Path only paste.

## Requirements

- [jj](https://github.com/jj-vcs/jj) 0.46 or newer: `brew install jj`
- [Claude Code](https://claude.com/claude-code), for the agents: `curl -fsSL https://claude.ai/install.sh | bash`
- [gh](https://cli.github.com), signed in, for pull requests: `brew install gh && gh auth login`

lgtm checks for these at startup and says what's missing. Only jj is required.

To build it you also need Rust, Node 22, pnpm, [just](https://github.com/casey/just), and the Xcode command line tools.

## Install

Download the .dmg from [Releases](https://github.com/jrdngr/lgtm/releases) and drag lgtm to Applications. Or build it:

```sh
git clone https://github.com/jrdngr/lgtm
cd lgtm
pnpm install
just install     # release build, copied to /Applications
```

The app's binary takes a repo path: `lgtm /path/to/repo`. Launched without one, it reopens the last repo, or shows the repo picker when that's gone. A second launch for the same repo focuses its window. Plain git repos get an offer to set up jj alongside git.

On first run, lgtm offers to register itself as an MCP server with Claude Code, so `claude` in an lgtm terminal can read and comment on the open review, and sync a PR's threads (stage, take and reply) with a review-reply workflow run in the terminal. Each write names the review's revset, or a thread's PR, and lgtm refuses it once the review has moved to another revset or to commits it hasn't loaded, or the workspace's PR has moved on. Link in the Comments panel (`l`) links the send target's agent to the whole PR review: it gets a short brief with your reply rules, and the panel shows the linked terminal, a dot while its agent works, and Unlink. The thread you last picked (a row in the Comments panel, or anywhere in a thread in the review) is the one you're on: it shows a violet terminal mark, and the hook adds it to each prompt you send there, whole the first time and by name after. Talk (the ›_ button in a thread's header, or ⌘>) picks the thread and focuses the linked terminal, linking one first if none is. The agent can set takes, move threads and suggest replies on any thread, but only you mark a reply ready; a suggestion shows under the reply with Use and Dismiss and a violet dot on its row, and a reply it gives a thread that already has one comes as a suggestion. Each exchange is filed under the thread you were on when you sent it, in the thread's folded Chat row (Show Chat in a row's menu opens it). Linking another terminal, Unlink, closing the terminal or ending its session drops the link, and the old agent hears so on its next prompt. Review next to a reply's Edit sends the thread, the reply and your reply rules to the linked terminal (or the send target), and the agent suggests a better reply. On a new, untriaged thread, Triage there sends it with your reply rules, and the agent sets its take and first reply; Triage N in the Comments panel sends every new thread to the linked terminal, or the send target when none is linked. Questions it asks (like a qqueue session) open in the Q&A tab, talking one over in the terminal can rewrite it there, and Answers sends your answers back to the terminal that asked. That agent also runs the tab's steps: after an answer that others depend on, Discuss and ⓘ on an option, and asking a question again after a discussion each paste a short prompt there and press Enter, waiting while the agent is busy. A discussion reply lands in the question's card when the agent finishes. If that agent is gone, the send target's agent takes the session over. Start Q&A sends a session you write in lgtm (its prompt, plus any Read first files dropped from the Files tree or Finder, or picked with Add) to the send target, and its questions come back the same way. The Architecture tab (New Tab › Architecture, or `g` in a review) maps the workspace's whole system from its imports, as C4 context, containers, components and one component's symbols (double-click or a card's chevron to drill in; the back button, the crumbs, the depth stepper or Backspace to go back; drag to pan, scroll or pinch to zoom). Containers show their biggest components as tiles; inside one, the other containers it talks to sit at the right edge as doors, and a minimap shows where you are. Focus draws the selection's edges or the heaviest ones, Changed what the review changes, All everything. The side panel shows what a node holds and talks to, and for a component the files to start in, its most used symbols, and what it uses and is used by. Click an edge for its import sites and what it imports. Symbols (the fourth level, or Symbols on a component) shows the component's symbols by file, with the components that call in on the left and those it calls on the right, and a faint line between each pair of files that call each other. Hover a symbol or component to draw its calls; click one to collapse the canvas into it, its callers and its calls (click a neighbor to move, the background or Esc to go back). A symbol's panel shows its signature, Open, its callers and its calls. Right-click a component for Symbols, Tour, Trace To…, Trace From…, Show in Files and Ask (⌘> asks about the selection). Find (`/`) jumps to a component, file or symbol (⌘Enter opens the file). Trace (with a node selected, it starts from that node) swaps the tools for From and To fields that search every container, component, person and external system at any level (↑/↓, Enter; clicking a node fills the field being picked; ⇄ swaps the ends; Esc or × ends it). With both ends set, the path draws straight from left to right, each hop numbered with its file and line or relation; All paths lists the ones up to two hops longer, and Show on map draws the path over the map with its hops in the side panel. With a review open, the Review toggle lays the review over the map: changed components in amber with their added and removed lines, new ones in green, impacted ones dashed, new imports in green and removed ones dashed red, and a red mark on anything with Look items. A strip under the bar shows the range, the counts, Transitive and what's left to look at, and the side panel adds the changed files, Look items and the changed components inside a container. Generate (in the bar and the sidebar) lists Description and, with a review open, Assessment, each Fresh, Stale or Missing, with the stale and missing ones checked; it sends one prompt for the checked ones to the send target, and an amber dot on it means something is stale or missing. The description gives names, descriptions, people, external systems and the tiers components sit in, and the Architecture tab opens; components changed since show as stale. Until there is one, or once it's stale, a pill on the map says Not described or Description stale, with Describe. The assessment gives a score, risk, layers and items to look at, replacing the last assessment of that revset in that workspace; lgtm refuses one handed back after the review moved to other commits, and once the revset moves on it shows as Stale. Tours lists the Overview, Every feature, and each container's and component's tour, which goes from the container down to the component, who uses it, its files, its symbols and what it uses (←/→ to step). Each has Play, or Write Tour, which asks the send target to write it; right-click one for Rewrite Tour. ⌘-click a link in markdown (editor or rendered) to open it: web addresses in the browser, repo paths in an editor tab. It also adds Claude Code hooks to `~/.claude/settings.json` (they do nothing outside lgtm), so each workspace in the sidebar shows its most urgent agent: needs input, failed, stalled (no output for 3 minutes), working, done, idle, or none. Hovering that mark lists each agent with how long it's been in its state, and clicking one brings up its tab. A tab running an agent shows the same mark before its title. A terminal tab takes its title from Claude Code's session unless you rename it, and a Claude Code session still open when lgtm closes resumes (the Claude command with `--resume`) in its terminal when lgtm opens again. Any agent can publish review findings to a report, with its own suggested fixes if it has them, and checked findings or fixes go back to a terminal: the Reports panel keeps the report you describe, and Create Report sends it with instructions to publish here. Each report has an id (`R12`, or a name you give it with Rename…) that you tell the agent, and publishing again under the same reviewer name replaces that reviewer's findings.

## Commands

```sh
lgtm [repo]               # open a repo, or the repo picker
lgtm gc [--force] [repo]  # remove leftover workspace directories, forget workspaces whose directory is gone
                          # (workspaces whose repo can't be found are kept unless --force)
lgtm mcp                  # the MCP server Claude Code starts; you don't run it yourself
```

## Develop

```sh
pnpm install
just dev         # hot reload; shows the repo picker
just go          # jj git fetch, jj new main@origin, then just dev
just test        # tsc + biome, vitest, cargo test (the Rust tests run jj)
just build       # release binary and .app
just install     # just build, then copy the .app to /Applications
just run [repo]  # run the built .app (with its Dock icon)
just gc [repo]   # lgtm gc with the release build
just bump 0.2.0  # set the app version before a release
```

To release: `just bump <version>`, push, then run the Release workflow in GitHub Actions. It builds a signed, notarized universal .dmg into a draft release to publish.

Debug builds regenerate `src/bindings.ts` from the Rust commands. `cargo test` fails if it's stale; rerun with `UPDATE_BINDINGS=1` to refresh it.

## Config

App settings are in `~/.config/lgtm/settings.toml` (lgtm › Settings…, ⌘,). The Claude command (an alias like `cc` works, since the terminal's shell runs it) starts Claude Code in a new Claude tab (New Tab › Claude) and resumes sessions. lgtm also runs it to check Claude Code's version and register its MCP server. Prompt overrides go in `~/.config/lgtm/`.

Optional, in the repo at `.lgtm/config.toml`. lgtm reads it from the review's base, so the changes under review can't change it:

```toml
# Files to skip: they start collapsed and count as done.
skip = ["*.lock", "**/generated/**"]

[anchor]
window = 2       # context lines above and below a marked line
threshold = 0.5  # minimum context score for a duplicate line to match
```

An executable `.lgtm/setup` runs in the first terminal of each new workspace. lgtm shows the whole script and asks before running it the first time, and again whenever it changes; Skip opens a plain shell, and a script that changes while it asks isn't run. A script over 64 KiB, or not UTF-8 text, isn't shown or run. It never runs for a pull request's workspace.
