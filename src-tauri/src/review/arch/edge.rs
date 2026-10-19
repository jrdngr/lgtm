//! An edge of the system map up close: where one component imports another, and which of
//! the imported component's symbols the importing files name.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;
use specta::Type;

use super::explore::{self, SysSymbolDto};
use super::model;
use crate::ctx::RepoCtx;

/// How many symbols an edge lists.
const NAMES: usize = 12;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SiteDto {
    pub path: String,
    /// 1-based.
    pub line: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EdgeSitesDto {
    /// Each import of a `to` file in a `from` file, by path then line.
    pub sites: Vec<SiteDto>,
    /// `to`'s symbols the importing files name, most used first.
    pub names: Vec<SysSymbolDto>,
}

fn edge_sites(
    root: &Path,
    snap: &model::Snapshot,
    comp: impl Fn(&str) -> String,
    from: &str,
    to: &str,
) -> EdgeSitesDto {
    let mut sites = Vec::new();
    // Each imported file, with the importing files.
    let mut importers: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (path, imports) in snap.iter().filter(|(p, _)| comp(p) == from) {
        for i in imports.iter().filter(|i| comp(&i.to) == to) {
            sites.push(SiteDto {
                path: path.clone(),
                line: i.line,
            });
            let users = importers.entry(&i.to).or_default();
            if !users.contains(&path.as_str()) {
                users.push(path);
            }
        }
    }
    sites.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
    let mut names = explore::named_by(root, &importers, to);
    names.truncate(NAMES);
    EdgeSitesDto { sites, names }
}

impl RepoCtx {
    /// Where component `from` imports component `to`, and the symbols it uses there.
    pub async fn system_edge(
        &self,
        workspace: Option<&str>,
        from: String,
        to: String,
    ) -> anyhow::Result<EdgeSitesDto> {
        self.explore(workspace, move |root, snap, comp| {
            edge_sites(root, snap, comp, &from, &to)
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::arch::model::Import;

    #[test]
    fn edge_sites_lists_imports_and_the_symbols_the_importers_name() {
        let dir = tempfile::tempdir().unwrap();
        for (path, text) in [
            (
                "lib/a.ts",
                "export function load() {}\nexport function save() {}\n",
            ),
            ("lib/b.ts", "export function other() {}\n"),
            (
                "ui/x.ts",
                "import { save } from '../lib/a';\nimport { other } from '../lib/b';\nsave(); other();\n",
            ),
            ("ui/y.ts", "import { save } from '../lib/a';\nsave();\n"),
            ("cli/z.ts", "import { load } from '../lib/a';\nload();\n"),
        ] {
            let p = dir.path().join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        let import = |to: &str, line| Import {
            to: to.into(),
            line,
        };
        let snap: model::Snapshot = [
            ("lib/a.ts", vec![]),
            ("lib/b.ts", vec![]),
            ("ui/y.ts", vec![import("lib/a.ts", 1)]),
            (
                "ui/x.ts",
                vec![import("lib/a.ts", 1), import("lib/b.ts", 2)],
            ),
            ("cli/z.ts", vec![import("lib/a.ts", 1)]),
        ]
        .into_iter()
        .map(|(p, i)| (p.to_string(), i))
        .collect();
        let comp = |p: &str| p.split('/').next().unwrap().to_string();
        let out = edge_sites(dir.path(), &snap, comp, "ui", "lib");
        assert_eq!(
            out.sites
                .iter()
                .map(|s| (&*s.path, s.line))
                .collect::<Vec<_>>(),
            [("ui/x.ts", 1), ("ui/x.ts", 2), ("ui/y.ts", 1)]
        );
        assert_eq!(
            out.names
                .iter()
                .map(|s| (&*s.name, &*s.path, s.users))
                .collect::<Vec<_>>(),
            [("save", "lib/a.ts", 2), ("other", "lib/b.ts", 1)]
        );
    }
}
