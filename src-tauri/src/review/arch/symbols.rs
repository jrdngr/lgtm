//! The symbols level of the system map: a component's functions and classes, the ones in
//! other components that use them or that they use, and which uses which, by name.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use serde::Serialize;
use specta::Type;

use super::explore::{MIN_NAME, Sym, names, symbols};
use super::model;
use crate::ctx::RepoCtx;
use crate::syntax::SymbolKind;

/// How many of its own symbols a component lists.
const MAX_SYMBOLS: usize = 80;
/// How many callers, and how many calls, a component lists.
const MAX_NEIGHBORS: usize = 16;
/// How many characters of a definition's first line a signature keeps.
const SIGNATURE: usize = 140;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SymNodeDto {
    pub id: u32,
    pub name: String,
    pub kind: SymbolKind,
    pub path: String,
    /// 1-based, inclusive.
    pub line: u32,
    pub end: u32,
    pub component: String,
    /// The definition's first line, trimmed.
    pub signature: String,
}

/// `from` uses `to`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SymEdgeDto {
    pub from: u32,
    pub to: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SymbolsDto {
    /// The component's, by file then line.
    pub symbols: Vec<SymNodeDto>,
    /// Symbols in other components that use them.
    pub callers: Vec<SymNodeDto>,
    /// Symbols in other components they use.
    pub calls: Vec<SymNodeDto>,
    pub edges: Vec<SymEdgeDto>,
}

/// The functions and classes `path` defines, leaving out functions nested in another kept
/// function.
fn defs(root: &Path, path: &str) -> Vec<Sym> {
    let mut out: Vec<Sym> = Vec::new();
    for s in symbols(root, path).iter() {
        let nested = || {
            out.iter()
                .any(|o| o.kind == SymbolKind::Function && o.line <= s.line && s.end <= o.end)
        };
        match s.kind {
            SymbolKind::Class => out.push(s.clone()),
            SymbolKind::Function if !nested() => out.push(s.clone()),
            _ => {}
        }
    }
    out
}

/// Lines `start..=end` (1-based) of `text`.
fn body(text: &str, start: u32, end: u32) -> &str {
    let mut lines = text.split_inclusive('\n');
    let skip: usize = lines
        .by_ref()
        .take(start.saturating_sub(1) as usize)
        .map(str::len)
        .sum();
    let len: usize = lines
        .take((end + 1).saturating_sub(start) as usize)
        .map(str::len)
        .sum();
    &text[skip..skip + len]
}

/// The first line of `s`'s definition that names it, trimmed and capped.
fn signature(text: &str, s: &Sym) -> String {
    let b = body(text, s.line, s.end);
    let line = b
        .lines()
        .find(|l| names(l, &s.name))
        .or_else(|| b.lines().next())
        .unwrap_or("");
    line.trim().chars().take(SIGNATURE).collect()
}

/// Whether one of `a` and `b` in the same file holds the other, like a class and its methods.
fn nests(a: &Sym, b: &Sym) -> bool {
    (a.line <= b.line && b.end <= a.end) || (b.line <= a.line && a.end <= b.end)
}

/// File texts read once each, "" for one that can't be read.
struct Texts<'a> {
    root: &'a Path,
    read: HashMap<String, String>,
}

impl Texts<'_> {
    fn get(&mut self, path: &str) -> &str {
        let root = self.root;
        self.read
            .entry(path.to_string())
            .or_insert_with(|| std::fs::read_to_string(root.join(path)).unwrap_or_default())
    }
}

/// A symbol in another component, with the component's symbols it's linked to.
struct Neighbor {
    path: String,
    sym: Sym,
    links: BTreeSet<usize>,
}

/// The `MAX_NEIGHBORS` with the most links, as (path, line) -> neighbor.
fn heaviest(all: BTreeMap<(String, u32), Neighbor>) -> Vec<Neighbor> {
    let mut v: Vec<Neighbor> = all.into_values().collect();
    v.sort_by(|a, b| {
        b.links
            .len()
            .cmp(&a.links.len())
            .then((&a.path, a.sym.line).cmp(&(&b.path, b.sym.line)))
    });
    v.truncate(MAX_NEIGHBORS);
    v
}

