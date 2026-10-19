//! The prompt asking a terminal agent to write a tour: the overview, every feature, or one
//! container or component.

use std::collections::BTreeMap;
use std::fmt::Write;

use anyhow::bail;

use super::describe::map_text;
use super::system::{SysComponentDto, SysContainerDto, SystemDto};
use crate::ctx::RepoCtx;

/// Past this many, the prompt lists only a component's first files.
const MAX_FILES: usize = 60;

const COMPONENT: &str = "Write a tour of this component for someone new to it, for lgtm's \
system map. Walk them from zoomed out to zoomed in: its container, the component itself, who \
uses it (a step with node set to the user and `to` set to this component), its main files \
(steps with a path), the key symbols in them (steps with a path and the symbol's name), then \
what it uses (node set to this component and `to` set to the other). Use six to ten steps.";

const CONTAINER: &str = "Write a tour of this container for someone new to it, for lgtm's \
system map. Walk them from zoomed out to zoomed in: the container, its main components, the \
key files and symbols of the most important ones (steps with a path, and the symbol's name for \
a symbol), then the containers and external systems it talks to (node set to this container \
and `to` set to the other, when it's on the map). Use six to ten steps.";

const OVERVIEW: &str = "Write a tour that walks someone new through this system in reading \
order, for lgtm's system map. Read the README, manifests and entry points first. Start at \
\"system\", then the containers and components that matter most, and how they connect. \
Use six to ten steps.";

const FEATURES: &str = "Write a tour of every main feature of this app, for lgtm's system \
map: one or two steps each, ten to twenty steps in all. Point each at the component, file or \
symbol that does it.";

const STEPS: &str = "Give each step a node (\"system\", a container id or a \
component id) and two or three sentences on what it does and how it connects to the steps \
before it. Read the code you need to; don't edit files. Then call lgtm's set_tour tool with \
node";

/// The container's components, edges in and out of it, and its description.
fn container_text(map: &SystemDto, c: &SysContainerDto) -> String {
    let desc = map.description.as_ref();
    let name = desc
        .and_then(|d| d.containers.iter().find(|dc| dc.id == c.id))
        .map_or(c.name.clone(), |dc| {
            format!("{} ({}): {}", dc.name, dc.tech, dc.description)
        });
    let mut out = format!(
        "The container `{}`, {}, has {} files.\n\n",
        c.id, name, c.files
    );
    out.push_str("Its components (id, files):\n\n");
    for comp in map.components.iter().filter(|x| x.container == c.id) {
        write!(out, "- `{}`: {} files", comp.id, comp.files).unwrap();
        if let Some(d) = desc.and_then(|d| d.components.iter().find(|dc| dc.id == comp.id)) {
            write!(out, ", {}", d.description).unwrap();
        }
        out.push('\n');
    }
    let container = |id: &str| {
        map.components
            .iter()
            .find(|x| x.id == id)
            .map_or(String::new(), |x| x.container.clone())
    };
    let mut inbound: BTreeMap<String, u32> = BTreeMap::new();
    let mut outbound: BTreeMap<String, u32> = BTreeMap::new();
    for e in &map.edges {
        let (from, to) = (container(&e.from), container(&e.to));
        if from == to {
            continue;
        }
        if to == c.id {
            *inbound.entry(from).or_default() += e.imports;
        } else if from == c.id {
            *outbound.entry(to).or_default() += e.imports;
        }
    }
    edges_text(&mut out, "Containers that import from it", &inbound);
    edges_text(&mut out, "Containers it imports from", &outbound);
    if let Some(d) = desc {
        let rels: Vec<_> = d
            .relations
            .iter()
            .filter(|r| r.from == c.id || r.to == c.id)
            .collect();
        if !rels.is_empty() {
            out.push_str("\nIts relations:\n\n");
            for r in rels {
                writeln!(out, "- {} -> {}: {}", r.from, r.to, r.label).unwrap();
            }
        }
    }
    out
}

/// The component's files, entry files, edges in and out of it, and its description.
fn component_text(map: &SystemDto, c: &SysComponentDto) -> String {
    let mut out = format!(
        "The component `{}` is in the container `{}`",
        c.id, c.container
    );
    if let Some(d) = map
        .description
        .as_ref()
        .and_then(|d| d.components.iter().find(|dc| dc.id == c.id))
    {
        write!(out, ": {}", d.description).unwrap();
    }
    out.push_str(".\n\nIts files:\n\n");
    let files: Vec<_> = map.files.iter().filter(|f| f.component == c.id).collect();
    for f in files.iter().take(MAX_FILES) {
        writeln!(out, "- {}", f.path).unwrap();
    }
    if files.len() > MAX_FILES {
        writeln!(out, "- and {} more", files.len() - MAX_FILES).unwrap();
    }
    if !c.entry.is_empty() {
        out.push_str("\nThe files other components import most:\n\n");
        for e in &c.entry {
            writeln!(out, "- {e}").unwrap();
        }
    }
    let side = |inbound: bool| -> BTreeMap<String, u32> {
        map.edges
            .iter()
            .filter(|e| {
                if inbound {
                    e.to == c.id
                } else {
                    e.from == c.id
                }
            })
            .map(|e| (if inbound { &e.from } else { &e.to }.clone(), e.imports))
            .collect()
    };
    edges_text(&mut out, "Components that import from it", &side(true));
    edges_text(&mut out, "Components it imports from", &side(false));
    out
}

