//! What agents read about lgtm, printed by `lgtm review guide`.

pub const GUIDE: &str = r#"# lgtm

lgtm is the human's terminal code review app for jj. They review a change in it, mark lines
reviewed, check files, and leave numbered comments for you (C1, C2, ...). Your commands write
to the same store, and an open lgtm shows what you did at once. Lines are on the new side of
the diff unless a command says removed.

## The review

Every review command takes `-r <revset>`. Without it they use the review open in lgtm in
this workspace, else `@`. `lgtm review` prints the revset it used: pass that same `-r` to the
commands that follow, so they stay on that review if the human opens another.

- `lgtm review [-r <revset>]` prints the review as JSON: its revset and commits, and per file
  whether it's checked, how many changed rows count as done, the line ranges still left, and
  the line ranges of each mark.
- `lgtm review check [-r <revset>] [--uncheck] <path>...` checks files as reviewed, as you.
  The human sees your checks apart from theirs, and theirs stay as they are.
- `lgtm review mark [-r <revset>] <path> [<lines>] [--removed <lines>] --reviewed|--flagged|--clear`
  marks changed lines with your own marks. `<lines>` is a list like `12,20-30`; `--removed`
  takes old-side lines of removed rows. Reviewed counts as done, flagged doesn't, and
  `--clear` removes your marks there. Lines the human marked keep their marks.

## Comments

- `lgtm review comments [-r <revset>]` prints the comments as JSON: by number, the file, the
  line it's on now, that line's code, and what was written. The human's are for you; ones an
  agent left name it as author. Resolved ones come last, marked resolved: leave them alone
  unless the human asks. Then the open threads of the review's PR on GitHub (P1, P2, ...),
  once the human pulled them in lgtm, with their posts.
- `lgtm review resolve <C3>...` marks comments done once you've dealt with them. The human no
  longer sees them unless they show resolved comments.
- `lgtm review reopen <C3>...` opens resolved comments again, when the human asks.
- `lgtm review comment [-r <revset>] <path>:<line> [--removed] [--as <name>] "<markdown>"`
  leaves a comment for the human on a line of the review (with `--removed`, an old-side line
  of a removed row). Without the text it reads it from stdin. `--as` names you; it defaults
  to Claude in Claude Code, else Agent, so other agents pass it, like `--as Codex`. Use it
  only for what they must see: a likely bug, a risky change, or a question only they can
  answer. A review rarely needs more than three. No style notes, praise or summaries.

## Reviewing a change for the human

When the human asks you to review the change in lgtm, or says `lgtm flag review`: run
`lgtm review`, read the diff of every file in it, then `mark --reviewed` the lines you
checked and are sure of, and `check` the files with nothing flagged. Flag very
conservatively: `mark --flagged` only the few extremely sensitive lines the human must read
themselves (security, data loss, money, irreversible or breaking behavior), never routine
changes or style, and `comment` on every flagged line saying why. Then tell the human in a
line or two what you flagged.

## Snapshots

lgtm records the files you change for each prompt as one snapshot under the working copy,
which the human can review, undo or move to its own commit. `lgtm snap "<label>"` starts a
new snapshot named for what it holds, like `Fix the off-by-one in paging`: your edits after
it go into it, until you run it again or your turn ends. When the human asks for work in
separate snapshots, run it before each part.
"#;
