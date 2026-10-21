//! The prompt asking a terminal agent for a guided review, or to bring the last one up to date.

use super::Guided;

/// How the agent hands the guided review back. The editable prompt says what goes in it.
const FORMAT: &str = "Then call lgtm's set_guided_review tool with the revset, from and to, \
and the chunks, excluded parts and context docs. Ranges are repository-relative paths with \
1-based lines on the new side. Every changed line belongs to a chunk or an excluded part. Don't \
edit files. Then tell me in a line how many chunks you made at each risk.";

/// The prompt for a guided review of `revset`, from `from` to `to`, with the person's
/// instructions `rules`. With `last`, the agent updates it: same ids for the same concerns, so
/// verdicts carry over.
pub fn text(revset: &str, from: &str, to: &str, last: Option<&Guided>, rules: &str) -> String {
    let mut out = format!(
        "The change under review is `{revset}` in this jj repository, from {from} to {to}. Read \
         its diff with `jj diff --from {from} --to {to}`.\n\n{}\n\n",
        rules.trim()
    );
    if let Some(last) = last {
        out.push_str(&update(last, to));
    }
    out.push_str(FORMAT);
    out
}

fn update(last: &Guided, to: &str) -> String {
    let was = last.commits.from.as_deref().unwrap_or("root()");
    let mut out = format!(
        "## Updating the last guided review\n\nThe last guided review was of {was} to {}. \
         {}Keep a chunk's id when it's about the same code, even if the code changed, so the \
         person's verdict follows it; give new concerns new ids and leave out chunks whose code \
         is gone. Its chunks:\n\n",
        last.commits.to,
        if last.commits.to == to {
            String::new()
        } else {
            format!("See what changed since with `jj diff --from {} --to {to}`. ", last.commits.to)
        }
    );
    for c in &last.chunks {
        let ranges: Vec<String> = c
            .ranges
            .iter()
            .map(|r| format!("{}:{}-{}", r.path, r.start, r.end))
            .collect();
        out.push_str(&format!(
            "- `{}` ({:?}): {} — {}\n",
            c.id,
            c.risk,
            c.title,
            ranges.join(", ")
        ));
    }
    out.push('\n');
    out
}
