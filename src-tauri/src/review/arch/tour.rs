//! Tours of the system map: the description's overview and features tours, and one per
//! container or component that a terminal agent writes through lgtm's set_tour tool.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::describe::{SystemDescDto, TourStepDto};
use super::explore;
use super::system::{SysComponentDto, SysContainerDto, SystemDto};
use crate::ctx::{RepoCtx, now_ms};
use crate::store::Store;

const KEY: &str = "tours";

/// A tour step's node for the whole system.
const SYSTEM: &str = "system";

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TourDto {
    /// "overview", "features", or the container or component id it tours.
    pub id: String,
    pub title: String,
    /// A line on what it covers, or empty.
    pub summary: String,
    pub steps: Vec<TourStepDto>,
}

/// set_tour's input.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct TourInput {
    pub node: String,
    pub summary: String,
    pub steps: Vec<TourStepDto>,
}

/// A container's or component's tour, as stored.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoredTour {
    pub summary: String,
    pub steps: Vec<TourStepDto>,
    pub created_at: f64,
}

pub(super) fn stored(store: &Store) -> anyhow::Result<BTreeMap<String, StoredTour>> {
    Ok(match store.arch_state(KEY)? {
        Some(data) => serde_json::from_str(&data)?,
        None => BTreeMap::new(),
    })
}

/// "system" and the map's containers and components.
pub(super) fn nodes(map: &SystemDto) -> HashSet<&str> {
    std::iter::once(SYSTEM)
        .chain(map.containers.iter().map(|c| c.id.as_str()))
        .chain(map.components.iter().map(|c| c.id.as_str()))
        .collect()
}

/// Keeps the steps on one of `nodes`, with `to` only when it's one too, `path` only when
/// it's a file on the map, and `symbol` only when `path` defines it, at its line.
pub(super) fn check_steps(
    steps: Vec<TourStepDto>,
    nodes: &HashSet<&str>,
    map: &SystemDto,
    root: &Path,
) -> Vec<TourStepDto> {
    let files: HashSet<&str> = map.files.iter().map(|f| f.path.as_str()).collect();
    steps
        .into_iter()
        .filter(|s| nodes.contains(s.node.as_str()))
        .map(|s| {
            let to = s.to.filter(|t| nodes.contains(t.as_str()) && *t != s.node);
            let path = s.path.filter(|p| files.contains(p.as_str()));
            let (symbol, line) = match (&path, s.symbol) {
                (Some(p), Some(name)) => explore::symbols(root, p)
                    .iter()
                    .find(|sym| sym.name == name)
                    .map_or((None, None), |sym| (Some(name), Some(sym.line))),
                _ => (None, None),
            };
            TourStepDto {
                node: s.node,
                path,
                symbol,
                line,
                to,
                text: s.text,
            }
        })
        .collect()
}

/// The overview and features tours, then the stored tours of the map's containers and
/// components, in map order.
pub(super) fn tours(
    desc: Option<&SystemDescDto>,
    stored: &BTreeMap<String, StoredTour>,
    containers: &[SysContainerDto],
    components: &[SysComponentDto],
) -> Vec<TourDto> {
    let mut out = Vec::new();
    let described = |id: &str, title: &str, steps: &[TourStepDto]| {
        (!steps.is_empty()).then(|| TourDto {
            id: id.into(),
            title: title.into(),
            summary: String::new(),
            steps: steps.to_vec(),
        })
    };
    if let Some(d) = desc {
        out.extend(described("overview", "Overview", &d.tour));
        out.extend(described("features", "Every feature", &d.features));
    }
    let names = containers
        .iter()
        .map(|c| {
            let name = desc
                .and_then(|d| d.containers.iter().find(|dc| dc.id == c.id))
                .map_or(&c.name, |dc| &dc.name);
            (format!("c:{}", c.id), name)
        })
        .chain(components.iter().map(|c| (format!("k:{}", c.id), &c.name)));
    for (id, name) in names {
        if let Some(t) = stored.get(&id) {
            out.push(TourDto {
                id,
                title: name.clone(),
                summary: t.summary.clone(),
                steps: t.steps.clone(),
            });
        }
    }
    out
}

/// Whether `node` is `c:` and a container's id, or `k:` and a component's.
pub(super) fn on_map(map: &SystemDto, node: &str) -> bool {
    if let Some(id) = node.strip_prefix("c:") {
        map.containers.iter().any(|c| c.id == id)
    } else if let Some(id) = node.strip_prefix("k:") {
        map.components.iter().any(|c| c.id == id)
    } else {
        false
    }
}

/// Checks `input` against `map` and the files under `root`.
fn check(input: TourInput, map: &SystemDto, root: &Path) -> anyhow::Result<StoredTour> {
    if !on_map(map, &input.node) {
        bail!("`{}` isn't a container or component on the map", input.node);
    }
    let mut nodes = nodes(map);
    if let Some(d) = &map.description {
        nodes.extend(d.containers.iter().map(|c| c.id.as_str()));
    }
    Ok(StoredTour {
        summary: input.summary,
        steps: check_steps(input.steps, &nodes, map, root),
        created_at: now_ms() as f64,
    })
}

