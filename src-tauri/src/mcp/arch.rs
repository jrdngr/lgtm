//! The architecture tool: storing an agent's assessment of the review's architecture change.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::{Deserialize, Serialize};
use tauri_specta::Event as _;

use super::server::{Lgtm, Out, failed};
use crate::events::ArchChanged;

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    Minimal,
    Low,
    Moderate,
    High,
    Critical,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Boundary {
    pub name: String,
    /// Component ids inside it.
    pub components: Vec<String>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Range {
    /// Repository-relative path.
    pub path: String,
    /// First line on the new side, 1-based.
    pub start: u32,
    /// Last line, inclusive.
    pub end: u32,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Layer {
    pub title: String,
    /// A sentence or two.
    pub summary: String,
    /// Component ids it touches.
    pub components: Vec<String>,
    pub ranges: Vec<Range>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// Structure: layering, coupling, cohesion, trust boundaries.
    Agent,
    /// A way untrusted input reaches something sensitive or crosses a trust boundary unchecked.
    Security,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Low,
    Medium,
    High,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Look {
    pub source: Source,
    pub severity: Severity,
    pub title: String,
    /// For security items, the attack path and its sites as path:line.
    pub detail: Option<String>,
    /// A component id.
    pub component: Option<String>,
    /// An edge's importing component id, with `to`.
    pub from: Option<String>,
    /// An edge's imported component id, with `from`.
    pub to: Option<String>,
    /// Repository-relative path.
    pub path: Option<String>,
    /// A line in `path` on the new side.
    pub line: Option<u32>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Assessment {
    /// The revset the prompt named.
    pub revset: String,
    /// 1 to 5: 5 is safe to merge as is, 1 needs rework.
    pub score: u8,
    pub risk: Risk,
    /// One line on why.
    pub reason: String,
    pub boundaries: Vec<Boundary>,
    /// The change as a stack in reading order, starting from what the rest builds on.
    pub layers: Vec<Layer>,
    /// What a reviewer should scrutinize, most severe first.
    pub look: Vec<Look>,
}

#[tool_router(router = arch_router, vis = "pub(super)")]
impl Lgtm {
    #[tool(
        description = "Store your assessment of the review's architecture change in lgtm, \
        replacing the last one. Its Architecture tab shows the score, risk, layers and look \
        items. Components are the ids the prompt listed; others are dropped."
    )]
    async fn set_assessment(&self, Parameters(p): Parameters<Assessment>) -> Out {
        let (label, ctx) = self.repo().map_err(failed)?;
        let review = ctx.current_review(self.workspace()).map_err(failed)?;
        let reply = serde_json::to_value(&p).map_err(|e| e.to_string())?;
        let a = ctx
            .save_assessment(review, &p.revset, reply)
            .await
            .map_err(failed)?;
        ArchChanged {
            workspace: self.workspace().map(str::to_string),
            review,
        }
        .emit_to(self.app(), &label)
        .ok();
        Ok(format!("Stored it: {}/5.", a.score))
    }
}
