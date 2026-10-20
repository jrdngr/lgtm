//! Symbols for exploring the system map: a component's most used ones, and finding any by
//! name. Symbols are parsed per file and cached by modification time.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::SystemTime;

use serde::Serialize;
use specta::Type;

use super::{model, system};
use crate::ctx::RepoCtx;
use crate::syntax::SymbolKind;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SysSymbolDto {
    pub name: String,
    pub path: String,
    pub line: u32,
    pub component: String,
    /// Files in other components that name it, for a component's most used symbols.
    pub users: u32,
}

/// Names shorter than this match too much text to count as uses.
pub(super) const MIN_NAME: usize = 3;
/// How many most used symbols a component lists.
const MOST_USED: usize = 5;
/// How many symbols a find returns.
const FOUND: usize = 20;
/// Past this many entries the cache starts over.
const MAX_CACHED: usize = 100_000;

/// A symbol a file defines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Sym {
    pub name: String,
    pub kind: SymbolKind,
    /// 1-based, inclusive.
    pub line: u32,
    pub end: u32,
}

type Stamp = (SystemTime, u64);
pub(super) type Syms = Arc<Vec<Sym>>;

static CACHE: LazyLock<Mutex<HashMap<PathBuf, (Stamp, Syms)>>> = LazyLock::new(Mutex::default);

/// The symbols `rel` defines, by line.
pub(super) fn symbols(root: &Path, rel: &str) -> Syms {
    let abs = root.join(rel);
    let Ok(meta) = std::fs::metadata(&abs) else {
        return Syms::default();
    };
    let stamp = (
        meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        meta.len(),
    );
    if let Some((s, syms)) = CACHE.lock().unwrap().get(&abs)
        && *s == stamp
    {
        return syms.clone();
    }
    let syms: Syms = Arc::new(
        std::fs::read_to_string(&abs)
            .ok()
            .and_then(|text| {
                let parsed = crate::syntax::parse(rel, &text)?;
                Some(
                    parsed
                        .symbols(&text)
                        .into_iter()
                        .map(|s| Sym {
                            name: s.name,
                            kind: s.kind,
                            line: s.start_line,
                            end: s.end_line,
                        })
                        .collect(),
                )
            })
            .unwrap_or_default(),
    );
    let mut cache = CACHE.lock().unwrap();
    if cache.len() >= MAX_CACHED {
        cache.clear();
    }
    cache.insert(abs, (stamp, syms.clone()));
    syms
}

/// Whether `name` appears in `text` as a whole identifier.
pub(super) fn names(text: &str, name: &str) -> bool {
    let ident = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    text.match_indices(name).any(|(i, _)| {
        !text[..i].chars().next_back().is_some_and(ident)
            && !text[i + name.len()..].chars().next().is_some_and(ident)
    })
}

/// `component`'s symbols that files in other components importing its files name, by how
/// many such files do, most first.
fn most_used(
    root: &Path,
    snap: &model::Snapshot,
    comp: impl Fn(&str) -> String,
    component: &str,
) -> Vec<SysSymbolDto> {
    // Each of the component's files, with the files elsewhere that import it.
    let mut importers: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (from, imports) in snap {
        if comp(from) == component {
            continue;
        }
        for i in imports.iter().filter(|i| comp(&i.to) == component) {
            importers.entry(&i.to).or_default().push(from);
        }
    }
    let mut out = named_by(root, &importers, component);
    out.truncate(MOST_USED);
    out
}

/// The symbols of each file in `importers` that the files importing it name, with how many
/// of those do, most first. All are in `component`.
pub(super) fn named_by(
    root: &Path,
    importers: &BTreeMap<&str, Vec<&str>>,
    component: &str,
) -> Vec<SysSymbolDto> {
    let mut texts: HashMap<&str, Option<String>> = HashMap::new();
    let mut out = Vec::new();
    for (file, users) in importers {
        for s in symbols(root, file).iter() {
            if s.name.len() < MIN_NAME {
                continue;
            }
            let n = users
                .iter()
                .filter(|u| {
                    texts
                        .entry(u)
                        .or_insert_with(|| std::fs::read_to_string(root.join(u)).ok())
                        .as_deref()
                        .is_some_and(|t| names(t, &s.name))
                })
                .count() as u32;
            if n > 0 {
                out.push(SysSymbolDto {
                    name: s.name.clone(),
                    path: file.to_string(),
                    line: s.line,
                    component: component.to_string(),
                    users: n,
                });
            }
        }
    }
    out.sort_by(|a, b| b.users.cmp(&a.users).then(a.name.cmp(&b.name)));
    out
}

