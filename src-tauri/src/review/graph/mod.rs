//! The symbol graph: a review's changed symbols, how they use each other, and the unchanged
//! symbols that use them. Without type information, symbols link by name.

mod callers;
mod scan;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use serde::Serialize;
use specta::Type;

use super::RepoCtx;
use super::build::{FileSide, ReviewFile};
use crate::anchor::Side;
use crate::diff::{FileStatus, RowKind};
use crate::syntax::{CodeSymbol, SymbolKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum NodeStatus {
    Added,
    Modified,
    Removed,
    /// Unchanged, but uses a changed symbol.
    Caller,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: u32,
    pub name: String,
    pub kind: SymbolKind,
    pub status: NodeStatus,
    pub path: String,
    /// The review file, absent for a caller in a file the review doesn't change.
    pub file: Option<u32>,
    /// 1-based, on the new side, or the old side for a removed symbol.
    pub line: u32,
    /// Changed rows directly in the symbol, by index among the file's rows, in order. Empty
    /// for callers.
    pub rows: Vec<u32>,
    /// The enclosing symbol's name, like the type of an `impl` for a method.
    pub parent: Option<String>,
    /// 0 for a changed symbol. For a caller, how many uses away from a changed symbol it is.
    pub hop: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    /// The node whose code uses `to`'s name.
    pub from: u32,
    pub to: u32,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GraphDto {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

impl RepoCtx {
    /// The open review's symbol graph, with callers from its workspace's working copy.
    pub async fn graph(&self, id: u32) -> anyhow::Result<GraphDto> {
        let (files, root) = {
            let review = self.review(id)?;
            let review = review.lock().unwrap();
            (review.files.clone(), review.ws.root.clone())
        };
        Ok(tokio::task::spawn_blocking(move || graph(&files, Some(&root))).await?)
    }
}

/// A changed symbol and where its code is.
struct Node {
    dto: GraphNode,
    side: Side,
    sym: usize,
}

/// Builds the graph of `files`, looking for callers in the rest of the files under `root`.
pub fn graph(files: &[ReviewFile], root: Option<&Path>) -> GraphDto {
    let nodes = changed(files);
    let mut edges = Vec::new();
    let dtos: Vec<GraphNode> = nodes.iter().map(|n| n.dto.clone()).collect();
    let index = name_index(&dtos);
    for n in &nodes {
        let side = files[n.dto.file.unwrap() as usize].side(n.side);
        for (name, call) in names_used(side, n.sym) {
            for to in targets(&index, name, call, &n.dto.path) {
                if to != n.dto.id {
                    edges.push(GraphEdge { from: n.dto.id, to });
                }
            }
        }
    }
    let mut dtos = dtos;
    let mut frontier = 0..dtos.len();
    for hop in 1..=callers::MAX_HOPS {
        // Callers of the last hop's symbols, by names long enough to count.
        let wanted: Vec<GraphNode> = dtos[frontier.clone()]
            .iter()
            .filter(|n| n.name.len() >= callers::MIN_NAME)
            .cloned()
            .collect();
        if wanted.is_empty() {
            break;
        }
        let index = name_index(&wanted);
        let mut found = callers::Found::new(hop);
        for (i, f) in files.iter().enumerate() {
            if f.status != FileStatus::Removed {
                found.scan(
                    &f.new.path,
                    Some(i as u32),
                    &f.new.symbols,
                    &f.new.refs,
                    &dtos,
                    &index,
                );
            }
        }
        if let Some(root) = root {
            let skip: HashSet<&str> = files.iter().map(|f| f.path.as_str()).collect();
            // Names worth looking for outside the review.
            let names: HashSet<String> = index.keys().map(|s| s.to_string()).collect();
            for s in scan::mentioning(root, &skip, &names) {
                found.scan(&s.path, None, &s.symbols, &s.refs, &dtos, &index);
            }
        }
        let start = dtos.len();
        found.finish(&mut dtos, &mut edges);
        frontier = start..dtos.len();
    }
    edges.sort_by_key(|e| (e.from, e.to));
    edges.dedup();
    GraphDto { nodes: dtos, edges }
}

/// The innermost symbol around `line`.
fn innermost(symbols: &[CodeSymbol], line: u32) -> Option<usize> {
    symbols
        .iter()
        .enumerate()
        .filter(|(_, s)| s.start_line <= line && line <= s.end_line)
        .min_by_key(|(_, s)| s.end_line - s.start_line)
        .map(|(i, _)| i)
}

/// The innermost symbol around symbol `i`, other than itself.
fn parent(symbols: &[CodeSymbol], i: usize) -> Option<usize> {
    let s = &symbols[i];
    symbols
        .iter()
        .enumerate()
        .filter(|(j, p)| *j != i && p.start_line <= s.start_line && s.end_line <= p.end_line)
        .min_by_key(|(_, p)| p.end_line - p.start_line)
        .map(|(j, _)| j)
}

/// Names a symbol's own code uses, leaving out code in nested symbols, and whether any use
/// is a call.
fn names_used(side: &FileSide, sym: usize) -> HashMap<&str, bool> {
    let s = &side.symbols[sym];
    let mut out: HashMap<&str, bool> = HashMap::new();
    for r in side
        .refs
        .iter()
        .filter(|r| s.start_line <= r.line && r.line <= s.end_line)
        .filter(|r| innermost(&side.symbols, r.line) == Some(sym))
    {
        *out.entry(&r.name).or_default() |= r.call;
    }
    out
}

/// Changed symbols by name.
type NameIndex<'a> = HashMap<&'a str, Vec<&'a GraphNode>>;

fn name_index(nodes: &[GraphNode]) -> NameIndex<'_> {
    let mut index: NameIndex = HashMap::new();
    for n in nodes {
        index.entry(&n.name).or_default().push(n);
    }
    index
}

/// Past this many same-named symbols in other files, a use can't say which it means.
const MAX_AMBIGUOUS: usize = 2;

/// The changed symbols a use of `name` in `path` points at. A function needs a call, a type
/// any mention, and a module or section is never linked, since its name is often a
/// variable's too. Ones in the same file win; otherwise too many candidates means none.
fn targets(index: &NameIndex, name: &str, call: bool, path: &str) -> Vec<u32> {
    let Some(all) = index.get(name) else {
        return vec![];
    };
    let fits = |n: &&&GraphNode| match n.kind {
        SymbolKind::Function => call,
        SymbolKind::Class => true,
        SymbolKind::Module | SymbolKind::Section => false,
    };
    let candidates: Vec<&&GraphNode> = all.iter().filter(fits).collect();
    let local: Vec<u32> = candidates
        .iter()
        .filter(|n| n.path == path)
        .map(|n| n.id)
        .collect();
    if !local.is_empty() {
        local
    } else if candidates.len() > MAX_AMBIGUOUS {
        vec![]
    } else {
        candidates.iter().map(|n| n.id).collect()
    }
}

/// One node per symbol with changed rows directly in it. A symbol changed on both sides is
/// one node, found on the new side by name, kind, and enclosing symbol.
fn changed(files: &[ReviewFile]) -> Vec<Node> {
    let mut nodes = Vec::new();
    for (fi, f) in files.iter().enumerate() {
        // (side, symbol) -> changed rows
        let mut touched: BTreeMap<(bool, usize), Vec<u32>> = BTreeMap::new();
        for (ri, r) in f.rows.iter().enumerate() {
            let (side, line) = match r.kind {
                RowKind::Added => (Side::New, r.new_line),
                RowKind::Removed => (Side::Old, r.old_line),
                RowKind::Context => continue,
            };
            let Some(si) = line.and_then(|l| innermost(&f.side(side).symbols, l)) else {
                continue;
            };
            touched
                .entry((side == Side::Old, si))
                .or_default()
                .push(ri as u32);
        }
        let mut new_side: BTreeMap<usize, Vec<u32>> = BTreeMap::new();
        let mut removed = Vec::new();
        for ((old, si), rows) in touched {
            match if old { counterpart(f, si) } else { Some(si) } {
                Some(t) => new_side.entry(t).or_default().extend(rows),
                None => removed.push((si, rows)),
            }
        }
        let mut push = |side: Side, si: usize, mut rows: Vec<u32>, status| {
            rows.sort();
            let fs = f.side(side);
            let s = &fs.symbols[si];
            nodes.push(Node {
                dto: GraphNode {
                    id: nodes.len() as u32,
                    name: s.name.clone(),
                    kind: s.kind,
                    status,
                    path: fs.path.clone(),
                    file: Some(fi as u32),
                    line: s.start_line,
                    rows,
                    parent: parent(&fs.symbols, si).map(|p| fs.symbols[p].name.clone()),
                    hop: 0,
                },
                side,
                sym: si,
            });
        };
        for (si, rows) in new_side {
            let status = if counterpart_old(f, si).is_some() {
                NodeStatus::Modified
            } else {
                NodeStatus::Added
            };
            push(Side::New, si, rows, status);
        }
        for (si, rows) in removed {
            push(Side::Old, si, rows, NodeStatus::Removed);
        }
    }
    nodes
}

/// The new side's symbol matching old symbol `si`.
fn counterpart(f: &ReviewFile, si: usize) -> Option<usize> {
    find_same(&f.old.symbols, si, &f.new.symbols)
}

/// The old side's symbol matching new symbol `si`.
fn counterpart_old(f: &ReviewFile, si: usize) -> Option<usize> {
    find_same(&f.new.symbols, si, &f.old.symbols)
}

fn find_same(from: &[CodeSymbol], si: usize, to: &[CodeSymbol]) -> Option<usize> {
    let s = &from[si];
    let parent_name = |syms: &[CodeSymbol], i| parent(syms, i).map(|p| syms[p].name.clone());
    let want = parent_name(from, si);
    (0..to.len())
        .find(|&j| to[j].name == s.name && to[j].kind == s.kind && parent_name(to, j) == want)
}
