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
- groups: with more than five containers, sort them into three to eight groups by domain \
(what they're for, like Billing or Notes, not by kind like apps or libs), each with a short \
name, a short description and its container ids. Put every container in exactly one group. \
With five or fewer, leave it empty.
- relations: who uses what, between people, externals and containers (by name for people and \
externals, by id for containers), each with a label of a few words like \"Reads diffs\". Leave \
out relations between containers that only stand for imports.
Keep every description under a dozen words. Then tell me in a line that \
it's done.";

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
pub struct TourStepDto {
    /// "system", a container id or a component id.
    pub node: String,
    /// A file, for a step about a file or a symbol in it.
    #[serde(default)]
    pub path: Option<String>,
    /// A symbol in `path`.
    #[serde(default)]
    pub symbol: Option<String>,
    /// The symbol's 1-based line.
    #[serde(default)]
    pub line: Option<u32>,
    /// Another node, for a step about how `node` uses it.
    #[serde(default)]
    pub to: Option<String>,
    pub text: String,
}

/// Containers that belong together, so a large system's context and containers maps show a
/// few groups instead of one block.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct GroupDto {
    pub name: String,
    pub description: String,
    /// Container ids.
    pub containers: Vec<String>,
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
    #[serde(default)]
    pub groups: Vec<GroupDto>,
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
    /// Empty for a small system, and for descriptions written before groups.
    #[serde(default)]
    pub groups: Vec<GroupDto>,
    pub relations: Vec<RelationDto>,
    /// The overview tour, from descriptions that wrote it. New ones keep the last one's.
    #[serde(default)]
    pub tour: Vec<TourStepDto>,
    /// The every-feature tour, likewise.
    #[serde(default)]
    pub features: Vec<TourStepDto>,
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
pub(super) fn map_text(map: &SystemDto) -> String {
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
    edges.sort_by_key(|e| std::cmp::Reverse(e.imports));
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

/// The id of the container `c` names, by id or else by its one scanned or described name.
fn container_id(c: &str, map: &SystemDto, described: &[ContainerDescDto]) -> Option<String> {
    let ids = map.containers.iter().map(|m| (&m.id, &m.name));
    let ids = ids.chain(described.iter().map(|m| (&m.id, &m.name)));
    let all: Vec<_> = ids.collect();
    if all.iter().any(|(id, _)| *id == c) {
        return Some(c.to_string());
    }
    let mut named = all.iter().filter(|(_, name)| *name == c).map(|(id, _)| *id);
    let first = named.next()?;
    named.all(|id| id == first).then(|| first.clone())
}

/// Keeps what names something on the map: known component ids, people, externals and
/// containers in relations, and known containers in groups, each in its first group only.
/// The overview and features tours stay `last`'s.
fn validate(
    d: Description,
    map: &SystemDto,
    commit: String,
    last: Option<SystemDescDto>,
) -> SystemDescDto {
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
    let mut grouped = HashSet::new();
    let groups = d
        .groups
        .into_iter()
        .map(|g| GroupDto {
            containers: g
                .containers
                .into_iter()
                .filter_map(|c| container_id(&c, map, &containers))
                .filter(|c| grouped.insert(c.clone()))
                .collect(),
            ..g
        })
        .filter(|g| !g.containers.is_empty())
        .collect();
    let tiers = d.tiers;
    let (tour, features) = last.map_or_else(Default::default, |l| (l.tour, l.features));
    SystemDescDto {
        tour,
        features,
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
        groups,
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
        let store = self.store.lock().unwrap();
        let desc = validate(d, &map, commit, described(&store)?);
        store.save_arch_state(KEY, &serde_json::to_string(&desc)?)?;
        Ok(desc)
    }
}

#[cfg(test)]
#[path = "describe_tests.rs"]
mod tests;
