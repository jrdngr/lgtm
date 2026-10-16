//! Finds files outside the review that mention a changed symbol's name.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::SystemTime;

use aho_corasick::AhoCorasick;

use crate::syntax::{self, CodeSymbol, Ref};

/// Past these the scan stops, so a huge repo can't stall the graph.
pub(in crate::review) const MAX_FILES: usize = 20_000;
pub(in crate::review) const MAX_BYTES: u64 = 512 * 1024;

pub(super) struct Scanned {
    pub path: String,
    pub symbols: Vec<CodeSymbol>,
    pub refs: Vec<Ref>,
}

type Stamp = (SystemTime, u64);

/// A file's symbols and refs, by its path and the stamp they were read at.
type Cache = Mutex<HashMap<PathBuf, (Stamp, Arc<Scanned>)>>;

static CACHE: LazyLock<Cache> = LazyLock::new(Mutex::default);

/// Past this many entries the cache starts over.
const MAX_CACHED: usize = 50_000;

/// Files under a root that could have symbols, outside the review.
pub(super) struct RepoFiles(Vec<(PathBuf, String, Stamp)>);

impl RepoFiles {
    /// Lists `root`'s files with symbols, skipping `skip` (paths relative to `root`) and
    /// anything ignored.
    pub fn walk(root: &Path, skip: &HashSet<&str>) -> Self {
        // Only the workspace's own ignore files: its repo may ignore the folder it sits in.
        let walk = ignore::WalkBuilder::new(root)
            .parents(false)
            .require_git(false)
            .filter_entry(|e| e.file_name() != ".jj")
            .build();
        let mut files = Vec::new();
        for entry in walk.flatten().take(MAX_FILES) {
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let Ok(rel) = entry.path().strip_prefix(root) else {
                continue;
            };
            let rel = rel.to_string_lossy().replace('\\', "/");
            if skip.contains(rel.as_str()) || !syntax::has_symbols(&rel) {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            if meta.len() > MAX_BYTES {
                continue;
            }
            let stamp = (
                meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                meta.len(),
            );
            files.push((entry.into_path(), rel, stamp));
        }
        Self(files)
    }

    /// The files that mention any of `names`, parsed.
    pub fn mentioning(&self, names: &HashSet<String>) -> Vec<Arc<Scanned>> {
        if names.is_empty() {
            return vec![];
        }
        let Ok(find) = AhoCorasick::new(names) else {
            return vec![];
        };
        crate::par::map(&self.0, |(abs, rel, stamp)| {
            let text = std::fs::read_to_string(abs).ok()?;
            if !find.is_match(&text) {
                return None;
            }
            parsed(abs, rel, *stamp, &text)
        })
        .into_iter()
        .flatten()
        .collect()
    }
}

fn parsed(abs: &Path, rel: &str, stamp: Stamp, text: &str) -> Option<Arc<Scanned>> {
    if let Some((s, hit)) = CACHE.lock().unwrap().get(abs)
        && *s == stamp
    {
        return Some(hit.clone());
    }
    let parsed = syntax::parse(rel, text)?;
    let scanned = Arc::new(Scanned {
        symbols: parsed.symbols(text),
        refs: parsed.refs(text),
        path: rel.to_string(),
    });
    let mut cache = CACHE.lock().unwrap();
    if cache.len() >= MAX_CACHED {
        cache.clear();
    }
    cache.insert(abs.to_path_buf(), (stamp, scanned.clone()));
    Some(scanned)
}
