# lgtm

Desktop code review for agentic development, built on jj. See `docs/` for the spec, stack, and plan.

## Develop

Needs Rust, Node 22, pnpm, and jj.

```sh
pnpm install
pnpm tauri dev   # shows the repo picker
pnpm check       # tsc + biome
pnpm test        # cargo test
```

Debug builds regenerate `src/bindings.ts` from the Rust commands. `cargo test` fails if it's stale; rerun with `UPDATE_BINDINGS=1` to refresh it.

A built app takes a repo path as its argument: `lgtm /path/to/repo`. A second launch for the same repo focuses its window.

## Shortcuts

| Keys | Action |
|---|---|
| ⌘T | New terminal tab |
| ⌘D | Split terminal right |
| ⌘⇧D | Split terminal down |

## M0 spikes

The toolbar opens two spike tabs. Numbers to report from macOS:

- The large diff spike renders 5,000 rows in CodeMirror with tints and a mark gutter. Scroll fast and read "worst frame while scrolling" (16.7 ms is 60 fps). Press ⌘A then space and read "last mark".
- The PTY throughput spike streams about 43 MiB through a PTY into xterm.js and reports IPC and render rates.
