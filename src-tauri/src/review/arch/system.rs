//! The system map: every component and the imports between them in a workspace's working copy,
//! with the description a terminal agent wrote of the system around them.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;

use serde::Serialize;
use specta::Type;

use super::describe::{self, SystemDescDto};
use super::group::Grouping;
use super::{Lens, inputs, model, roots, scan, state};
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

/// The working copy under `root`: its source files with their imports, and how they group.
pub(super) fn scanned(root: &Path, lens: Lens, repo: &str) -> (model::Snapshot, Grouping) {
    let wc = scan::scan(root, &HashSet::new());
    let roots = roots::detect(&wc.paths, &wc.manifests, repo);
    let snap = inputs::snapshot(&wc.paths, &wc.imports, &wc.manifests);
    (snap, Grouping::new(roots, lens, repo))
}

impl RepoCtx {
    /// The system map of `workspace`'s working copy.
    pub async fn system_map(&self, workspace: Option<&str>) -> anyhow::Result<SystemDto> {
        let ws = self.workspace(workspace).await?;
        let repo = super::load::repo_name(&self.root);
        let lens = state::lens(&self.store.lock().unwrap())?;
        let description = describe::described(&self.store.lock().unwrap())?;
        let root = ws.root.clone();
        let repo_name = repo.clone();
        let (st, path_comps) = tokio::task::spawn_blocking(move || {
            let (snap, g) = scanned(&root, lens, &repo_name);
            let st = structure(&snap, &g);
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
                match ws.jj.diff(&range).await {
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
        Ok(SystemDto {
            repo,
            containers,
            components,
            edges,
            files,
            description,
            stale,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::arch::roots::Root;
    use crate::review::arch::{Lens, model::Import};

    #[test]
    fn structure_counts_files_import_sites_and_entry_files_per_component() {
        let import = |to: &str, line| Import {
            to: to.into(),
            line,
        };
        let snap: model::Snapshot = [
            ("src/ui/a.ts", vec![import("src/db/x.ts", 1), import("src/db/y.ts", 2)]),
            ("src/ui/b.ts", vec![import("src/db/x.ts", 4), import("src/ui/a.ts", 5)]),
            ("src/db/x.ts", vec![]),
            ("src/db/y.ts", vec![]),
            ("build.ts", vec![import("src/ui/a.ts", 1)]),
        ]
        .into_iter()
        .map(|(p, i)| (p.to_string(), i))
        .collect();
        let roots = vec![Root {
            path: "src".into(),
            name: "app".into(),
        }];
        let g = Grouping::new(roots, Lens::default(), "repo");
        let Structure {
            containers,
            components,
            edges,
            files: _,
        } = structure(&snap, &g);
        let files = |cs: &[SysComponentDto]| -> Vec<(String, String, u32)> {
            cs.iter()
                .map(|c| (c.id.clone(), c.container.clone(), c.files))
                .collect()
        };
        assert_eq!(
            containers.iter().map(|c| (&*c.id, &*c.name, c.files)).collect::<Vec<_>>(),
            [("", "repo", 1), ("src", "app", 4)]
        );
        assert_eq!(
            files(&components),
            [
                ("".into(), "".into(), 1),
                ("src/db".into(), "src".into(), 2),
                ("src/ui".into(), "src".into(), 2),
            ]
        );
        assert_eq!(
            edges.iter().map(|e| (&*e.from, &*e.to, e.imports)).collect::<Vec<_>>(),
            [("", "src/ui", 1), ("src/ui", "src/db", 3)]
        );
        assert_eq!(
            edges.iter().map(|e| (&*e.path, e.line)).collect::<Vec<_>>(),
            [("build.ts", 1), ("src/ui/a.ts", 1)]
        );
        let entry = |id: &str| components.iter().find(|c| c.id == id).unwrap().entry.clone();
        assert_eq!(entry("src/db"), ["src/db/x.ts", "src/db/y.ts"]);
        assert_eq!(entry("src/ui"), ["src/ui/a.ts"]);
    }
}
