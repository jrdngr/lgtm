# lgtm

Desktop code review for agentic development, built on jj.

This software is made for my personal use. If you don't want to use jj, gh, or Claude Code, fork it. You're welcome to file issues and feature requests, but I may or may not ever look at them. Please don't open pull requests; I already spend too much time reading them. You're better off filing an issue and hoping I tell Claude to read my issues.

This program may or may not be safe to run. Use it at your own risk.

lgtm runs on macOS. Help › Keyboard Shortcuts lists every shortcut.

## Requirements

- [jj](https://github.com/jj-vcs/jj) 0.46 or newer: `brew install jj`
- [Claude Code](https://claude.com/claude-code), for the agents: `curl -fsSL https://claude.ai/install.sh | bash`
- [gh](https://cli.github.com), signed in, for pull requests: `brew install gh && gh auth login`

lgtm checks for these at startup and says what's missing. Only jj is required.

To build it you also need Rust, Node 22, pnpm, [just](https://github.com/casey/just), and the Xcode command line tools.

## Install

```sh
git clone https://github.com/jrdngr/lgtm
cd lgtm
pnpm install
just build
cp -R src-tauri/target/release/bundle/macos/lgtm.app /Applications/
```

The app's binary takes a repo path: `lgtm /path/to/repo`. A second launch for the same repo focuses its window. Plain git repos get an offer to set up jj alongside git.

On first run, lgtm offers to register itself as an MCP server with Claude Code, so `claude` in an lgtm terminal can read and comment on the open review.

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
just run [repo]  # run the release build
just gc [repo]   # lgtm gc with the release build
```

Debug builds regenerate `src/bindings.ts` from the Rust commands. `cargo test` fails if it's stale; rerun with `UPDATE_BINDINGS=1` to refresh it.

## Config

App settings are in `~/.config/lgtm/settings.toml` (lgtm › Settings…, ⌘,). Reviewer personas and prompt overrides go in `~/.config/lgtm/`.

Optional, in the repo at `.lgtm/config.toml`:

```toml
# Files to skip: they start collapsed and count as done.
skip = ["*.lock", "**/generated/**"]

[anchor]
window = 2       # context lines above and below a marked line
threshold = 0.5  # minimum context score for a duplicate line to match
```

An executable `.lgtm/setup` runs in the first terminal of each new workspace. It never runs for a pull request's workspace.
