//! The app side of `lgtm_arch::inputs`: the changes and the symbol graph of a review's files.

use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::{Arc, Mutex};

use lgtm_arch::RangeDto;
pub use lgtm_arch::inputs::*;
use lgtm_arch::model::{Changed, Model, Use};

use crate::diff::{FileStatus, RowKind};
use crate::review::build::ReviewFile;
use crate::review::graph::{self, GraphDto};

/// The last symbol graph, by a hash of its root and files, so reloads after an edit or a
/// toggle skip the repo scan.
static GRAPH: Mutex<Option<(u64, Arc<GraphDto>)>> = Mutex::new(None);

/// The changes a review file makes, with its index when `indexed`.
pub fn changed_of(files: &[ReviewFile], indexed: bool) -> Vec<Changed> {
    files
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let file = indexed.then_some(i as u32);
            let removed = f.status == FileStatus::Removed;
            let lines = f.rows.iter().filter_map(|r| match (r.kind, removed) {
                (RowKind::Added, false) => r.new_line,
                (RowKind::Removed, true) => r.old_line,
                _ => None,
            });
            Changed {
                path: f.path.clone(),
                old_path: f.old_path.clone(),
                status: f.status,
                file,
                ranges: spans(lines)
                    .into_iter()
                    .map(|(start, end)| RangeDto {
                        path: f.path.clone(),
                        file,
                        start,
                        end,
                    })
                    .collect(),
            }
        })
        .collect()
}

/// Adds the symbol graph of `files`, with uses found under `root`.
pub fn add_graph(model: &mut Model, files: &[ReviewFile], root: &Path) {
    if files.is_empty() {
        return;
    }
    let g = symbol_graph(files, root);
    let GraphDto { nodes, edges } = &*g;
    model.uses = edges
        .iter()
        .map(|e| {
            let (f, t) = (&nodes[e.from as usize], &nodes[e.to as usize]);
            Use {
                from_path: f.path.clone(),
                from_changed: f.file.is_some(),
                hop: f.hop,
                to_path: t.path.clone(),
                to_name: t.name.clone(),
                to_line: t.line,
                to_hop: t.hop,
            }
        })
        .collect();
}

fn symbol_graph(files: &[ReviewFile], root: &Path) -> Arc<GraphDto> {
    let mut h = std::hash::DefaultHasher::new();
    root.hash(&mut h);
    for f in files {
        (
            &f.path,
            f.fingerprint,
            &f.old.commit,
            &f.new.commit,
            &f.texts,
        )
            .hash(&mut h);
    }
    let key = h.finish();
    if let Some((k, g)) = &*GRAPH.lock().unwrap()
        && *k == key
    {
        return g.clone();
    }
    let g = Arc::new(graph::graph(files, Some(root)));
    *GRAPH.lock().unwrap() = Some((key, g.clone()));
    g
}
