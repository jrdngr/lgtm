//! Describe: a terminal agent names and describes the system map, its people and external
//! systems, and hands the description back through lgtm's set_system tool.

use std::collections::HashSet;
use std::fmt::Write;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use specta::Type;

use super::system::SystemDto;
use crate::ctx::{RepoCtx, now_ms};
use crate::store::Store;

const KEY: &str = "system";

/// Past this many, the prompt lists only the heaviest edges.
const MAX_EDGES: usize = 150;

const INSTRUCTIONS: &str = "Describe this system as the top three levels of a C4 model, for a \
map in lgtm. Read the README, manifests and entry points to learn what it does; skim code only \
where the names don't say. Don't edit files. Then call lgtm's set_system tool with:

- summary: what the system does, in a sentence.
- people: who uses it, each with a name and a short description.
- externals: the systems outside this repository it talks to (services, CLIs, databases, \
platforms), each with a name, a kind such as CLI, API or Database, and a short description.
- containers: one per container above, by its id, with a short name, its tech and a short \
description. Add containers the code doesn't have its own folder for, like a database the app \
keeps or a second binary, with a new id.
- tiers: names for the rows the components sit in, from entry points down to foundations, like \
Entry, Features, Platform. Three or four.
- components: each component above by its id, with a description of a few words and its tier.
- relations: who uses what, between people, externals and containers (by name for people and \
externals, by id for containers), each with a label of a few words like \"Reads diffs\". Leave \
out relations between containers that only stand for imports.

