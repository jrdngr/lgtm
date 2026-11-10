//! What agents send to change a review, and how the tools that take it read to them. The
//! desktop and terminal apps serve the same tools.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::store::MarkColor;

pub const GET_REVIEW: &str = "The open review as the human has reviewed it so far: its \
    revset and commits, whether the human marks lines (marks), and per file whether it's \
    checked or changed since it was checked, how many changed rows count as done, the line \
    ranges still left, and, with marks in use, the line ranges of each mark (by mark label).";

/// How `get_comments` reads, for an app to add what else it lists.
#[macro_export]
macro_rules! get_comments_doc {
    () => {
        "The comments on the code, by number (C1, C2, ...): the file, the line it's on now, \
        that line's code, and what was written. The human's are for you; ones an agent left \
        with add_comment name it as author. Resolved ones come last, marked resolved: leave \
        them alone unless the human asks."
    };
}

pub const RESOLVE_COMMENT: &str = "Mark a comment done once you've dealt with it. The human \
    no longer sees it unless they show resolved comments.";

pub const UNRESOLVE_COMMENT: &str = "Open a resolved comment again, when the human asks.";

/// What the server's instructions say about comments.
pub const COMMENTS_INSTRUCTIONS: &str = "The human leaves numbered comments for you (C1, C2, \
    ...). Read them with get_comments, and once you've dealt with one, mark it done with \
    resolve_comment. get_comments lists resolved comments too, marked resolved: leave those \
    alone unless the human asks about them, and reopen one with unresolve_comment when they do.";

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ResolveComment {
    /// The comment's number: 3 for C3.
    pub number: i64,
}

pub const CHECK_FILES: &str = "Check or uncheck files in the open review as reviewed, as \
    yourself: the human sees your checks apart from theirs. Their own checks stay as they are.";

pub const MARK_LINES: &str = "Mark changed lines of a file in the open review with your own \
    marks: reviewed (counts as done) or flagged (doesn't). The human sees your marks apart \
    from theirs, and lines they marked keep their marks. Leave mark out to clear yours. Flag \
    only extremely sensitive lines, each with a comment from add_comment saying why.";

pub const ADD_COMMENT: &str = "Leave a comment for the human on a line of the open review. \
    Use it only for what they must see: a likely bug, a risky change, or a question only they \
    can answer. A review rarely needs more than three. No style notes, praise or summaries.";

/// What the server's instructions say about reviewing a change for the human.
pub const REVIEW_INSTRUCTIONS: &str = "When the human asks you to review the change in lgtm, \
    or says `lgtm flag review`: call get_review, read the diff of every file in it, then \
    mark_lines reviewed for lines you checked and are sure of, and check_files for files with \
    nothing flagged. Flag very conservatively: mark_lines flagged only for the few extremely \
    sensitive lines the human must read themselves (security, data loss, money, irreversible \
    or breaking behavior), never for routine changes or style, and add_comment on every \
    flagged line saying why. Then tell the human in a line or two what you flagged.";

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddComment {
    /// The review's revset, as get_review gives it. lgtm refuses the call once its review
    /// shows another.
    pub revset: String,
    pub path: String,
    /// A new-side line number, of a line in the file's diff.
    #[serde(default)]
    pub line: Option<u32>,
    /// Or an old-side line number of a removed line.
    #[serde(default)]
    pub removed_line: Option<u32>,
    /// Markdown.
    pub body: String,
    /// Who wrote it, filled in by lgtm from the MCP client.
    #[serde(default)]
    #[schemars(skip)]
    pub author: Option<String>,
}

/// The name an agent goes by in the review, from its MCP client's name.
pub fn agent_name(client: &str) -> String {
    let client = client.to_lowercase();
    if client.contains("claude") {
        "Claude".into()
    } else if client.contains("codex") {
        "Codex".into()
    } else if client
        .split(|c: char| !c.is_alphanumeric())
        .any(|w| w == "pi")
    {
        "Pi".into()
    } else {
        "Agent".into()
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CheckFiles {
    /// The review's revset, as get_review gives it. lgtm refuses the call once its review
    /// shows another.
    pub revset: String,
    /// The files' paths, as get_review gives them.
    pub paths: Vec<String>,
    /// False unchecks them.
    pub checked: bool,
}

/// An agent's mark: reviewed rows count as done, flagged ones don't.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum AgentMark {
    Reviewed,
    Flagged,
}

impl AgentMark {
    pub fn color(self) -> MarkColor {
        match self {
            AgentMark::Reviewed => MarkColor::AgentReviewed,
            AgentMark::Flagged => MarkColor::AgentFlagged,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarkLines {
    /// The review's revset, as get_review gives it. lgtm refuses the call once its review
    /// shows another.
    pub revset: String,
    pub path: String,
    /// New-side line numbers or ranges, like `12` or `12-20`.
    #[serde(default)]
    pub lines: Vec<String>,
    /// Old-side line numbers or ranges of removed lines.
    #[serde(default)]
    pub removed_lines: Vec<String>,
    /// The mark to set. Leave it out to clear your marks on those lines.
    pub mark: Option<AgentMark>,
}

#[cfg(test)]
mod tests {
    use super::agent_name;

    #[test]
    fn agents_go_by_their_clients_names() {
        assert_eq!(agent_name("claude-code"), "Claude");
        assert_eq!(agent_name("codex-mcp-client"), "Codex");
        assert_eq!(agent_name("pi"), "Pi");
        assert_eq!(agent_name("pipeline"), "Agent");
    }
}
