//! Tool parameters, with the JSON schemas agents see.

use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, JsonSchema)]
pub struct Show {
    /// Workspace-relative path of a file.
    pub path: String,
    /// A line in the working copy, or on the new side of the diff.
    pub line: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Revset {
    /// A jj revset, like `@`, `@-::@` or `trunk()..@`.
    pub revset: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct AddComment {
    /// Repository-relative path of a file in the review.
    pub path: String,
    /// First line on the new side of the diff (the old side for a deleted file).
    pub start_line: u32,
    /// Last line, inclusive.
    pub end_line: u32,
    /// The comment, in markdown.
    pub body: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct Reply {
    /// The comment's id, from list_comments.
    pub comment_id: u32,
    pub text: String,
}

#[derive(Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    High,
    Medium,
    Low,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Hotspot {
    /// Repository-relative path of a file in the review.
    pub path: String,
    /// First line on the new side of the diff (the old side for a deleted file).
    pub start: u32,
    /// Last line, inclusive.
    pub end: u32,
    pub level: Level,
    /// One line saying what to check.
    pub reason: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct Hotspots {
    /// At most 15, tight ranges. These replace the review's current hotspots.
    pub hotspots: Vec<Hotspot>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Location {
    /// Repository-relative path.
    pub path: String,
    /// First line on the new side, when it's about specific lines.
    pub start: Option<u32>,
    pub end: Option<u32>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Finding {
    pub severity: Severity,
    pub title: String,
    pub locations: Vec<Location>,
    /// What the code does, checkable facts.
    pub facts: Vec<String>,
    /// Why it matters.
    pub explanation: String,
    /// How to fix it.
    pub fix: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Findings {
    /// The name the findings show under, like `claude`. Defaults to `terminal-agent`.
    pub reviewer: Option<String>,
    /// One or two sentences on the change overall.
    pub verdict: String,
    pub findings: Vec<Finding>,
}
