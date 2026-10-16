//! The workspace's working copy: its files, their imports, and its manifests. Imports are
//! cached per file by modification time, so a reload only re-reads what changed.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::SystemTime;

use super::extract::{self, Raw};
use super::roots::{Manifests, is_manifest};
use crate::review::graph::scan::{MAX_BYTES, MAX_FILES};

pub struct WorkingCopy {
    /// Every file, other than skipped ones.
    pub paths: BTreeSet<String>,
    /// Source files' imports.
    pub imports: HashMap<String, Arc<Vec<Raw>>>,
    pub manifests: Manifests,
}

type Stamp = (SystemTime, u64);

type Cache = Mutex<HashMap<PathBuf, (Stamp, Arc<Vec<Raw>>)>>;

static CACHE: LazyLock<Cache> = LazyLock::new(Mutex::default);

/// Past this many entries the cache starts over.
const MAX_CACHED: usize = 100_000;

/// Scans `root`, leaving out `skip` (repo-relative paths) and anything ignored. Manifests are
/// read even when skipped.
pub fn scan(root: &Path, skip: &HashSet<String>) -> WorkingCopy {
    // Only the workspace's own ignore files: its repo may ignore the folder it sits in.
    let walk = ignore::WalkBuilder::new(root)
        .parents(false)
        .require_git(false)
        .filter_entry(|e| e.file_name() != ".jj")
        .build();
    let mut wc = WorkingCopy {
        paths: BTreeSet::new(),
        imports: HashMap::new(),
        manifests: Manifests::default(),
    };
    let mut sources = Vec::new();
    for entry in walk.flatten().take(MAX_FILES) {
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let Ok(rel) = entry.path().strip_prefix(root) else {
            continue;
        };
        let rel = rel.to_string_lossy().replace('\\', "/");
        if is_manifest(&rel)
            && let Ok(text) = std::fs::read_to_string(entry.path())
        {
            wc.manifests.0.insert(rel.clone(), text);
        }
        if skip.contains(&rel) {
            continue;
        }
        if extract::is_source(&rel)
            && let Ok(meta) = entry.metadata()
            && meta.len() <= MAX_BYTES
        {
            sources.push((entry.into_path(), rel.clone(), stamp(&meta)));
        }
        wc.paths.insert(rel);
    }
    let raws = crate::par::map(&sources, |(abs, rel, stamp)| imports(abs, rel, *stamp));
    for ((_, rel, _), raw) in sources.into_iter().zip(raws) {
        if let Some(raw) = raw {
            wc.imports.insert(rel, raw);
        }
    }
    wc
}

fn stamp(meta: &std::fs::Metadata) -> Stamp {
    (
        meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        meta.len(),
    )
}

fn imports(abs: &Path, rel: &str, stamp: Stamp) -> Option<Arc<Vec<Raw>>> {
    if let Some((s, raw)) = CACHE.lock().unwrap().get(abs)
        && *s == stamp
    {
        return Some(raw.clone());
    }
    let text = std::fs::read_to_string(abs).ok()?;
    let raw = extract::imports(rel, &text);
    let mut cache = CACHE.lock().unwrap();
    if cache.len() >= MAX_CACHED {
        cache.clear();
    }
    cache.insert(abs.to_path_buf(), (stamp, raw.clone()));
    Some(raw)
}
