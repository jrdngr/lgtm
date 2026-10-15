//! What each step's agent returns. `schema` has the JSON schemas that make it return that.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;

use super::Step;

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

/// A persona that raised a merged finding, with its own rating.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Raiser {
    pub persona: String,
    pub severity: Severity,
    pub in_lens: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// `<persona>#<n>` for a persona's finding, which the app assigns, and the synthesis's own
    /// number for a merged one.
    #[serde(default)]
    pub id: String,
    pub severity: Severity,
    pub title: String,
    pub locations: Vec<Location>,
    pub facts: Vec<String>,
    pub explanation: String,
    /// Absent for off-lens findings.
    #[serde(default)]
    pub fix: Option<String>,
    #[serde(default)]
    pub off_lens: bool,
    /// Merged findings only: the persona findings this one combines.
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub raised_by: Vec<Raiser>,
    /// How the personas disagreed, and the ruling.
    #[serde(default)]
    pub contested: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Review {
    pub verdict: String,
    pub findings: Vec<Finding>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Challenge {
    pub finding: String,
    pub argument: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Stance {
    Concede,
    Defend,
    PartlyConcede,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Response {
    pub finding: String,
    pub stance: Stance,
    pub evidence: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    Kept,
    Dropped,
    Downgraded,
    Upgraded,
}

/// The ruling on a challenged persona finding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Ruling {
    /// The persona finding's id.
    pub finding: String,
    pub outcome: Outcome,
    pub severity: Option<Severity>,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct QuickWin {
    pub finding: String,
    pub fix: String,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Synthesis {
    pub verdict: String,
    pub findings: Vec<Finding>,
    #[serde(default)]
    pub rulings: Vec<Ruling>,
    pub quick_wins: Vec<QuickWin>,
    pub questions: Vec<Question>,
    pub fix_first: Vec<Action>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Minimal {
    pub task: String,
    /// Synthesis finding ids that matter to the task.
    pub kept: Vec<String>,
    /// Critical and high findings left out as unrelated to the task.
    pub off_task: Vec<String>,
    pub recommendations: String,
}

/// A finished part's result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Output {
    Review(Review),
    Challenges { challenges: Vec<Challenge> },
    Rebuttals { responses: Vec<Response> },
    Synthesis(Synthesis),
    Minimal(Minimal),
}

impl Output {
    /// Reads `step`'s structured result. A persona's findings get ids from its name.
    pub fn parse(step: Step, persona: Option<&str>, value: Value) -> anyhow::Result<Self> {
        Ok(match step {
            Step::Reports => {
                let mut review: Review = serde_json::from_value(value)?;
                review.findings.sort_by_key(|f| f.severity);
                for (i, f) in review.findings.iter_mut().enumerate() {
                    f.id = format!("{}#{}", persona.unwrap_or("persona"), i + 1);
                }
                Output::Review(review)
            }
            Step::Challenges => {
                serde_json::from_value(value).map(|c: Challenges| Output::Challenges {
                    challenges: c.challenges,
                })?
            }
            Step::Rebuttals => {
                serde_json::from_value(value).map(|r: Rebuttals| Output::Rebuttals {
                    responses: r.responses,
                })?
            }
            Step::Synthesis => Output::Synthesis(serde_json::from_value(value)?),
            Step::Minimal => Output::Minimal(serde_json::from_value(value)?),
        })
    }
}

#[derive(Deserialize)]
struct Challenges {
    challenges: Vec<Challenge>,
}

#[derive(Deserialize)]
struct Rebuttals {
    responses: Vec<Response>,
}
