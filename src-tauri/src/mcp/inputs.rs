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
    /// Why, leading with the facts that justify the verdict.
    pub why: String,
    /// For decide: what the human has to decide.
    pub question: Option<String>,
    /// The whole reply to post, in the human's voice. Leave it out to keep the current one.
    pub reply: Option<String>,
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
pub struct QaSession {
    /// The session's id, from list_qa or qa_ask.
    pub session: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct QaChoice {
    pub label: String,
    /// What it means, in a line.
    pub means: String,
    /// What it gets you, a short line each.
    #[serde(default)]
    pub pros: Vec<String>,
    /// What it costs, a short line each.
    #[serde(default)]
    pub cons: Vec<String>,
    /// Downsides worse than a con: what could go badly wrong.
    #[serde(default)]
    pub risks: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct QaQuestion {
    /// A subquestion of the top-level question before it.
    #[serde(default)]
    pub sub: bool,
    /// A few words naming the decision.
    pub subject: String,
    /// Only the facts that change the answer, one per bullet.
    pub context: Vec<String>,
    /// The recommended option first, always.
    pub options: Vec<QaChoice>,
    /// Why option 1 is recommended, in a line or two.
    pub why: String,
    /// Files you read to build the question.
    #[serde(default)]
    pub sources: Vec<String>,
    /// Ids of questions whose answers could change this one; leave it empty when none could.
    /// Questions in this call are numbered in the order given, on from the session's last
    /// number (Q1, Q2, ... in a new session), with subquestions lettered under the question
    /// before them (Q2a, Q2b). lgtm moves each question after the ones it depends on, so
    /// the returned ids can differ.
    #[serde(default)]
    pub depends: Vec<String>,
    /// Only when option 1 is obvious and the other options are non-starters: why, in a short
    /// line. lgtm records option 1 as the answer, and the human can reopen it.
    pub auto: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct QaAsk {
    /// The session to add to, from list_qa. Leave it out to start one.
    pub session: Option<String>,
    /// For a new session: what it's deciding, in a line.
    pub title: Option<String>,
    /// For a new session: everything from this conversation that lgtm's agent can't look up
    /// in the repo, since it updates the questions after each answer without seeing this
    /// chat: the goal, what's settled and why, what was ruled out, and the human's
    /// constraints. Leave out what the repo says.
    pub brief: Option<String>,
    /// For a new session: workspace paths lgtm's agent reads before it updates questions.
    #[serde(default)]
    pub read_first: Vec<String>,
    /// In the order to ask them.
    pub questions: Vec<QaQuestion>,
}
