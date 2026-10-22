//! References to a name and symbols by name, from the tree-sitter parse of each file.

use std::path::Path;

use serde::Serialize;
use specta::Type;

use super::{Hit, Hits, MAX_HITS, clip, files, read, spans};
use crate::syntax::{self, SymbolKind};

/// Every place `name` appears as an identifier under `root`. Lines that define it come
/// first, then the rest by path and line.
pub fn references(root: &Path, name: &str) -> Hits {
    if name.is_empty() {
        return Hits { hits: vec![], truncated: false };
    }
    let targets: Vec<_> = files(root)
        .into_iter()
        .filter(|(_, rel)| syntax::has_symbols(rel))
        .collect();
    let per_file = crate::par::map(&targets, |(abs, rel)| {
        let text = read(abs)?;
        if !text.contains(name) {
            return None;
        }
        Some(file_refs(rel, &text, name))
    });
    let mut hits: Vec<Hit> = per_file.into_iter().flatten().flatten().collect();
    let truncated = hits.len() > MAX_HITS;
    hits.truncate(MAX_HITS);
    hits.sort_by_key(|h| !h.def);
    Hits { hits, truncated }
}

fn file_refs(rel: &str, text: &str, name: &str) -> Vec<Hit> {
    let Some(parsed) = syntax::parse(rel, text) else {
        return vec![];
    };
    let mut lines: Vec<u32> = parsed
        .refs(text)
        .into_iter()
        .filter(|r| r.name == name)
        .map(|r| r.line)
        .collect();
    lines.dedup();
    // A symbol's range starts at its doc comments and attributes, so its definition is the
    // first line in it that names it.
    let defs: Vec<u32> = parsed
        .symbols(text)
        .into_iter()
        .filter(|s| s.name == name)
        .filter_map(|s| {
            lines
                .iter()
                .copied()
                .find(|l| (s.start_line..=s.end_line).contains(l))
        })
        .collect();
    let source: Vec<&str> = text.lines().collect();
    lines
        .into_iter()
        .filter_map(|line| {
            let s = source.get(line as usize - 1)?;
            Some(Hit {
                path: rel.to_string(),
                line,
                text: clip(s).to_string(),
                spans: spans(s, word_ranges(s, name)),
                def: defs.contains(&line),
                before: (line > 1)
                    .then(|| source.get(line as usize - 2).map(|l| clip(l).to_string()))
                    .flatten(),
                after: source.get(line as usize).map(|l| clip(l).to_string()),
            })
        })
        .collect()
}

/// Where `name` appears in `line` as a whole word.
fn word_ranges(line: &str, name: &str) -> Vec<(usize, usize)> {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    line.match_indices(name)
        .filter(|(i, _)| {
            let before = line[..*i].chars().next_back();
            let after = line[i + name.len()..].chars().next();
            !before.is_some_and(word) && !after.is_some_and(word)
        })
        .map(|(i, _)| (i, i + name.len()))
        .collect()
}

/// Where `name` is defined in `text`: the line that names it, its kind, and its code.
pub fn definition(rel: &str, text: &str, name: &str) -> Option<(u32, SymbolKind, String)> {
    let parsed = syntax::parse(rel, text)?;
    let lines: Vec<u32> = parsed
        .refs(text)
        .into_iter()
        .filter(|r| r.name == name)
        .map(|r| r.line)
        .collect();
    let source: Vec<&str> = text.lines().collect();
    parsed
        .symbols(text)
        .into_iter()
        .filter(|s| s.name == name)
        .find_map(|s| {
            let line = lines
                .iter()
                .copied()
                .find(|l| (s.start_line..=s.end_line).contains(l))?;
            let code = source
                .get(s.start_line as usize - 1..(s.end_line as usize).min(source.len()))?
                .join("\n");
            Some((line, s.kind, code))
        })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SymbolHit {
    pub name: String,
    pub kind: SymbolKind,
    pub path: String,
    /// The line it starts on, 1-based.
    pub line: u32,
}

/// Symbols under `root` whose names match `query` as a subsequence, ignoring case, best
/// first: exact names, then prefixes, then shorter names.
pub fn symbols(root: &Path, query: &str, limit: usize) -> Vec<SymbolHit> {
    let q = query.to_lowercase();
    if q.is_empty() {
        return vec![];
    }
    let targets: Vec<_> = files(root)
        .into_iter()
        .filter(|(_, rel)| syntax::has_symbols(rel))
        .collect();
    let per_file = crate::par::map(&targets, |(abs, rel)| {
        let text = read(abs)?;
        let parsed = syntax::parse(rel, &text)?;
        let found: Vec<_> = parsed
            .symbols(&text)
            .into_iter()
            .filter_map(|s| {
                let score = score(&s.name.to_lowercase(), &q)?;
                Some((
                    score,
                    SymbolHit {
                        name: s.name,
                        kind: s.kind,
                        path: rel.clone(),
                        line: s.start_line,
                    },
                ))
            })
            .collect();
        Some(found)
    });
    let mut all: Vec<_> = per_file.into_iter().flatten().flatten().collect();
    all.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.name.cmp(&b.1.name)));
    all.into_iter().take(limit).map(|(_, s)| s).collect()
}

/// Lower is better; none when `q` isn't a subsequence of `name`.
fn score(name: &str, q: &str) -> Option<(u8, usize)> {
    let mut rest = name.chars();
    if !q.chars().all(|c| rest.any(|n| n == c)) {
        return None;
    }
    let tier = if name == q {
        0
    } else if name.starts_with(q) {
        1
    } else if name.contains(q) {
        2
    } else {
        3
    };
    Some((tier, name.len()))
}
