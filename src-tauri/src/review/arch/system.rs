//! The system map: every component and the imports between them in a workspace's working copy,
//! with the description a terminal agent wrote of the system around them.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;

use serde::Serialize;
use specta::Type;

use super::describe::{self, SystemDescDto};
use super::group::Grouping;
use super::tour::{self, TourDto};
use super::view::{ArchViewDto, Files, SMALL};
use super::{inputs, model, roots, scan};
use crate::ctx::RepoCtx;
use crate::jj::Range;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SysContainerDto {
    /// The source root's repo-relative path, "" for files outside every root.
    pub id: String,
    pub name: String,
    pub files: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SysComponentDto {
    pub id: String,
    pub name: String,
    pub container: String,
    pub files: u32,
    /// Its files other components import most, most first.
    pub entry: Vec<String>,
}

/// `from` imports from `to` at `imports` sites, the first of them at `path`:`line`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SysEdgeDto {
    pub from: String,
    pub to: String,
    pub imports: u32,
    pub path: String,
    pub line: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SysFileDto {
    pub path: String,
    pub component: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SystemDto {
    pub repo: String,
    pub containers: Vec<SysContainerDto>,
    pub components: Vec<SysComponentDto>,
    pub edges: Vec<SysEdgeDto>,
    /// Every source file.
    pub files: Vec<SysFileDto>,
    pub description: Option<SystemDescDto>,
    /// Components with files changed since the description was written.
    pub stale: Vec<String>,
    /// The overview and features tours, then container and component tours in map order.
    pub tours: Vec<TourDto>,
    /// Whether containers show in groups.
    pub grouped: bool,
}

/// How many entry files a component lists.
const ENTRY: usize = 3;

pub(super) struct Structure {
    pub containers: Vec<SysContainerDto>,
    pub components: Vec<SysComponentDto>,
    pub edges: Vec<SysEdgeDto>,
    pub files: Vec<SysFileDto>,
}

/// Components and edges of the files in `snap`.
pub(super) fn structure(snap: &model::Snapshot, g: &Grouping) -> Structure {
    let mut files: BTreeMap<String, u32> = BTreeMap::new();
    for path in snap.keys() {
        *files.entry(g.comp(path)).or_default() += 1;
    }
    let imports = model::edges(snap, g);
    let mut inbound: BTreeMap<&str, u32> = BTreeMap::new();
    for site in imports.values().flatten() {
        *inbound.entry(&site.to_path).or_default() += 1;
    }
    let mut entries: BTreeMap<String, Vec<(&str, u32)>> = BTreeMap::new();
    for (path, n) in inbound {
        entries.entry(g.comp(path)).or_default().push((path, n));
    }
    let mut containers: BTreeMap<String, u32> = BTreeMap::new();
    let components = files
        .iter()
        .map(|(id, n)| {
            let container = g.container(id);
            *containers.entry(container.clone()).or_default() += n;
            SysComponentDto {
                id: id.clone(),
                name: g.name(id),
                container,
                files: *n,
                entry: entries.get_mut(id).map_or_else(Vec::new, |e| {
                    e.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
                    e.iter().take(ENTRY).map(|(p, _)| p.to_string()).collect()
                }),
            }
        })
        .collect();
    let containers = containers
        .into_iter()
        .map(|(id, files)| {
            let c = g.container_dto(&id);
            SysContainerDto {
                id: c.id,
                name: c.name,
                files,
            }
        })
        .collect();
    let edges = imports
        .into_iter()
        .map(|((from, to), sites)| {
            let first = sites
                .iter()
                .min_by(|a, b| (&a.from_path, a.line).cmp(&(&b.from_path, b.line)))
                .expect("an edge has a site");
            SysEdgeDto {
                imports: sites.len() as u32,
                path: first.from_path.clone(),
                line: first.line,
                from,
                to,
            }
        })
        .collect();
    let files = snap
        .keys()
        .map(|p| SysFileDto {
            path: p.clone(),
            component: g.comp(p),
        })
        .collect();
    Structure {
        containers,
        components,
        edges,
        files,
    }
}

/// The working copy under `root`: the source files `files` keeps with their imports, and how
/// they group.
pub(super) fn scanned(root: &Path, repo: &str, files: &Files) -> (model::Snapshot, Grouping) {
    let mut wc = scan::scan(root, &HashSet::new());
    wc.paths.retain(|p| files.keeps(p));
    wc.imports.retain(|p, _| files.keeps(p));
    let roots = roots::detect(&wc.paths, &wc.manifests, repo);
    let snap = inputs::snapshot(&wc.paths, &wc.imports, &wc.manifests);
    (snap, Grouping::new(roots, repo))
}

/// `st` without small containers, their components and what wires to them, unless `view`
/// shows them.
fn without_small(st: Structure, view: &ArchViewDto) -> Structure {
    if view.small {
        return st;
    }
    let kept: HashSet<String> = st
        .containers
        .iter()
        .filter(|c| c.files >= SMALL)
        .map(|c| c.id.clone())
        .collect();
    let components: Vec<SysComponentDto> = st
        .components
        .into_iter()
        .filter(|k| kept.contains(&k.container))
        .collect();
    let comps: HashSet<&str> = components.iter().map(|k| k.id.as_str()).collect();
    Structure {
        edges: st
            .edges
            .into_iter()
            .filter(|e| comps.contains(e.from.as_str()) && comps.contains(e.to.as_str()))
            .collect(),
        files: st
            .files
            .into_iter()
            .filter(|f| comps.contains(f.component.as_str()))
            .collect(),
        containers: st
            .containers
            .into_iter()
            .filter(|c| kept.contains(&c.id))
            .collect(),
        components,
    }
}

/// `d` without the people and external systems or groups `view` hides.
fn viewed(mut d: SystemDescDto, view: &ArchViewDto) -> SystemDescDto {
    if !view.groups {
        d.groups.clear();
    }
    if !view.outside {
        let outside: HashSet<String> = d
            .people
            .drain(..)
            .map(|p| p.name)
            .chain(d.externals.drain(..).map(|e| e.name))
            .collect();
        d.relations
            .retain(|r| !outside.contains(&r.from) && !outside.contains(&r.to));
    }
    d
}

impl RepoCtx {
    /// The system map of `workspace`'s working copy.
    pub async fn system_map(&self, workspace: Option<&str>) -> anyhow::Result<SystemDto> {
        let ws = self.workspace(workspace).await?;
        let repo = super::load::repo_name(&self.root);
        let (description, stored) = {
            let store = self.store.lock().unwrap();
            (describe::described(&store)?, tour::stored(&store)?)
        };
        let root = ws.root.clone();
        let repo_name = repo.clone();
        let (view, files) = self.view_files(workspace).await?;
        let (st, path_comps) = tokio::task::spawn_blocking(move || {
            let (snap, g) = scanned(&root, &repo_name, &files);
            let st = without_small(structure(&snap, &g), &view);
            (st, move |p: &str| g.comp(p))
        })
        .await?;
        let Structure {
            containers,
            components,
            edges,
            files,
        } = st;
        let stale = match &description {
            Some(d) => {
                let range = Range {
                    from: Some(d.commit.clone()),
                    to: "@".into(),
                };
                // A commit jj can no longer read leaves nothing to compare with.
                match ws.jj.diff_paths(&range).await {
                    Ok(diffs) => diffs
                        .iter()
                        .map(|f| path_comps(&f.path))
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .filter(|id| components.iter().any(|c: &SysComponentDto| c.id == *id))
                        .collect(),
                    Err(_) => vec![],
                }
            }
            None => vec![],
        };
        let tours = tour::tours(description.as_ref(), &stored, &containers, &components);
        let description = description.map(|d| viewed(d, &view));
        Ok(SystemDto {
            repo,
            containers,
            components,
            edges,
            files,
            description,
            stale,
            tours,
            grouped: view.groups,
        })
    }
}

#[cfg(test)]
#[path = "system_tests.rs"]
mod tests;
