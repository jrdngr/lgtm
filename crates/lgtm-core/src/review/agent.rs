//! What `lgtm agent review` passes to change a review.

use crate::store::MarkColor;

/// A comment an agent leaves.
#[derive(Debug)]
pub struct AddComment {
    pub path: String,
    /// A new-side line number, of a line in the file's diff.
    pub line: Option<u32>,
    /// Or an old-side line number of a removed line.
    pub removed_line: Option<u32>,
    /// Markdown.
    pub body: String,
    /// Who wrote it.
    pub author: String,
}

/// The name an agent goes by in the review: `named`, else the agent whose environment this
/// runs in.
pub fn author(named: Option<&str>, claude_code: bool) -> String {
    match named.map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) => n.to_string(),
        None if claude_code => "Claude".into(),
        None => "Agent".into(),
    }
}

#[derive(Debug)]
pub struct CheckFiles {
    /// The files' paths, as the review gives them.
    pub paths: Vec<String>,
    /// False unchecks them.
    pub checked: bool,
}

/// An agent's mark: reviewed rows count as done, flagged ones don't.
#[derive(Clone, Copy, Debug)]
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

#[derive(Debug)]
pub struct MarkLines {
    pub path: String,
    /// New-side line numbers or ranges, like `12` or `12-20`.
    pub lines: Vec<String>,
    /// Old-side line numbers or ranges of removed lines.
    pub removed_lines: Vec<String>,
    /// The mark to set, or `None` to clear the agent's marks on those lines.
    pub mark: Option<AgentMark>,
}

#[cfg(test)]
mod tests {
    use super::author;

    #[test]
    fn a_given_name_beats_the_environment() {
        assert_eq!(author(Some("Codex"), true), "Codex");
        assert_eq!(author(Some("  "), true), "Claude");
        assert_eq!(author(None, false), "Agent");
    }
}
