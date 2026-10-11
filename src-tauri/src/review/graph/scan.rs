//! Finds files outside the review that mention a changed symbol's name.

use std::collections::HashSet;
use std::path::Path;

use crate::syntax::{self, CodeSymbol, Ref};

/// Past these the scan stops, so a huge repo can't stall the graph.
const MAX_FILES: usize = 20_000;
const MAX_BYTES: u64 = 512 * 1024;

pub(super) struct Scanned {
    pub path: String,
    pub symbols: Vec<CodeSymbol>,
    pub refs: Vec<Ref>,
}

/// Parsed files under `root` that mention any of `names`, skipping `skip` (paths relative
/// to `root`) and anything ignored.
pub(super) fn mentioning(
    root: &Path,
    skip: &HashSet<&str>,
    names: &HashSet<String>,
) -> Vec<Scanned> {
    if names.is_empty() {
        return vec![];
    }
    let walk = ignore::WalkBuilder::new(root)
        .require_git(false)
        .filter_entry(|e| e.file_name() != ".jj")
        .build();
    let mut out = Vec::new();
    for entry in walk.flatten().take(MAX_FILES) {
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let Ok(rel) = entry.path().strip_prefix(root) else {
            continue;
        };
        let rel = rel.to_string_lossy().replace('\\', "/");
        if skip.contains(rel.as_str())
            || !syntax::has_symbols(&rel)
            || entry.metadata().map_or(true, |m| m.len() > MAX_BYTES)
        {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        if !names.iter().any(|n| text.contains(n.as_str())) {
            continue;
        }
        let Some(parsed) = syntax::parse(&rel, &text) else {
            continue;
        };
        out.push(Scanned {
            symbols: parsed.symbols(&text),
            refs: parsed.refs(&text),
            path: rel,
        });
    }
    out
}