fn component_symbols(
    root: &Path,
    snap: &model::Snapshot,
    comp: impl Fn(&str) -> String,
    component: &str,
) -> SymbolsDto {
    let mut texts = Texts {
        root,
        read: HashMap::new(),
    };
    let files: Vec<&str> = snap
        .keys()
        .filter(|p| comp(p) == component)
        .map(String::as_str)
        .collect();
    let own: Vec<(&str, Sym)> = files
        .iter()
        .flat_map(|f| defs(root, f).into_iter().map(move |s| (*f, s)))
        .take(MAX_SYMBOLS)
        .collect();
    let bodies: Vec<String> = own
        .iter()
        .map(|(p, s)| body(texts.get(p), s.line, s.end).to_string())
        .collect();
    let usable = |s: &Sym| s.name.len() >= MIN_NAME;
    let imports = |p: &str| -> BTreeSet<&str> {
        snap.get(p)
            .into_iter()
            .flatten()
            .map(|i| i.to.as_str())
            .collect()
    };

    // Uses inside the component: in the same file or a file the user's file imports.
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for (a, (a_path, a_sym)) in own.iter().enumerate() {
        let reach = imports(a_path);
        for (b, (b_path, b_sym)) in own.iter().enumerate() {
            let near = if a_path == b_path {
                a != b && !nests(a_sym, b_sym)
            } else {
                reach.contains(b_path)
            };
            if near && usable(b_sym) && names(&bodies[a], &b_sym.name) {
                edges.push((a, b));
            }
        }
    }

    // Calls: symbols of imported files elsewhere that the component's symbols name.
    let mut calls: BTreeMap<(String, u32), Neighbor> = BTreeMap::new();
    for file in &files {
        let users: Vec<usize> = (0..own.len()).filter(|&i| own[i].0 == *file).collect();
        if users.is_empty() {
            continue;
        }
        for target in imports(file).into_iter().filter(|t| comp(t) != component) {
            let text = texts.get(file).to_string();
            for sym in defs(root, target)
                .into_iter()
                .filter(|s| usable(s) && names(&text, &s.name))
            {
                let links: BTreeSet<usize> = users
                    .iter()
                    .copied()
                    .filter(|&a| names(&bodies[a], &sym.name))
                    .collect();
                if !links.is_empty() {
                    let n = calls
                        .entry((target.to_string(), sym.line))
                        .or_insert_with(|| Neighbor {
                            path: target.to_string(),
                            sym,
                            links: BTreeSet::new(),
                        });
                    n.links.extend(links);
                }
            }
        }
    }

    // Callers: symbols of files elsewhere that import the component's files and name its
    // symbols.
    let mut callers: BTreeMap<(String, u32), Neighbor> = BTreeMap::new();
    for (path, list) in snap {
        if comp(path) == component {
            continue;
        }
        let targets: BTreeSet<&str> = list
            .iter()
            .map(|i| i.to.as_str())
            .filter(|t| comp(t) == component)
            .collect();
        if targets.is_empty() {
            continue;
        }
        let text = texts.get(path).to_string();
        let named: Vec<usize> = (0..own.len())
            .filter(|&b| {
                targets.contains(own[b].0) && usable(&own[b].1) && names(&text, &own[b].1.name)
            })
            .collect();
        if named.is_empty() {
            continue;
        }
        for sym in defs(root, path) {
            let user = body(&text, sym.line, sym.end);
            let links: BTreeSet<usize> = named
                .iter()
                .copied()
                .filter(|&b| names(user, &own[b].1.name))
                .collect();
            if !links.is_empty() {
                callers.insert(
                    (path.clone(), sym.line),
                    Neighbor {
                        path: path.clone(),
                        sym,
                        links,
                    },
                );
            }
        }
    }

    let node = |texts: &mut Texts, id: usize, path: &str, s: &Sym| SymNodeDto {
        id: id as u32,
        name: s.name.clone(),
        kind: s.kind,
        path: path.to_string(),
        line: s.line,
        end: s.end,
        component: comp(path),
        signature: signature(texts.get(path), s),
    };
    let symbols: Vec<SymNodeDto> = own
        .iter()
        .enumerate()
        .map(|(i, (p, s))| node(&mut texts, i, p, s))
        .collect();
    let mut out_edges: Vec<SymEdgeDto> = edges
        .into_iter()
        .map(|(a, b)| SymEdgeDto {
            from: a as u32,
            to: b as u32,
        })
        .collect();
    let mut next = own.len();
    let mut place = |list: Vec<Neighbor>, outward: bool, texts: &mut Texts| -> Vec<SymNodeDto> {
        list.into_iter()
            .map(|n| {
                let id = next;
                next += 1;
                for &o in &n.links {
                    let (from, to) = if outward { (o, id) } else { (id, o) };
                    out_edges.push(SymEdgeDto {
                        from: from as u32,
                        to: to as u32,
                    });
                }
                node(texts, id, &n.path, &n.sym)
            })
            .collect()
    };
    let callers = place(heaviest(callers), false, &mut texts);
    let calls = place(heaviest(calls), true, &mut texts);
    out_edges.sort();
    out_edges.dedup();
    SymbolsDto {
        symbols,
        callers,
        calls,
        edges: out_edges,
    }
}

