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

#[derive(Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    /// Needs a careful look; give a level.
    Important,
    /// A numbered step in a named flow through several places.
    Flow,
    /// Intent or background the diff doesn't show.
    Context,
    /// Something the reviewer or author should settle.
    Question,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct FlowStep {
    /// The flow's short name, the same on each of its steps.
    pub flow: String,
    /// From 1.
    pub step: u32,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Note {
    /// Repository-relative path of a file in the review.
    pub path: String,
    /// First line on the new side of the diff (the old side for a deleted file).
    pub start: u32,
    /// Last line, inclusive.
    pub end: u32,
    pub category: Category,
    /// For important notes: high, medium or low.
    pub level: Option<Level>,
    /// One line, under 80 characters.
    pub title: String,
    /// One to four sentences of markdown.
    pub body: String,
    /// For flow notes.
    pub flow: Option<FlowStep>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Notes {
    /// In reading order, starting with a context note on the entry point. These replace the
    /// review's current walkthrough.
    pub notes: Vec<Note>,
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

#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    Agree,
    AgreeWithChanges,
    PushBack,
    /// The human has a call to make.
    NeedsYou,
}

#[derive(Deserialize, JsonSchema)]
pub struct SetTake {
    /// The thread's id, from list_threads.
    pub thread_id: u32,
    pub verdict: Verdict,
    /// Why, leading with the facts that justify the verdict.
    pub why: String,
    /// For needsYou: what the human has to decide.
    pub question: Option<String>,
    /// The whole reply to post, in the human's voice. Leave it out to keep the current one.
    pub reply: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct ThreadId {
    /// The thread's id, from list_threads.
    pub thread_id: u32,
}

#[derive(Deserialize, JsonSchema)]
pub struct Design {
    /// The design's folder name, from list_designs.
    pub design: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct DesignChoice {
    pub label: String,
    /// 1-3 bullets: what it means, and what it costs.
    pub bullets: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct DesignQuestion {
    /// A subquestion of the top-level question before it.
    #[serde(default)]
    pub sub: bool,
    /// A few words naming the decision.
    pub subject: String,
    /// Only the facts that change the answer, one per bullet.
    pub context: Vec<String>,
    /// The recommended option first, always.
    pub options: Vec<DesignChoice>,
    /// Why option 1 is recommended, in a line or two.
    pub why: String,
    /// Files you read to build the question.
    #[serde(default)]
    pub sources: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct DesignAsk {
    /// The design's folder name. A design that doesn't exist yet is created.
    pub design: String,
    /// In the order to ask them.
    pub questions: Vec<DesignQuestion>,
}
