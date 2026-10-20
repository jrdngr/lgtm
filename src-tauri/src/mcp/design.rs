//! The design tool: storing an agent's map of a design's spec files.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::{Deserialize, Serialize};
use tauri_specta::Event as _;

use super::server::{Lgtm, Out, failed};
use crate::events::DesignChanged;

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Change {
    New,
    Changed,
    Removed,
    Same,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Built {
    /// The code has it as designed.
    Built,
    /// The code doesn't have it yet.
    Planned,
    /// The code has it otherwise.
    Differs,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct SpecRef {
    /// A spec file's repository-relative path.
    pub path: String,
    /// 1-based.
    pub line: u32,
    /// What it says there, in a sentence.
    pub text: String,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Actor {
    pub name: String,
    pub description: String,
    pub change: Change,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct External {
    pub name: String,
    /// Like API, Database or Platform.
    pub kind: String,
    pub description: String,
    pub change: Change,
    pub built: Option<Built>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Container {
    /// A code container's id when the design keeps or changes it, else a short kebab-case id.
    pub id: String,
    pub name: String,
    pub tech: String,
    pub description: String,
    pub change: Change,
    /// The code container's id it is or changes.
    pub code: Option<String>,
    /// Ids of removed containers it takes over from.
    pub replaces: Vec<String>,
    pub spec: Vec<SpecRef>,
    pub built: Option<Built>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Interface {
    pub name: String,
    /// As the spec writes it, like `send_push(user, msg) -> Delivery`.
    pub signature: String,
    pub description: String,
    pub spec: Vec<SpecRef>,
    pub built: Option<Built>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Component {
    /// `<container id>/<name>`, or the code component's id when it is one.
    pub id: String,
    /// A container id from containers.
    pub container: String,
    pub name: String,
    pub description: String,
    pub change: Change,
    /// The code component's id it is or changes.
    pub code: Option<String>,
    /// Ids of removed components it takes over from.
    pub replaces: Vec<String>,
    pub spec: Vec<SpecRef>,
    pub interfaces: Vec<Interface>,
    pub built: Option<Built>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Relation {
    /// A person's or external's name, or a container or component id.
    pub from: String,
    pub to: String,
    pub label: String,
    pub change: Change,
    pub built: Option<Built>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Difference {
    /// The design node it concerns: a container or component id, or a name.
    pub node: String,
    pub text: String,
    pub path: Option<String>,
    pub line: Option<u32>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
pub struct Design {
    /// The design's id from the prompt.
    #[serde(skip_serializing)]
    pub id: String,
    /// What the design does, in a sentence.
    pub summary: String,
    pub people: Vec<Actor>,
    pub externals: Vec<External>,
    pub containers: Vec<Container>,
    pub components: Vec<Component>,
    pub relations: Vec<Relation>,
    pub differences: Vec<Difference>,
}

#[tool_router(router = design_router, vis = "pub(super)")]
impl Lgtm {
    #[tool(
        description = "Store your map of a design in lgtm, replacing its last one. The \
        Architecture tab shows it over the code's system map. Components in unknown containers, \
        unknown code ids, and relations and differences on unknown nodes are dropped."
    )]
    async fn set_design(&self, Parameters(p): Parameters<Design>) -> Out {
        let (label, ctx) = self.repo().map_err(failed)?;
        let id = p.id.clone();
        let reply = serde_json::to_value(&p).map_err(|e| e.to_string())?;
        let d = ctx
            .save_design_map(self.workspace(), &id, reply)
            .await
            .map_err(failed)?;
        DesignChanged { id: id.clone() }
            .emit_to(self.app(), &label)
            .ok();
        let m = d.map.as_ref();
        Ok(format!(
            "Stored design `{id}`: {} containers, {} components.",
            m.map_or(0, |m| m.containers.len()),
            m.map_or(0, |m| m.components.len())
        ))
    }
}
