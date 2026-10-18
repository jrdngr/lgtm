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

/// One finding, shown as a card with its severity, title and locations, then its facts,
/// explanation and fix.
#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Finding {
    pub severity: Severity,
    /// A short headline naming the problem.
    pub title: String,
    /// Where it is. Locations in the change link to their rows in the review.
    pub locations: Vec<Location>,
    /// What the code does, as checkable facts, one per bullet.
    pub facts: Vec<String>,
    /// Why it matters.
    pub explanation: String,
    /// How to fix it.
    pub fix: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Publish {
    /// The report to publish to, by the id the human gave you, like `R12` or `auth-pass`.
    /// Leave it out to start a new report on the open review.
    pub report: Option<String>,
    /// The name of your section of the report, like `claude`. Publishing again under the same
    /// name replaces that section. Defaults to `terminal-agent`.
    pub reviewer: Option<String>,
    /// One or two sentences on the change overall.
    pub verdict: String,
    pub findings: Vec<Finding>,
    /// What the change is meant to do, in a few words. Give it with `fixes` to show your own
    /// suggested fixes.
    pub task: Option<String>,
    /// The fixes the task needs, most important first.
    pub fixes: Option<Vec<FixAction>>,
    /// Critical and high findings left out as unrelated to the task, by number.
    pub off_task: Option<Vec<u32>>,
    /// Ambiguities in the requirements that block a correct fix.
    pub questions: Option<Vec<OpenQuestion>>,
}

/// One suggested fix.
#[derive(Deserialize, JsonSchema)]
pub struct FixAction {
    /// What to do, in a sentence.
    pub action: String,
    /// The findings it closes, by their 1-based place in `findings`.
    pub closes: Vec<u32>,
}

#[derive(Deserialize, JsonSchema)]
pub struct OpenQuestion {
    pub question: String,
    /// The findings it affects, by their 1-based place in `findings`.
    pub findings: Vec<u32>,
}

#[derive(Deserialize, JsonSchema)]
pub struct GetReport {
    /// The report's id, like `R12` or `auth-pass`. Defaults to the newest report.
    pub report: Option<String>,
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    Agree,
    /// Agree, with a different fix than asked.
    Change,
    Disagree,
    /// The human has a call to make.
    Decide,
    /// The comment asks a question, and the reply answers it.
    Answer,
}

#[derive(Deserialize, JsonSchema)]
pub struct SetTake {
    /// The thread's id, from list_threads.
    pub thread_id: u32,
    pub verdict: Verdict,
    /// Why: the facts that justify the verdict, as short markdown bullets.
    pub why: String,
    /// For decide: what the human has to decide.
    pub question: Option<String>,
    /// The whole reply to post, in the human's voice. Leave it out to keep the current one.
    pub reply: Option<String>,
    /// Three to six words naming what the thread asks. Leave it out to keep the current one.
    pub summary: Option<String>,
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ThreadStage {
    New,
    /// The human has a call to make.
    Decide,
    /// Code to change.
    Fix,
    /// A reply to work out.
    Reply,
    /// The reply is final and waits to be sent.
    Ready,
    /// Sent, fixed with nothing to say, or set aside.
    Done,
}

#[derive(Deserialize, JsonSchema)]
pub struct SetStage {
    /// The thread's id, from list_threads.
    pub thread_id: u32,
    pub stage: ThreadStage,
}

#[derive(Deserialize, JsonSchema)]
pub struct ThreadId {
    /// The thread's id, from list_threads.
    pub thread_id: u32,
}

#[derive(Deserialize, JsonSchema)]
pub struct SuggestReply {
    /// The thread's id, from list_threads.
    pub thread_id: u32,
    /// The whole reply, in the human's voice.
    pub text: String,
}