Keep every description under a dozen words. Then tell me in a line that it's done.";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ActorDto {
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExternalDto {
    pub name: String,
    pub kind: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ContainerDescDto {
    pub id: String,
    pub name: String,
    pub tech: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ComponentDescDto {
    pub id: String,
    pub description: String,
    pub tier: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RelationDto {
    pub from: String,
    pub to: String,
    pub label: String,
}

/// set_system's input.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Description {
    pub summary: String,
    pub people: Vec<ActorDto>,
    pub externals: Vec<ExternalDto>,
    pub containers: Vec<ContainerDescDto>,
    pub tiers: Vec<String>,
    pub components: Vec<ComponentDescDto>,
    pub relations: Vec<RelationDto>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SystemDescDto {
    pub summary: String,
    pub people: Vec<ActorDto>,
    pub externals: Vec<ExternalDto>,
    pub containers: Vec<ContainerDescDto>,
    /// Top to bottom.
    pub tiers: Vec<String>,
    pub components: Vec<ComponentDescDto>,
    pub relations: Vec<RelationDto>,
    /// The working copy's commit when it was written.
    pub commit: String,
    pub created_at: f64,
}

pub fn described(store: &Store) -> anyhow::Result<Option<SystemDescDto>> {
    Ok(match store.arch_state(KEY)? {
        Some(data) => Some(serde_json::from_str(&data)?),
        None => None,
    })
}

/// The map's structure, for the prompt.
fn map_text(map: &SystemDto) -> String {
    let mut out = String::from(
        "Containers are source roots and components are folders in them, with ids as given. \
         An edge means `from` imports from `to`.\n\nContainers (id, name, files):\n\n",
    );
    for c in &map.containers {
        writeln!(out, "- `{}` {}: {} files", c.id, c.name, c.files).unwrap();
    }
    out.push_str("\nComponents (id, container, files):\n\n");
    for c in &map.components {
        writeln!(out, "- `{}` in `{}`: {} files", c.id, c.container, c.files).unwrap();
    }
    let mut edges: Vec<_> = map.edges.iter().collect();
    edges.sort_by(|a, b| b.imports.cmp(&a.imports));
    let hidden = edges.len().saturating_sub(MAX_EDGES);
    out.push_str("\nEdges (from -> to: import sites):\n\n");
    for e in edges.into_iter().take(MAX_EDGES) {
        writeln!(out, "- `{}` -> `{}`: {}", e.from, e.to, e.imports).unwrap();
    }
    if hidden > 0 {
        writeln!(out, "- and {hidden} lighter edges").unwrap();
    }
    out
}

/// Keeps what names something on the map: known component ids, people, externals and
/// containers in relations.
fn validate(d: Description, map: &SystemDto, commit: String) -> SystemDescDto {
    let comps: HashSet<&str> = map.components.iter().map(|c| c.id.as_str()).collect();
    let mut containers = d.containers;
    containers.dedup_by(|a, b| a.id == b.id);
    let names: HashSet<String> = d
        .people
        .iter()
        .map(|p| p.name.clone())
        .chain(d.externals.iter().map(|e| e.name.clone()))
        .chain(containers.iter().map(|c| c.id.clone()))
        .chain(map.containers.iter().map(|c| c.id.clone()))
        .collect();
    let tiers = d.tiers;
    SystemDescDto {
        summary: d.summary,
        people: d.people,
        externals: d.externals,
        containers,
        components: d
            .components
            .into_iter()
            .filter(|c| comps.contains(c.id.as_str()))
            .map(|c| ComponentDescDto {
                tier: c.tier.filter(|t| tiers.contains(t)),
                ..c
            })
            .collect(),
        tiers,
        relations: d
            .relations
            .into_iter()
            .filter(|r| r.from != r.to && names.contains(&r.from) && names.contains(&r.to))
            .collect(),
        commit,
        created_at: now_ms() as f64,
    }
}

impl RepoCtx {
    /// The prompt asking a terminal agent to describe `workspace`'s system map.
    pub async fn describe_prompt(&self, workspace: Option<&str>) -> anyhow::Result<String> {
        let map = self.system_map(workspace).await?;
        Ok(format!(
            "The system map of this repository, from its imports:\n\n{}\n{INSTRUCTIONS}",
            map_text(&map)
        ))
    }

    /// Stores `reply`, in set_system's format, as the description of the system map,
    /// replacing the last one.
    pub async fn save_description(
        &self,
        workspace: Option<&str>,
        reply: serde_json::Value,
    ) -> anyhow::Result<SystemDescDto> {
        let d: Description =
            serde_json::from_value(reply).context("the description doesn't match the format")?;
        let map = self.system_map(workspace).await?;
        let ws = self.workspace(workspace).await?;
        let commit = ws
            .jj
            .commit_ids("@")
            .await?
            .into_iter()
            .next()
            .context("no working-copy commit")?;
        let desc = validate(d, &map, commit);
        self.store
            .lock()
            .unwrap()
            .save_arch_state(KEY, &serde_json::to_string(&desc)?)?;
        Ok(desc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::arch::system::{SysComponentDto, SysContainerDto};

    fn map() -> SystemDto {
        SystemDto {
            repo: "app".into(),
            containers: vec![SysContainerDto {
                id: "src".into(),
                name: "app".into(),
                files: 3,
            }],
            components: vec![SysComponentDto {
                id: "src/ui".into(),
                name: "ui".into(),
                container: "src".into(),
                files: 3,
            }],
            edges: vec![],
            description: None,
            stale: vec![],
        }
    }

    fn relation(from: &str, to: &str) -> RelationDto {
        RelationDto {
            from: from.into(),
            to: to.into(),
            label: "Uses".into(),
        }
    }

    #[test]
    fn validate_drops_unknown_components_tiers_and_relation_ends() {
        let d = Description {
            summary: "An app".into(),
            people: vec![ActorDto {
                name: "User".into(),
                description: "Uses it".into(),
            }],
            externals: vec![ExternalDto {
                name: "GitHub".into(),
                kind: "API".into(),
                description: "PRs".into(),
            }],
            containers: vec![ContainerDescDto {
                id: "db".into(),
                name: "Store".into(),
                tech: "SQLite".into(),
                description: "State".into(),
            }],
            tiers: vec!["Entry".into()],
            components: vec![
                ComponentDescDto {
                    id: "src/ui".into(),
                    description: "Screens".into(),
                    tier: Some("Nowhere".into()),
                },
                ComponentDescDto {
                    id: "src/gone".into(),
                    description: "Old".into(),
                    tier: Some("Entry".into()),
                },
            ],
            relations: vec![
                relation("User", "src"),
                relation("src", "db"),
                relation("src", "GitHub"),
                relation("src", "Slack"),
                relation("src", "src"),
            ],
        };
        let out = validate(d, &map(), "abc".into());
        assert_eq!(
            out.components.iter().map(|c| (&*c.id, c.tier.as_deref())).collect::<Vec<_>>(),
            [("src/ui", None)]
        );
        assert_eq!(
            out.relations.iter().map(|r| (&*r.from, &*r.to)).collect::<Vec<_>>(),
            [("User", "src"), ("src", "db"), ("src", "GitHub")]
        );
        assert_eq!(out.commit, "abc");
    }

    #[test]
    fn map_text_keeps_the_heaviest_edges() {
        let mut m = map();
        m.edges = (0..MAX_EDGES as u32 + 2)
            .map(|i| crate::review::arch::system::SysEdgeDto {
                from: format!("a{i}"),
                to: "b".into(),
                imports: i,
            })
            .collect();
        let text = map_text(&m);
        assert!(text.contains(&format!("`a{}` -> `b`", MAX_EDGES + 1)));
        assert!(!text.contains("`a0` -> `b`"));
        assert!(text.contains("and 2 lighter edges"));
    }
}
