//! What a report holds: each reviewer's findings, and the suggested fixes.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

/// A place in the change: a repository-relative path, with new-side lines when known.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
pub struct Location {
    pub path: String,
    pub start: Option<u32>,
    pub end: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// `<reviewer>#<n>`, which the app assigns.
    #[serde(default)]
    pub id: String,
    pub severity: Severity,
    pub title: String,
    pub locations: Vec<Location>,
    pub facts: Vec<String>,
    pub explanation: String,
    #[serde(default)]
    pub fix: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Review {
    pub verdict: String,
    pub findings: Vec<Finding>,
}

impl Review {
    /// Reads a reviewer's review, sorted by severity, its findings numbered `<reviewer>#<n>`.
    pub fn parse(reviewer: &str, value: Value) -> anyhow::Result<Self> {
        let mut review: Review = serde_json::from_value(value)?;
        review.findings.sort_by_key(|f| f.severity);
        for (i, f) in review.findings.iter_mut().enumerate() {
            f.id = format!("{reviewer}#{}", i + 1);
        }
        Ok(review)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Question {
    pub question: String,
    pub findings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Action {
    pub action: String,
    pub closes: Vec<String>,
}

/// Suggested fixes for the change's task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Minimal {
    pub task: String,
    /// Ordered actions, each with the ids of the findings it closes.
    #[serde(default)]
    pub fixes: Vec<Action>,
    /// Critical and high findings left out as unrelated to the task.
    pub off_task: Vec<String>,
    #[serde(default)]
    pub questions: Vec<Question>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Output {
    Review(Review),
    Minimal(Minimal),
}
