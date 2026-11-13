//! What agents read about an issues folder. `init` writes it to `ISSUES.md`.

pub const GUIDE: &str = r#"# Issues

This folder is an issue tracker for agents, read by `lgtm issues`. Each issue is one file,
`<id>-<slug>.md`. Run `lgtm issues` with no arguments here to see them.

## Commands

Run these in this folder, or pass `-C <this folder>`. `--as <name>` sets who you are; by
default it's your lgterm tab's name.

- `lgtm issues new "<title>" [--body "<text>"] [--severity <s>] [--blocked-by <ids>]` files an
  issue and prints its id.
- `lgtm issues list [--all]` lists open issues; `--all` adds done and dropped ones.
- `lgtm issues show <id>` prints an issue's file.
- `lgtm issues claim <id>` makes it yours and marks it working.
- `lgtm issues log <id> "<text>"` adds a note: progress, findings, what you changed.
- `lgtm issues set <id> <field> [value]` sets a header field; no value clears it:
  - `severity`: low, medium, high or critical.
  - `revision`: the jj change id holding the code you wrote for it. Set it once you've written
    code, and again if the code moves to another change.
  - `blocked-by`: the ids of issues that must be done first, like `3 7`. Don't start a blocked
    issue; `list` shows which blockers are still open.
  - `duplicate-of`: the issue this one repeats. It drops this one.
- `lgtm issues ask <id> <Qn> [--after <Qm>] < question.md` asks the user. Write the question
  in the qq shape: the last decision, `Question N of M`, `Qn — <subject>`, `Context:` bullets,
  `Options:` numbered with the recommendation first, each with `Pro:`/`Con:`/`Risk:` bullets,
  and a `Recommendation:` line. `--after` holds it until `Qm` is answered. End your turn after
  asking: answers arrive as messages in your terminal.
- An answer picks an option, with notes or not. A reply instead says none of the options fit:
  the question is closed, so rethink it with what the reply says and ask a new one if needed.
- `lgtm issues answer <id> <Qn> [--pick <n>] ["<text>"]` answers for the user: `--pick` with
  notes, or a reply without it.
- `lgtm issues close <id> ["<note>"]` marks it done. `drop` marks it dropped.

## File format

```
---
title: Login redirect loops after session expiry
status: open | working | needs-input | done | dropped
severity: low | medium | high | critical
owner: <tab name of the agent working on it>
term: <its lgterm terminal, like t12>
filed-by: <who filed it>
revision: <jj change id with its code>
blocked-by: 3, 7
duplicate-of: <issue id>
created: 2026-11-12T22:40Z
---

What's wrong, how to reproduce it, what done looks like.

## 2026-11-12T22:52Z auth-fixer
A note.

## 2026-11-12T23:03Z auth-fixer question Q1
Question 1 of 2
Q1 — Where an expired session goes
...

## 2026-11-12T23:03Z auth-fixer question Q2 after Q1
...

## 2026-11-12T23:06Z jordan answer Q1
1
keep the next param short

## 2026-11-12T23:08Z jordan reply Q2
Neither. Return 401 and let the client show the login sheet.
```

Times are UTC. Prefer the commands to editing files by hand, so the header and log stay valid.
"#;
