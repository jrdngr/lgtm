//! A change as a headless agent's prompt describes it.

use crate::jj::Range;
use crate::workspace::Workspace;

/// Diffs longer than this are cut short, and the agents read the rest from the files.
const MAX_DIFF: usize = 200_000;

/// Describes the change from `from` to `to` for an agent: its commits, stat and diff.
pub async fn describe(
    ws: &Workspace,
    revset: &str,
    from: Option<String>,
    to: String,
) -> anyhow::Result<String> {
    let short = |c: &str| c.chars().take(8).collect::<String>();
    let span = match &from {
        Some(from) => format!("from commit {} to {}", short(from), short(&to)),
        None => format!("up to commit {}", short(&to)),
    };
    let range = Range { from, to };
    let (commits, stat, diff) = tokio::try_join!(
        ws.jj.descriptions(&range),
        ws.jj.diff_stat(&range),
        ws.jj.git_diff(&range),
    )?;
    Ok(context(revset, &span, &commits, &stat, &diff))
}

fn context(revset: &str, span: &str, commits: &str, stat: &str, diff: &str) -> String {
    let diff = if diff.len() > MAX_DIFF {
        let cut = diff[..diff.floor_char_boundary(MAX_DIFF)]
            .rfind('\n')
            .unwrap_or(0);
        format!(
            "{}\n(The diff is cut short here. Read the changed files for the rest.)",
            &diff[..cut]
        )
    } else {
        diff.to_string()
    };
    let fence = fence(&diff);
    format!(
        "The change under review is `{revset}` in this jj repository, {span}. Files you read \
         are the working copy, which may differ from those commits; where they disagree, the \
         diff is the change.\n\n\
         Its commits, oldest first:\n\n{commits}\n\nFiles changed:\n\n{stat}\n\n\
         The diff:\n\n{fence}diff\n{diff}\n{fence}"
    )
}

/// A code fence longer than any run of backticks in `text`.
fn fence(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    "`".repeat(longest.max(2) + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fences_outrun_backticks_in_the_diff() {
        assert_eq!(fence("plain"), "```");
        assert_eq!(fence("a ```` b"), "`````");
    }

    #[test]
    fn long_diffs_are_cut_at_a_line() {
        let diff = "+line\n".repeat(MAX_DIFF / 6 + 10);
        let ctx = context("@", "up to x", "", "", &diff);
        assert!(ctx.contains("+line\n(The diff is cut short here."));
        assert!(ctx.len() < MAX_DIFF + 1_000);
    }
}
