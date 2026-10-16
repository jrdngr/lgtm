//! Reports: review findings agents publish through the MCP server, a section per reviewer, with
//! the agent's suggested fixes when it gives them.

mod describe;
pub mod output;
mod reports;
mod submitted;
#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};
use specta::Type;

pub use describe::describe;
pub use output::Output;
pub use submitted::Summary;

/// A reviewer with a section in a report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Persona {
    pub name: String,
    pub description: String,
}

/// A reviewer's findings, or the suggested fixes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Part {
    /// The reviewer's name. Absent for suggested fixes.
    pub persona: Option<String>,
    pub output: Output,
}

/// A report on a revset, frozen at the commits it resolved to when it was first published.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BotRun {
    pub id: u32,
    /// What the user renamed it to. Its id is `R<id>` until then.
    #[serde(default)]
    pub name: Option<String>,
    pub workspace: Option<String>,
    pub revset: String,
    pub from: Option<String>,
    pub to: String,
    pub personas: Vec<Persona>,
    pub parts: Vec<Part>,
    pub created_at: f64,
    /// Ids of findings the user dismissed.
    #[serde(default)]
    pub dismissed: Vec<String>,
}
