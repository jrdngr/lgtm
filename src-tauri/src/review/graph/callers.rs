//! Unchanged symbols that use the review's changed symbols.

use std::collections::{BTreeMap, HashMap, HashSet};

use super::{GraphEdge, GraphNode, NodeStatus, innermost, name_index, parent, targets};
use crate::syntax::{CodeSymbol, Ref};

/// Callers shown per changed symbol, so a common name doesn't flood the graph.
const MAX_PER_NODE: usize = 8;
/// Shorter names match too much to count as uses.
const MIN_NAME: usize = 3;

#[derive(Default)]
pub(super) struct Found {
    callers: Vec<GraphNode>,
    /// (caller index, changed node)
    edges: Vec<(usize, u32)>,
    per_node: HashMap<u32, usize>,
}

impl Found {
    /// Names worth looking for outside the review.
    pub fn wanted_names(&self, nodes: &[GraphNode]) -> HashSet<String> {
        nodes
            .iter()
            .filter(|n| n.name.len() >= MIN_NAME)
            .map(|n| n.name.clone())
            .collect()
    }

    /// Adds the symbols in one file that use a changed symbol's name.
    pub fn scan(
        &mut self,
        path: &str,
        file: Option<u32>,
        symbols: &[CodeSymbol],
        refs: &[Ref],
        nodes: &[GraphNode],
    ) {
        let long: Vec<GraphNode> = nodes
            .iter()
            .filter(|n| n.name.len() >= MIN_NAME)
            .cloned()
            .collect();
        let index = name_index(&long);
        let mut used: BTreeMap<usize, HashMap<&str, bool>> = BTreeMap::new();
        for r in refs.iter().filter(|r| index.contains_key(r.name.as_str())) {
            if let Some(si) = innermost(symbols, r.line) {
                *used.entry(si).or_default().entry(&r.name).or_default() |= r.call;
            }
        }
        for (si, names) in used {
            let s = &symbols[si];
            let is_node = |n: &GraphNode| {
                n.path == path && n.line == s.start_line && n.status != NodeStatus::Removed
            };
            if nodes.iter().any(is_node) {
                continue;
            }
            let mut names: Vec<(&str, bool)> = names.into_iter().collect();
            names.sort();
            let mut me = None;
            for (name, call) in names {
                for to in targets(&index, name, call, path) {
                    let count = self.per_node.entry(to).or_default();
                    if *count >= MAX_PER_NODE {
                        continue;
                    }
                    *count += 1;
                    let caller = *me.get_or_insert_with(|| {
                        self.callers.push(GraphNode {
                            id: 0,
                            name: s.name.clone(),
                            kind: s.kind,
                            status: NodeStatus::Caller,
                            path: path.to_string(),
                            file,
                            line: s.start_line,
                            rows: vec![],
                            parent: parent(symbols, si).map(|p| symbols[p].name.clone()),
                        });
                        self.callers.len() - 1
                    });
                    self.edges.push((caller, to));
                }
            }
        }
    }

    /// Adds the callers found to the graph.
    pub fn finish(self, nodes: &mut Vec<GraphNode>, edges: &mut Vec<GraphEdge>) {
        let base = nodes.len() as u32;
        for (i, mut c) in self.callers.into_iter().enumerate() {
            c.id = base + i as u32;
            nodes.push(c);
        }
        edges.extend(self.edges.into_iter().map(|(c, to)| GraphEdge {
            from: base + c as u32,
            to,
        }));
    }
}
