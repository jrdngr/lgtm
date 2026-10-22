//! Searching a workspace's working copy: text, references to a name, symbols and file paths.

mod refs;
mod text;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use serde::Serialize;
use specta::Type;

pub use refs::{SymbolHit, definition, references, symbols};
pub use text::{TextQuery, replace, search};

/// Past these a search stops, so a huge repo can't stall it.
const MAX_FILES: usize = 50_000;
const MAX_BYTES: u64 = 1024 * 1024;
const MAX_HITS: usize = 5_000;
/// Longer lines are cut, so a minified file can't flood the results.
const MAX_LINE: usize = 400;

/// A line with matches in it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub path: String,
    /// 1-based.
    pub line: u32,
    pub text: String,
    /// Match ranges in `text`, in UTF-16 code units.
    pub spans: Vec<(u32, u32)>,
    /// The line defines the symbol searched for.
    pub def: bool,
    /// The lines around it, for references.
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Hits {
    pub hits: Vec<Hit>,
    /// The search stopped at its limit, so there are more.
    pub truncated: bool,
}

/// The workspace's files, relative and `/`-separated, skipping what its own ignore files
/// ignore. Ignore files above the root don't count: its repo may ignore the folder it sits in.
pub fn files(root: &Path) -> Vec<(PathBuf, String)> {
    let walk = ignore::WalkBuilder::new(root)
        .hidden(false)
        .parents(false)
        .require_git(false)
        .filter_entry(|e| e.file_name() != ".jj" && e.file_name() != ".git")
        .build();
    let mut out = Vec::new();
    for entry in walk.flatten() {
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let Ok(rel) = entry.path().strip_prefix(root) else {
            continue;
        };
        let rel = rel.to_string_lossy().replace('\\', "/");
        out.push((entry.into_path(), rel));
        if out.len() >= MAX_FILES {
            break;
        }
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

/// A file's text, unless it's too big or not UTF-8.
pub(crate) fn read(abs: &Path) -> Option<String> {
    if std::fs::metadata(abs).ok()?.len() > MAX_BYTES {
        return None;
    }
    std::fs::read_to_string(abs).ok()
}

/// A line cut to `MAX_LINE` characters.
fn clip(line: &str) -> &str {
    match line.char_indices().nth(MAX_LINE) {
        Some((i, _)) => &line[..i],
        None => line,
    }
}

fn utf16(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

/// `ranges` (byte ranges in `line`) as UTF-16 spans of its clipped text.
fn spans(line: &str, ranges: impl IntoIterator<Item = (usize, usize)>) -> Vec<(u32, u32)> {
    let shown = clip(line).len();
    ranges
        .into_iter()
        .filter(|&(s, _)| s < shown)
        .map(|(s, e)| (utf16(&line[..s]), utf16(&line[..e.min(shown)])))
        .collect()
}
