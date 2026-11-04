//! What agents send to change a review, and how the tools that take it read to them. The
//! desktop and terminal apps serve the same tools.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::store::MarkColor;

pub const GET_REVIEW: &str = "The open review as the human has reviewed it so far: its \
    revset and commits, whether the human marks lines (marks), and per file whether it's \
    checked or changed since it was checked, how many changed rows count as done, the line \
    ranges still left, and, with marks in use, the line ranges of each mark (by mark label).";

pub const CHECK_FILES: &str = "Check or uncheck files in the open review as reviewed, as \
    yourself: the human sees your checks apart from theirs. Their own checks stay as they are.";

pub const MARK_LINES: &str = "Mark changed lines of a file in the open review with your own \
    marks: reviewed (counts as done) or flagged (doesn't). The human sees your marks apart \
    from theirs, and lines they marked keep their marks. Leave mark out to clear yours. Marks \
    show only while the human uses marks (get_review's marks).";

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
