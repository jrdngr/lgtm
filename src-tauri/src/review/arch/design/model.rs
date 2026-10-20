//! A design's map: the people, external systems, containers, components and interfaces a spec
//! describes, each marked by how it changes the code's system and, once code lands, whether
//! it's built.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::ctx::now_ms;
use crate::review::arch::system::SystemDto;

/// How a design changes a node of the code's system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum DesignChange {
    New,
    Changed,
    Removed,
    Same,
}

/// Whether the code has what the design plans.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum DesignBuilt {
    Built,
    Planned,
    Differs,
}

/// A line of a spec file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SpecRefDto {
    pub path: String,
    pub line: u32,
    /// What it says there, in a sentence.
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DesignActorDto {
    pub name: String,
    pub description: String,
    pub change: DesignChange,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DesignExternalDto {
    pub name: String,
    pub kind: String,
    pub description: String,
    pub change: DesignChange,
    pub built: Option<DesignBuilt>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DesignContainerDto {
    pub id: String,
    pub name: String,
    pub tech: String,
    pub description: String,
    pub change: DesignChange,
    /// The code's container it is or changes.
    pub code: Option<String>,
    /// Removed containers it takes over from.
    pub replaces: Vec<String>,
    pub spec: Vec<SpecRefDto>,
    pub built: Option<DesignBuilt>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceDto {
    pub name: String,
    /// Its shape as the spec gives it, like `send_push(user, msg) -> Delivery`.
    pub signature: String,
    pub description: String,
    pub spec: Vec<SpecRefDto>,
    pub built: Option<DesignBuilt>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DesignComponentDto {
    pub id: String,
    pub container: String,
    pub name: String,
    pub description: String,
    pub change: DesignChange,
    /// The code's component it is or changes.
    pub code: Option<String>,
    /// Removed components it takes over from.
    pub replaces: Vec<String>,
    pub spec: Vec<SpecRefDto>,
    pub interfaces: Vec<InterfaceDto>,
    pub built: Option<DesignBuilt>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DesignRelationDto {
    pub from: String,
    pub to: String,
    pub label: String,
    pub change: DesignChange,
    pub built: Option<DesignBuilt>,
}

/// Where the code differs from the design: something built otherwise, or not in the spec.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DifferenceDto {
    /// The design node it's about: a container or component id, or a person's or external's
    /// name.
    pub node: String,
    pub text: String,
    pub path: Option<String>,
    pub line: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DesignMapDto {
    pub summary: String,
    pub people: Vec<DesignActorDto>,
    pub externals: Vec<DesignExternalDto>,
    pub containers: Vec<DesignContainerDto>,
    pub components: Vec<DesignComponentDto>,
    pub relations: Vec<DesignRelationDto>,
    pub differences: Vec<DifferenceDto>,
    /// The bookmark's commit it was written from.
    pub spec_commit: String,
    /// The working copy's commit it was compared with.
    pub code_commit: Option<String>,
    pub created_at: f64,
}

/// set_design's format.
#[derive(Clone, Debug, Deserialize)]
pub struct DesignReply {
    pub summary: String,
    pub people: Vec<DesignActorDto>,
    pub externals: Vec<DesignExternalDto>,
    pub containers: Vec<DesignContainerDto>,
    pub components: Vec<DesignComponentDto>,
    pub relations: Vec<DesignRelationDto>,
    #[serde(default)]
    pub differences: Vec<DifferenceDto>,
}

/// Keeps what hangs together: components in known containers, code links to the code's
/// containers and components, and relations and differences on known nodes.
pub(super) fn validate(
    d: DesignReply,
    map: &SystemDto,
    spec_commit: String,
    code_commit: Option<String>,
) -> DesignMapDto {
    let code_containers: HashSet<&str> = map.containers.iter().map(|c| c.id.as_str()).collect();
    let code_components: HashSet<&str> = map.components.iter().map(|c| c.id.as_str()).collect();
    let containers: Vec<DesignContainerDto> = d
        .containers
        .into_iter()
        .map(|c| DesignContainerDto {
            code: c.code.filter(|id| code_containers.contains(id.as_str())),
            ..c
        })
        .collect();
    let ids: HashSet<String> = containers.iter().map(|c| c.id.clone()).collect();
    let containers: Vec<DesignContainerDto> = containers
        .into_iter()
        .map(|mut c| {
            c.replaces.retain(|r| r != &c.id && ids.contains(r));
            c
        })
        .collect();
    let components: Vec<DesignComponentDto> = d
        .components
        .into_iter()
        .filter(|k| ids.contains(&k.container))
        .map(|k| DesignComponentDto {
            code: k.code.filter(|id| code_components.contains(id.as_str())),
            ..k
        })
        .collect();
    let kids: HashSet<String> = components.iter().map(|k| k.id.clone()).collect();
    let components: Vec<DesignComponentDto> = components
        .into_iter()
        .map(|mut k| {
            k.replaces.retain(|r| r != &k.id && kids.contains(r));
            k
        })
        .collect();
    let nodes: HashSet<String> = d
        .people
        .iter()
        .map(|p| p.name.clone())
        .chain(d.externals.iter().map(|e| e.name.clone()))
        .chain(ids)
        .chain(components.iter().map(|k| k.id.clone()))
        .collect();
    DesignMapDto {
        summary: d.summary,
        people: d.people,
        externals: d.externals,
        containers,
        components,
        relations: d
            .relations
            .into_iter()
            .filter(|r| r.from != r.to && nodes.contains(&r.from) && nodes.contains(&r.to))
            .collect(),
        differences: d
            .differences
            .into_iter()
            .filter(|x| nodes.contains(&x.node))
            .collect(),
        spec_commit,
        code_commit,
        created_at: now_ms() as f64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_drops_orphan_components_unknown_links_and_loose_relations() {
        let reply: DesignReply = serde_json::from_value(serde_json::json!({
            "summary": "Notifications",
            "people": [{"name": "Shopper", "description": "", "change": "same"}],
            "externals": [],
            "containers": [
                {"id": "api", "name": "api", "tech": "", "description": "", "change": "changed",
                 "code": "api", "replaces": [], "spec": [], "built": null},
                {"id": "notify", "name": "notify-worker", "tech": "", "description": "",
                 "change": "new", "code": "nowhere", "replaces": ["api", "gone"], "spec": [], "built": null}
            ],
            "components": [
                {"id": "notify/push", "container": "notify", "name": "push", "description": "",
                 "change": "new", "code": null, "replaces": [], "spec": [], "interfaces": [], "built": null},
                {"id": "gone/x", "container": "gone", "name": "x", "description": "",
                 "change": "new", "code": null, "replaces": [], "spec": [], "interfaces": [], "built": null}
            ],
            "relations": [
                {"from": "Shopper", "to": "api", "label": "Uses", "change": "same", "built": null},
                {"from": "api", "to": "gone/x", "label": "Calls", "change": "new", "built": null}
            ],
            "differences": [
                {"node": "notify", "text": "Calls Redis", "path": null, "line": null},
                {"node": "Redis", "text": "Not in spec", "path": null, "line": null}
            ]
        }))
        .unwrap();
        let map = SystemDto {
            repo: "shop".into(),
            containers: vec![crate::review::arch::system::SysContainerDto {
                id: "api".into(),
                name: "api".into(),
                files: 3,
            }],
            components: vec![],
            edges: vec![],
            files: vec![],
            description: None,
            stale: vec![],
            tours: vec![],
            grouped: true,
        };
        let out = validate(reply, &map, "c1".into(), None);
        assert_eq!(
            out.containers
                .iter()
                .map(|c| c.code.as_deref())
                .collect::<Vec<_>>(),
            [Some("api"), None]
        );
        assert_eq!(out.containers[1].replaces, ["api"]);
        assert_eq!(
            out.components.iter().map(|k| &*k.id).collect::<Vec<_>>(),
            ["notify/push"]
        );
        assert_eq!(
            out.relations
                .iter()
                .map(|r| (&*r.from, &*r.to))
                .collect::<Vec<_>>(),
            [("Shopper", "api")]
        );
        assert_eq!(
            out.differences.iter().map(|x| &*x.node).collect::<Vec<_>>(),
            ["notify"]
        );
    }
}