/// Symbols whose names hold `query`, ignoring case: exact matches, then prefixes, then the
/// rest, shorter first.
fn find(
    root: &Path,
    snap: &model::Snapshot,
    comp: impl Fn(&str) -> String,
    query: &str,
) -> Vec<SysSymbolDto> {
    let q = query.to_lowercase();
    let paths: Vec<&String> = snap.keys().collect();
    let all = crate::par::map(&paths, |p| symbols(root, p));
    let mut hits: Vec<(u8, SysSymbolDto)> = paths
        .iter()
        .zip(all)
        .flat_map(|(path, syms)| {
            syms.iter()
                .filter_map(|s| {
                    let lower = s.name.to_lowercase();
                    let rank = if lower == q {
                        0
                    } else if lower.starts_with(&q) {
                        1
                    } else if lower.contains(&q) {
                        2
                    } else {
                        return None;
                    };
                    Some((
                        rank,
                        SysSymbolDto {
                            name: s.name.clone(),
                            path: path.to_string(),
                            line: s.line,
                            component: comp(path),
                            users: 0,
                        },
                    ))
                })
                .collect::<Vec<_>>()
        })
        .collect();
    hits.sort_by(|(ra, a), (rb, b)| {
        (ra, a.name.len(), &a.name, &a.path).cmp(&(rb, b.name.len(), &b.name, &b.path))
    });
    hits.dedup_by(|(_, a), (_, b)| a.name == b.name && a.path == b.path);
    hits.into_iter().take(FOUND).map(|(_, s)| s).collect()
}

impl RepoCtx {
    /// Runs `f` on `workspace`'s working copy: its root, its files' imports and their
    /// components.
    pub(super) async fn explore<T: Send + 'static>(
        &self,
        workspace: Option<&str>,
        f: impl FnOnce(&Path, &model::Snapshot, &dyn Fn(&str) -> String) -> T + Send + 'static,
    ) -> anyhow::Result<T> {
        let ws = self.workspace(workspace).await?;
        let repo = super::load::repo_name(&self.root);
        let root = ws.root.clone();
        let (_, files) = self.view_files(workspace).await?;
        Ok(tokio::task::spawn_blocking(move || {
            let (snap, g) = system::scanned(&root, &repo, &files);
            f(&root, &snap, &|p| g.comp(p))
        })
        .await?)
    }

    /// `component`'s symbols other components use most.
    pub async fn system_most_used(
        &self,
        workspace: Option<&str>,
        component: String,
    ) -> anyhow::Result<Vec<SysSymbolDto>> {
        self.explore(workspace, move |root, snap, comp| {
            most_used(root, snap, comp, &component)
        })
        .await
    }

    /// Symbols in the working copy whose names hold `query`.
    pub async fn system_find(
        &self,
        workspace: Option<&str>,
        query: String,
    ) -> anyhow::Result<Vec<SysSymbolDto>> {
        if query.trim().len() < 2 {
            return Ok(vec![]);
        }
        self.explore(workspace, move |root, snap, comp| {
            find(root, snap, comp, query.trim())
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::arch::model::Import;

    fn repo(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (path, text) in files {
            let p = dir.path().join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        dir
    }

    fn comp(p: &str) -> String {
        p.rsplit_once('/').map_or("", |(d, _)| d).to_string()
    }

    #[test]
    fn names_matches_whole_identifiers_only() {
        assert!(names("useReview(x)", "useReview"));
        assert!(!names("useReviewKeys()", "useReview"));
        assert!(!names("_useReview", "useReview"));
    }

    #[test]
    fn most_used_counts_other_components_files_that_name_each_symbol() {
        let dir = repo(&[
            (
                "lib/a.ts",
                "export function load() {}\nexport function save() {}\nexport function unused() {}\n",
            ),
            (
                "ui/x.ts",
                "import { load, save } from '../lib/a';\nload(); save();\n",
            ),
            ("ui/y.ts", "import { load } from '../lib/a';\nload();\n"),
            ("lib/b.ts", "import { save } from './a';\nsave();\n"),
        ]);
        let import = || {
            vec![Import {
                to: "lib/a.ts".into(),
                line: 1,
            }]
        };
        let snap: model::Snapshot = [
            ("lib/a.ts", vec![]),
            ("ui/x.ts", import()),
            ("ui/y.ts", import()),
            ("lib/b.ts", import()),
        ]
        .into_iter()
        .map(|(p, i)| (p.to_string(), i))
        .collect();
        let used = most_used(dir.path(), &snap, comp, "lib");
        assert_eq!(
            used.iter()
                .map(|s| (&*s.name, s.users, s.line))
                .collect::<Vec<_>>(),
            [("load", 2, 1), ("save", 1, 2)]
        );
    }

    #[test]
    fn find_ranks_exact_then_prefix_then_contains() {
        let dir = repo(&[(
            "lib/a.ts",
            "export function loadNotes() {}\nexport function notes() {}\nexport function useNotesPanel() {}\nexport function other() {}\n",
        )]);
        let snap: model::Snapshot = [("lib/a.ts".to_string(), vec![])].into_iter().collect();
        let found = find(dir.path(), &snap, comp, "Notes");
        assert_eq!(
            found.iter().map(|s| &*s.name).collect::<Vec<_>>(),
            ["notes", "loadNotes", "useNotesPanel"]
        );
        assert_eq!(found[0].component, "lib");
    }
}