impl RepoCtx {
    /// Stores `reply`, in set_tour's format, as its container's or component's tour,
    /// replacing that node's last one. Returns the node and how many steps it kept.
    pub async fn save_tour(
        &self,
        workspace: Option<&str>,
        reply: serde_json::Value,
    ) -> anyhow::Result<(String, usize)> {
        let input: TourInput =
            serde_json::from_value(reply).context("the tour doesn't match the format")?;
        let map = self.system_map(workspace).await?;
        let ws = self.workspace(workspace).await?;
        let node = input.node.clone();
        let tour = check(input, &map, &ws.root)?;
        let steps = tour.steps.len();
        let store = self.store.lock().unwrap();
        let mut all = stored(&store)?;
        all.insert(node.clone(), tour);
        store.save_arch_state(KEY, &serde_json::to_string(&all)?)?;
        Ok((node, steps))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::arch::system::SysFileDto;

    fn step(node: &str, path: Option<&str>, symbol: Option<&str>, to: Option<&str>) -> TourStepDto {
        TourStepDto {
            node: node.into(),
            path: path.map(Into::into),
            symbol: symbol.map(Into::into),
            line: Some(99),
            to: to.map(Into::into),
            text: "Text".into(),
        }
    }

    fn map() -> SystemDto {
        SystemDto {
            repo: "app".into(),
            containers: vec![SysContainerDto {
                id: "src".into(),
                name: "app".into(),
                files: 1,
            }],
            components: vec![SysComponentDto {
                id: "src/lib".into(),
                name: "lib".into(),
                container: "src".into(),
                files: 1,
                entry: vec![],
            }],
            edges: vec![],
            files: vec![SysFileDto {
                path: "src/lib/a.ts".into(),
                component: "src/lib".into(),
            }],
            description: None,
            stale: vec![],
            tours: vec![],
        }
    }

    #[test]
    fn check_steps_drops_unknown_nodes_and_paths_and_resolves_symbol_lines() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src/lib")).unwrap();
        std::fs::write(
            dir.path().join("src/lib/a.ts"),
            "export const x = 1;\n\nexport function load() {}\n",
        )
        .unwrap();
        let m = map();
        let steps = vec![
            step("system", None, None, Some("src/lib")),
            step("src/gone", None, None, None),
            step(
                "src/lib",
                Some("src/lib/a.ts"),
                Some("load"),
                Some("nowhere"),
            ),
            step("src/lib", Some("src/lib/a.ts"), Some("missing"), None),
            step("src", Some("src/lib/b.ts"), Some("load"), None),
        ];
        let out = check_steps(steps, &nodes(&m), &m, dir.path());
        let got: Vec<_> = out
            .iter()
            .map(|s| {
                (
                    &*s.node,
                    s.path.as_deref(),
                    s.symbol.as_deref(),
                    s.line,
                    s.to.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            got,
            [
                ("system", None, None, None, Some("src/lib")),
                ("src/lib", Some("src/lib/a.ts"), Some("load"), Some(3), None),
                ("src/lib", Some("src/lib/a.ts"), None, None, None),
                ("src", None, None, None, None),
            ]
        );
    }

    #[test]
    fn check_refuses_a_tour_of_an_unknown_node() {
        let input = TourInput {
            node: "k:src/gone".into(),
            summary: String::new(),
            steps: vec![],
        };
        assert!(check(input, &map(), Path::new("/nonexistent")).is_err());
    }

    #[test]
    fn tours_lists_described_tours_then_node_tours_in_map_order() {
        let m = map();
        let stored_tour = |summary: &str| StoredTour {
            summary: summary.into(),
            steps: vec![step("src", None, None, None)],
            created_at: 0.0,
        };
        let stored: BTreeMap<String, StoredTour> = [
            ("k:src/lib".to_string(), stored_tour("The library")),
            ("c:src".to_string(), stored_tour("The app")),
            ("k:src/gone".to_string(), stored_tour("Old")),
        ]
        .into_iter()
        .collect();
        let desc = SystemDescDto {
            summary: String::new(),
            people: vec![],
            externals: vec![],
            containers: vec![crate::review::arch::describe::ContainerDescDto {
                id: "src".into(),
                name: "Desktop app".into(),
                tech: String::new(),
                description: String::new(),
            }],
            tiers: vec![],
            components: vec![],
            relations: vec![],
            tour: vec![step("system", None, None, None)],
            features: vec![],
            commit: String::new(),
            created_at: 0.0,
        };
        let out = tours(Some(&desc), &stored, &m.containers, &m.components);
        assert_eq!(
            out.iter()
                .map(|t| (&*t.id, &*t.title, &*t.summary))
                .collect::<Vec<_>>(),
            [
                ("overview", "Overview", ""),
                ("c:src", "Desktop app", "The app"),
                ("k:src/lib", "lib", "The library"),
            ]
        );
    }
}
