//! The design tool: storing an agent's map of a design's spec files.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::{Deserialize, Serialize};
use tauri_specta::Event as _;

use super::server::{Lgtm, Out, failed, json};
use crate::events::DesignChanged;
use crate::review::arch::design::DesignInput;

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

#[derive(Deserialize, JsonSchema)]
pub struct NewDesign {
    /// What the user calls it.
    pub name: String,
    /// The jj revision holding the spec files: a change id, `@`, a bookmark name, or any
    /// revset of one commit. `@` when left out.
    #[serde(default, alias = "bookmark")]
    pub revision: Option<String>,
    /// Spec files and folders, relative to the repo root; a folder takes everything under it.
    pub files: Vec<String>,
    /// What to map from them, like one concept or some phases. Empty for all of it.
    #[serde(default)]
    pub prompt: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct DesignId {
    /// A design's id from list_designs.
    pub id: String,
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
        DesignChanged {
            id: id.clone(),
            workspace: self.workspace().map(String::from),
        }
        .emit_to(self.app(), &label)
        .ok();
        let m = d.map.as_ref();
        Ok(format!(
            "Stored design `{id}`: {} containers, {} components.",
            m.map_or(0, |m| m.containers.len()),
            m.map_or(0, |m| m.components.len())
        ))
    }

    #[tool(
        description = "Make a design in lgtm: a named set of spec files at a jj revision, mapped \
        as a C4 diagram in the Architecture tab. Returns the instructions for mapping it, which \
        end with calling set_design."
    )]
    async fn create_design(&self, Parameters(p): Parameters<NewDesign>) -> Out {
        let (label, ctx) = self.repo().map_err(failed)?;
        let input = DesignInput {
            name: p.name,
            revision: p.revision.unwrap_or_else(|| "@".into()),
            files: p.files,
            prompt: p.prompt,
        };
        let d = ctx
            .create_design(self.workspace(), input)
            .await
            .map_err(failed)?;
        DesignChanged {
            id: d.id.clone(),
            workspace: self.workspace().map(String::from),
        }
        .emit_to(self.app(), &label)
        .ok();
        ctx.design_prompt(self.workspace(), &d.id)
            .await
            .map_err(failed)
    }

    #[tool(
        description = "List the repo's designs: id, name, revision (as stored, and the commit \
        it resolves to now), files, prompt, and whether its map is fresh, stale (its files \
        changed since it was mapped) or missing (not mapped, or its revision or files are gone)."
    )]
    async fn list_designs(&self) -> Out {
        let (_, ctx) = self.repo().map_err(failed)?;
        let items = ctx.designs(self.workspace()).await.map_err(failed)?;
        let out: Vec<_> = items
            .into_iter()
            .map(|i| {
                serde_json::json!({
                    "id": i.design.id,
                    "name": i.design.name,
                    "revision": i.design.revision,
                    "commit": i.resolved.as_ref().map(|r| &r.commit),
                    "files": i.design.files,
                    "prompt": i.design.prompt,
                    "state": i.state,
                })
            })
            .collect();
        json(&out)
    }

    #[tool(
        description = "The instructions for mapping design `id` again, which end with \
        calling set_design. Use it on a stale or missing design from list_designs."
    )]
    async fn design_instructions(&self, Parameters(p): Parameters<DesignId>) -> Out {
        let (_, ctx) = self.repo().map_err(failed)?;
        ctx.design_prompt(self.workspace(), &p.id)
            .await
            .map_err(failed)
    }
}