impl RepoCtx {
    /// `component`'s symbols, the symbols elsewhere that use them or that they use, and
    /// which uses which.
    pub async fn system_symbols(
        &self,
        workspace: Option<&str>,
        component: String,
    ) -> anyhow::Result<SymbolsDto> {
        self.explore(workspace, move |root, snap, comp| {
            component_symbols(root, snap, comp, &component)
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::arch::model::Import;

    #[test]
    fn body_takes_inclusive_one_based_lines() {
        assert_eq!(body("a\nb\nc\nd", 2, 3), "b\nc\n");
        assert_eq!(body("a\nb", 2, 5), "b");
    }

    #[test]
    fn component_symbols_links_uses_inside_and_across_components() {
        let dir = tempfile::tempdir().unwrap();
        for (path, text) in [
            (
                "lib/a.ts",
                "import { fetchRow } from '../db/q';\n\
                 export function load(id) {\n  function inner() { return 1; }\n  return fetchRow(id);\n}\n\
                 export class Cache {\n  get() { return load(1); }\n}\n",
            ),
            (
                "lib/b.ts",
                "import { load } from './a';\nexport function save() { load(2); }\n",
            ),
            (
                "db/q.ts",
                "export function fetchRow(id) {}\nexport function unused() {}\n",
            ),
            (
                "ui/v.ts",
                "import { save } from '../lib/b';\nexport function view() { save(); }\n",
            ),
        ] {
            let p = dir.path().join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        let import = |to: &str| Import {
            to: to.into(),
            line: 1,
        };
        let snap: model::Snapshot = [
            ("lib/a.ts", vec![import("db/q.ts")]),
            ("lib/b.ts", vec![import("lib/a.ts")]),
            ("db/q.ts", vec![]),
            ("ui/v.ts", vec![import("lib/b.ts")]),
        ]
        .into_iter()
        .map(|(p, i)| (p.to_string(), i))
        .collect();
        let comp = |p: &str| p.split('/').next().unwrap().to_string();
        let out = component_symbols(dir.path(), &snap, comp, "lib");
        let all: Vec<&SymNodeDto> = out
            .symbols
            .iter()
            .chain(&out.callers)
            .chain(&out.calls)
            .collect();
        let name = |id: u32| &*all.iter().find(|n| n.id == id).unwrap().name;
        assert_eq!(
            out.symbols
                .iter()
                .map(|s| (&*s.name, s.line))
                .collect::<Vec<_>>(),
            [("load", 2), ("Cache", 6), ("get", 7), ("save", 2)]
        );
        assert_eq!(out.symbols[0].signature, "export function load(id) {");
        assert_eq!(
            out.callers
                .iter()
                .map(|s| (&*s.name, &*s.component))
                .collect::<Vec<_>>(),
            [("view", "ui")]
        );
        assert_eq!(
            out.calls
                .iter()
                .map(|s| (&*s.name, &*s.component))
                .collect::<Vec<_>>(),
            [("fetchRow", "db")]
        );
        let mut edges: Vec<_> = out
            .edges
            .iter()
            .map(|e| (name(e.from), name(e.to)))
            .collect();
        edges.sort();
        assert_eq!(
            edges,
            [
                ("Cache", "load"),
                ("get", "load"),
                ("load", "fetchRow"),
                ("save", "load"),
                ("view", "save")
            ]
        );
    }
}
