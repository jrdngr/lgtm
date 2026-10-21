//! A chunk's summary: what the agent tells the person before they read the code, in sections
//! that are each left empty when they don't apply.

use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(default)]
pub struct ChunkSummary {
    /// Markdown: why a new function, command, type or system exists. Empty for a change to
    /// existing code that needs no why.
    pub purpose: String,
    /// The questions a reviewer would ask first, each with a short answer.
    pub context: Vec<Qa>,
    /// The choices behind this implementation, a line of markdown each.
    pub decisions: Vec<String>,
    /// What users or callers see differently.
    pub behavior: Vec<BehaviorChange>,
    /// New, changed or removed signatures, commands, flags, MCP tools, schemas or config.
    pub interface: Vec<InterfaceChange>,
    /// Effects on stored or persisted data: migrations, formats, state; a line of markdown each.
    pub data: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(default)]
pub struct Qa {
    pub question: String,
    /// Markdown, a sentence or two.
    pub answer: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(default)]
pub struct BehaviorChange {
    /// Markdown; empty for new behavior.
    pub before: String,
    /// Markdown; empty for removed behavior.
    pub after: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(default)]
pub struct InterfaceChange {
    /// What it is, like a function name or `lgtm mcp --socket`.
    pub name: String,
    /// The old signature or shape as code; empty when it's new.
    pub before: String,
    /// The new one as code; empty when it's removed.
    pub after: String,
}

impl ChunkSummary {
    /// The summary with text trimmed and empty entries dropped.
    pub fn cleaned(self) -> Self {
        let lines = |v: Vec<String>| -> Vec<String> {
            v.into_iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        };
        Self {
            purpose: self.purpose.trim().to_string(),
            context: self
                .context
                .into_iter()
                .map(|q| Qa {
                    question: q.question.trim().to_string(),
                    answer: q.answer.trim().to_string(),
                })
                .filter(|q| !q.question.is_empty() && !q.answer.is_empty())
                .collect(),
            decisions: lines(self.decisions),
            behavior: self
                .behavior
                .into_iter()
                .map(|b| BehaviorChange {
                    before: b.before.trim().to_string(),
                    after: b.after.trim().to_string(),
                })
                .filter(|b| !b.before.is_empty() || !b.after.is_empty())
                .collect(),
            interface: self
                .interface
                .into_iter()
                .map(|i| InterfaceChange {
                    name: i.name.trim().to_string(),
                    before: i.before.trim_end().to_string(),
                    after: i.after.trim_end().to_string(),
                })
                .filter(|i| !i.before.trim().is_empty() || !i.after.trim().is_empty())
                .collect(),
            data: lines(self.data),
        }
    }
}
