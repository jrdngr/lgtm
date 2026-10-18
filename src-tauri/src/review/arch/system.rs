//! The system map: every component and the imports between them in a workspace's working copy,
//! with the description a terminal agent wrote of the system around them.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde::Serialize;
use specta::Type;

use super::describe::{self, SystemDescDto};
use super::group::Grouping;
use super::{inputs, model, roots, scan, state};
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
}

/// `from` imports from `to` at `imports` sites.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SysEdgeDto {
    pub from: String,
    pub to: String,
    pub imports: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SystemDto {
    pub repo: String,
    pub containers: Vec<SysContainerDto>,
    pub components: Vec<SysComponentDto>,
    pub edges: Vec<SysEdgeDto>,
    pub description: Option<SystemDescDto>,
    /// Components with files changed since the description was written.
    pub stale: Vec<String>,
}

/// Components and edges of the files in `snap`.
pub(super) fn structure(
    snap: &model::Snapshot,
    g: &Grouping,
) -> (Vec<SysContainerDto>, Vec<SysComponentDto>, Vec<SysEdgeDto>) {
    let mut files: BTreeMap<String, u32> = BTreeMap::new();
    for path in snap.keys() {
        *files.entry(g.comp(path)).or_default() += 1;
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
    let edges = model::edges(snap, g)
        .into_iter()
        .map(|((from, to), imports)| SysEdgeDto {
            from,
            to,
            imports: imports.len() as u32,
        })
        .collect();
    (containers, components, edges)
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
        let (containers, components, edges, path_comps) =
            tokio::task::spawn_blocking(move || {
                let wc = scan::scan(&root, &HashSet::new());
                let roots = roots::detect(&wc.paths, &wc.manifests, &repo_name);
                let snap = inputs::snapshot(&wc.paths, &wc.imports, &wc.manifests);
                let g = Grouping::new(roots, lens, &repo_name);
                let (containers, components, edges) = structure(&snap, &g);
                let comp_of = move |p: &str| g.comp(p);
                (containers, components, edges, comp_of)
            })
            .await?;
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
    fn structure_counts_files_and_import_sites_per_component() {
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
        let (containers, components, edges) = structure(&snap, &g);
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
    }
}