fn edges_text(out: &mut String, title: &str, edges: &BTreeMap<String, u32>) {
    if edges.is_empty() {
        return;
    }
    writeln!(out, "\n{title} (id: import sites):\n").unwrap();
    for (id, n) in edges {
        writeln!(out, "- `{id}`: {n}").unwrap();
    }
}

/// The prompt for `node`'s tour, or an error when it isn't on `map`.
fn prompt(map: &SystemDto, node: &str) -> anyhow::Result<String> {
    let container = node.strip_prefix("c:");
    let component = node.strip_prefix("k:");
    let (context, ask) = if node == "overview" || node == "features" {
        let ask = if node == "overview" { OVERVIEW } else { FEATURES };
        (
            format!("The system map, from its imports:\n\n{}", map_text(map)),
            ask,
        )
    } else if let Some(c) = map.containers.iter().find(|c| Some(&*c.id) == container)
    {
        (container_text(map, c), CONTAINER)
    } else if let Some(c) = map.components.iter().find(|c| Some(&*c.id) == component) {
        (component_text(map, c), COMPONENT)
    } else {
        bail!("`{node}` isn't a container or component on the map");
    };
    let summary = map
        .description
        .as_ref()
        .map(|d| format!("The system: {}\n\n", d.summary))
        .unwrap_or_default();
    Ok(format!(
        "{summary}{context}\n{ask} {STEPS} `{node}`, a one-line summary of what the tour \
         covers, and the steps, and tell me in a line that it's done."
    ))
}

impl RepoCtx {
    /// The prompt asking a terminal agent to write the overview or features tour, or the tour
    /// of a container or component.
    pub async fn tour_prompt(&self, workspace: Option<&str>, node: &str) -> anyhow::Result<String> {
        let map = self.system_map(workspace).await?;
        prompt(&map, node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::arch::system::{SysEdgeDto, SysFileDto};

    #[test]
    fn prompt_gives_a_component_its_files_and_edges_and_a_container_its_components() {
        let comp = |id: &str, container: &str| SysComponentDto {
            id: id.into(),
            name: id.into(),
            container: container.into(),
            files: 1,
            entry: vec![],
        };
        let edge = |from: &str, to: &str, imports| SysEdgeDto {
            from: from.into(),
            to: to.into(),
            imports,
            path: String::new(),
            line: 1,
        };
        let map = SystemDto {
            repo: "app".into(),
            containers: ["src", "web"]
                .map(|id| SysContainerDto {
                    id: id.into(),
                    name: id.into(),
                    files: 1,
                })
                .to_vec(),
            components: vec![
                comp("src/ui", "src"),
                comp("src/db", "src"),
                comp("web/api", "web"),
            ],
            edges: vec![edge("src/ui", "src/db", 2), edge("web/api", "src/db", 3)],
            files: vec![SysFileDto {
                path: "src/db/x.ts".into(),
                component: "src/db".into(),
            }],
            description: None,
            stale: vec![],
            tours: vec![],
        };
        let db = prompt(&map, "k:src/db").unwrap();
        assert!(db.contains("- src/db/x.ts"));
        assert!(db.contains(
            "Components that import from it (id: import sites):\n\n- `src/ui`: 2\n- `web/api`: 3"
        ));
        assert!(!db.contains("Components it imports from"));
        let src = prompt(&map, "c:src").unwrap();
        assert!(src.contains("- `src/db`: 1 files"));
        assert!(!src.contains("`web/api`: 1 files"));
        assert!(src.contains("Containers that import from it (id: import sites):\n\n- `web`: 3"));
        assert!(db.contains("set_tour tool with node `k:src/db`,"));
        let features = prompt(&map, "features").unwrap();
        assert!(features.contains("- `web/api` in `web`: 1 files"));
        assert!(features.contains("set_tour tool with node `features`,"));
        // A bare id could be the container or its loose files' component.
        assert!(prompt(&map, "src").is_err());
        assert!(prompt(&map, "nowhere").is_err());
    }
}
