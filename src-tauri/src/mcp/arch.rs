//! The architecture tools: storing an agent's assessment of the review's architecture change,
//! its description of the system map, and its tours.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::{Deserialize, Serialize};
use tauri_specta::Event as _;

use super::server::{Lgtm, Out, failed};
use crate::events::{ArchChanged, SystemChanged};

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
    /// The base commit the prompt named.
    pub from: String,
    /// The head commit the prompt named.
    pub to: String,
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

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Actor {
    pub name: String,
    pub description: String,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct External {
    pub name: String,
    /// Like CLI, API or Database.
    pub kind: String,
    pub description: String,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Container {
    /// A container id from the prompt, or a new one for a container without code here.
    pub id: String,
    pub name: String,
    pub tech: String,
    pub description: String,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Group {
    /// A domain, like Billing or Notes.
    pub name: String,
    pub description: String,
    /// Container ids from the prompt or the containers above; each in one group only.
    pub containers: Vec<String>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Component {
    /// A component id from the prompt.
    pub id: String,
    pub description: String,
    /// One of `tiers`.
    pub tier: Option<String>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Relation {
    /// A person's or external's name, or a container id.
    pub from: String,
    pub to: String,
    /// A few words, like "Reads diffs".
    pub label: String,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct TourStep {
    /// "system", a container id or a component id.
    pub node: String,
    /// A repository-relative file, for a step about a file or a symbol in it.
    pub path: Option<String>,
    /// A symbol's name in `path`, for a step about a symbol.
    pub symbol: Option<String>,
    /// Another node, for a step about how `node` uses it.
    pub to: Option<String>,
    /// Two or three sentences.
    pub text: String,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct System {
    /// What the system does, in a sentence.
    pub summary: String,
    pub people: Vec<Actor>,
    pub externals: Vec<External>,
    pub containers: Vec<Container>,
    /// Row names for components, from entry points down to foundations.
    pub tiers: Vec<String>,
    pub components: Vec<Component>,
    /// With more than five containers, three to eight groups of them by domain; else empty.
    #[serde(default)]
    pub groups: Vec<Group>,
    pub relations: Vec<Relation>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Tour {
    /// The node the prompt named: `overview`, `features`, `k:src/ui` for a component or
    /// `c:src` for a container.
    pub node: String,
    /// One line on what the tour covers.
    pub summary: String,
    /// From zoomed out to zoomed in.
    pub steps: Vec<TourStep>,
}

#[tool_router(router = arch_router, vis = "pub(super)")]
impl Lgtm {
    #[tool(
        description = "Store your assessment of the review's architecture change in lgtm, \
        replacing the last one. Its Architecture tab shows the score, risk, layers and look \
        items. Components are the ids the prompt listed; others are dropped. It's refused once \
        the review has moved off the from and to commits the prompt named."
    )]
    async fn set_assessment(&self, Parameters(p): Parameters<Assessment>) -> Out {
        let (label, ctx) = self.repo().map_err(failed)?;
        let review = ctx.current_review(self.workspace()).map_err(failed)?;
        let reply = serde_json::to_value(&p).map_err(|e| e.to_string())?;
        let a = ctx
            .save_assessment(review, &p.revset, (&p.from, &p.to), reply)
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

    #[tool(
        description = "Store your description of the system map in lgtm, replacing the last \
        one. Its Architecture tab shows it as C4 context, container and component diagrams. \
        Components and containers are the ids the prompt listed; unknown components are \
        dropped."
    )]
    async fn set_system(&self, Parameters(p): Parameters<System>) -> Out {
        let (label, ctx) = self.repo().map_err(failed)?;
        let reply = serde_json::to_value(&p).map_err(|e| e.to_string())?;
        let d = ctx
            .save_description(self.workspace(), reply)
            .await
            .map_err(failed)?;
        SystemChanged {
            workspace: self.workspace().map(str::to_string),
        }
        .emit_to(self.app(), &label)
        .ok();
        Ok(format!(
            "Stored it: {} containers described, {} components.",
            d.containers.len(),
            d.components.len()
        ))
    }

    #[tool(
        description = "Store your tour of the system map in lgtm (the overview, every \
        feature, or one container or component), replacing that tour's last version. Its \
        Architecture tab lists it with the other tours. \
        Steps on unknown nodes are dropped, and so are unknown files and symbols."
    )]
    async fn set_tour(&self, Parameters(p): Parameters<Tour>) -> Out {
        let (label, ctx) = self.repo().map_err(failed)?;
        let reply = serde_json::to_value(&p).map_err(|e| e.to_string())?;
        let (node, steps) = ctx
            .save_tour(self.workspace(), reply)
            .await
            .map_err(failed)?;
        SystemChanged {
            workspace: self.workspace().map(str::to_string),
        }
        .emit_to(self.app(), &label)
        .ok();
        Ok(format!("Stored the tour of `{node}`: {steps} steps."))
    }
}
