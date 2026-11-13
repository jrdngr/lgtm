//! What agents read about an issues folder. `init` writes it to `ISSUES.md`, and `guide`
//! prints it followed by the folder's own workflow.

use crate::workflow::Workflow;

pub const GUIDE: &str = r#"# Issues

This folder is an issue tracker for agents, read by `lgtm issues`. Each issue is one file,
`<id>-<slug>.md`, in `issues/<status>/`: the folder it's in is its status. Each status folder
can hold a `STATUS.md` saying what it means and a `TEMPLATE.md` new issues filed there start
from, and `issues/workflow.md` orders the statuses and lists the header fields. Run
`lgtm issues guide` to read this tracker's workflow, and `lgtm issues` with no arguments here
to see the issues.

## Commands

Run these in this folder, or pass `-C <this folder>`. `--as <name>` sets who you are; by
default it's your lgterm tab's name.

- `lgtm issues new "<title>" [--body "<text>"] [--status <s>] [--<field> <value>]` files an
  issue and prints its id.
- `lgtm issues list [--all] [--status <s>]` lists issues not done; `--all` adds done ones and
  `--status` shows one status.
- `lgtm issues show <id>` prints an issue's file.
- `lgtm issues statuses` lists the statuses in order.
- `lgtm issues claim <id>` makes it yours and moves it to the claim status when it's in one
  before that. A blocked issue is refused; don't start it.
- `lgtm issues move <id> <status> ["<note>"]` moves it to a status. A status can require
  fields, or no open blockers, before an issue moves in; the refusal says what's missing.
  `--force` moves it anyway, on `claim` and the other verbs too: only when the user says so.
- `lgtm issues log <id> "<text>"` adds a note: progress, findings, what you changed.
- `lgtm issues set <id> <field> [value]` sets a header field; no value clears it. The
  workflow lists the fields this tracker uses. These are built in:
  - `revision`: where the code you wrote for it is: the jj change id, or in a git repo the
    branch, else the commit. Set it once you've written code, and again if the code moves.
  - `blocked-by`: the ids of issues that must be done first, like `3 7`. `list` shows which
    blockers are still open.
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
- `lgtm issues close <id> ["<note>"]` moves it to the close status, `drop` to the drop status
  and `reopen` to the reopen status, as the workflow names them.

## File format

```
---
title: Login redirect loops after session expiry
severity: <a field the workflow lists, like severity>
owner: <tab name of the agent working on it>
term: <its lgterm terminal, like t12>
filed-by: <who filed it>
revision: <jj change id, git branch or commit with its code>
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

/// The guide, then the tracker's workflow and what each status means.
pub fn full(wf: &Workflow) -> String {
    let mut out = format!("{GUIDE}\n# This tracker\n\n");
    if !wf.body.is_empty() {
        out += &format!("{}\n\n", wf.body);
    }
    let fields: Vec<String> = wf
        .fields
        .iter()
        .map(|f| match &f.kind {
            crate::fields::Kind::Choice(cs) => {
                let values: Vec<&str> = cs.iter().map(|c| c.value.as_str()).collect();
                format!("`{}` ({})", f.name, values.join(", "))
            }
            _ => format!("`{}`", f.name),
        })
        .collect();
    out += &format!("Fields: {}.\n\n## Statuses\n", fields.join(", "));
    for s in &wf.statuses {
        out += &format!("\n### {} (`{}`)\n\n", s.label, s.id);
        let mut facts = Vec::new();
        if s.done {
            facts.push("Counts as done.".to_owned());
        }
        if !s.requires.is_empty() {
            facts.push(format!("Requires {} to move in.", s.requires.join(", ")));
        }
        if !facts.is_empty() {
            out += &format!("{}\n\n", facts.join(" "));
        }
        if !s.body.is_empty() {
            out += &format!("{}\n", s.body);
        }
    }
    out
}
