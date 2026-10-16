# Tech stack evaluation

## Recommendation

Use **Tauri v2**: a Rust backend and a TypeScript UI in the system webview.

- Rust owns everything stateful or CPU-bound: the jj adapter, diff parsing, mark anchoring and matching, tree-sitter, file watching, PTYs, persistence, the GitHub client, agent processes, and the MCP server.
- The webview owns rendering. The hard UI pieces (terminal, diff/editor, docking) already exist as mature web libraries. Rebuilding them natively is most of the cost of a native Rust GUI.
- v1 is macOS only, where WKWebView is solid. Linux (WebKitGTK) is the weak platform later; Electron is the fallback if it can't be made to work.

## What the spec makes hard

| Requirement | What it needs |
|---|---|
| Terminal panes, arbitrary splits | A full VT emulator, PTY spawning, fast output streaming |
| Review diff view | Custom gutters, row tints, selection units, collapsible hunks, gap bars, inline comment blocks |
| Editor | Syntax highlighting, tree-sitter symbols |
| Dockable tabs, per-repo layouts | Drag-to-split docking with serializable state |
| jj | Spawning the CLI, one writer per workspace, debounced snapshots |
| Live updates | Watching `.jj/repo/op_heads/heads/` and working copies |
| Agents | Spawning agent CLIs headless, streaming output, an MCP server |
| Anchoring | Hashing, line diffs, scoring across many files: cheap in any language |

## Options

### Electron + TypeScript

- Terminal: xterm.js + node-pty. Same pair VS Code uses.
- Diff/editor: CodeMirror 6 (decorations, gutters, block widgets for inline comments) or Monaco.
- Docking: dockview, FlexLayout, or golden-layout.
- Tree-sitter: web-tree-sitter (WASM).
- Agents: the Claude Agent SDK is TypeScript, so it runs in-process.
- The spec already assumes a TS backend (Zod, `parse-git-diff`).
- Costs: ~150 MB+ install, ~200 MB+ idle memory, Chromium security patch cadence, and no Rust.

### Tauri v2 (recommended)

- UI: the same libraries as Electron (xterm.js, CodeMirror 6, dockview).
- Backend crates: `portable-pty` (from wezterm), `notify`, `tree-sitter` (native, faster than WASM), `rusqlite`, the `gh` CLI for GitHub, `rmcp` for MCP, `serde` instead of Zod.
- IPC: Tauri channels stream PTY output and events to the webview. Commands are typed end to end with `tauri-specta` or `ts-rs`.
- Agents: spawn `claude -p --output-format stream-json` as a child process, or paste into a terminal pane running `claude` (subscription users). Other agent CLIs plug in behind the same adapter. No in-process Agent SDK.
- Small binaries, low idle memory.
- Costs: three different webviews. WKWebView on macOS is fine. WebView2 on Windows is Chromium, so it's fine. WebKitGTK on Linux is the weak one: slower, and xterm.js's WebGL renderer is unreliable there (the DOM renderer works). The UI is TS either way, so a tauri-specta boundary keeps an Electron fallback open.

### Native Rust GUI

| Framework | Terminal | Editor/diff | Docking | State |
|---|---|---|---|---|
| GPUI (Zed) + gpui-component | `alacritty_terminal`, as Zed does | gpui-component has an editor; Zed's own editor isn't reusable | gpui-component dock | Pre-1.0, few docs, API follows Zed's needs |
| Iced | `iced_term` | `text_editor` widget, basic | built-in `pane_grid` | 0.x, breaking releases |
| egui | `egui_term` | weak for code | `egui_dock` | Immediate mode is a poor fit for large diffs with rich rows |
| Slint, Freya, Xilem | none mature | none mature | none | Too early |

- Every review-specific surface is custom work: tinted rows, selection levels, gap bars, inline comment blocks with markdown, graph views.
- Best performance and a single language. GPUI is the only option that has shipped an app like this (Zed).
- Worth it only if native feel matters more than shipping the review features. It would roughly double M1 and M3.

### Rust core inside Electron

- Ship the Rust core as a sidecar process or a napi-rs addon.
- Gives Chromium everywhere plus Rust logic, at the cost of Electron's footprint and two runtimes to package. A fallback, not a starting point.

## Scorecard

| | Electron | Tauri v2 | Native (GPUI) |
|---|---|---|---|
| Terminal | best | best on mac/Windows, weaker on Linux | good (alacritty) |
| Diff/review UI | fast to build | fast to build | slow, all custom |
| Docking | off the shelf | off the shelf | gpui-component, young |
| jj/PTY/watch backend | good | best | best |
| Footprint | heavy | light | lightest |
| Rust | no | backend | all |
| Cross-platform consistency | best | Linux is the risk | good |
| Time to a usable M1 | shortest | short | long |

## Decisions inside the Tauri choice

- jj stays behind the CLI, as the spec says. `jj-lib` would give in-process revsets and diffs, but its API changes every release and it doesn't apply the user's jj config the same way. Revisit if CLI spawn latency shows up in profiles.
- Diff parsing moves to Rust. `jj diff --git` output is simple to parse, and the anchoring engine needs the hunks in Rust anyway.
- Persistence is SQLite per repo (marks, comments, sessions, layout).
See the UI framework section below.

## UI framework

### SolidJS

Pros:
- Fine-grained signals. Marking a row updates that row's DOM, not a component subtree. Fits thousands of rows changing tint, filter, and selection state.
- No virtual DOM and no re-render model to fight, so no `memo`/`useCallback` tuning in the diff view.
- Small runtime, fast startup.
- Plain DOM refs make wrapping imperative libraries (xterm.js, CodeMirror) simple.

Cons:
- Smaller ecosystem. dockview has React, Vue, and Angular bindings but not Solid, so Solid would wrap `dockview-core` by hand.
- Fewer component libraries (menus, popovers, command palette); expect to build or adapt more.
- Props can't be destructured without losing reactivity. Easy mistake for people and agents used to React.
- Agents write React far more fluently, so generated Solid code needs more review.

### React

Pros:
- Largest ecosystem: dockview, Radix/shadcn, cmdk, react-markdown, graph libraries (React Flow) all work directly.
- Agents and contributors know it best.
- Mature dev tools and profiler.

Cons:
- Re-render model needs care in the diff view: memoized rows, external stores (Zustand or `useSyncExternalStore`), and virtualization, or marking lags on big diffs.
- Bigger runtime, though it doesn't matter much in a desktop app.
- Wrapping imperative editors and terminals needs effect and ref plumbing.

### Lean

React. The diff view is the one place Solid clearly wins, and that view is mostly CodeMirror, which manages its own DOM and updates either way. React's ecosystem (dockview, React Flow for the graph views, menu and palette components) saves more time than Solid's rendering model.

## Risks to check in M0

1. Rendering a 5,000-line diff with tints and gutter marks in CodeMirror 6 while scrolling.
2. PTY output throughput over Tauri channels (`cat` a large file, run `yes` briefly).
3. Keybinding conflicts between webview defaults, xterm.js, and the app's single-letter keys.

Library versions and status are from my knowledge as of mid-2026. Confirm them when M0 starts.
