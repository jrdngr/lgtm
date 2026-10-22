//! Gathering a model: the changed files' imports at both ends of a range, the working copy's
//! for the rest, and the symbol graph's uses.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::{Arc, LazyLock, Mutex};

use super::RangeDto;
use super::extract::{self, Raw};
use super::model::{Changed, Import, Model, Snapshot, Use};
use super::resolve::Resolver;
use super::roots::{self, Manifests, Root};
use super::scan;
use crate::diff::{FileStatus, RowKind};
use crate::jj::{DiffPath, Jj, Range};
use crate::review::build::ReviewFile;
use crate::review::graph::{self, GraphDto};

/// Past this many entries the cache starts over.
const MAX_CACHED: usize = 50_000;

/// Imports by (commit, path).
type Cache = Mutex<HashMap<(String, String), Arc<Vec<Raw>>>>;

static CACHE: LazyLock<Cache> = LazyLock::new(Mutex::default);

/// The last symbol graph, by a hash of its root and files, so reloads after an edit or a
/// toggle skip the repo scan.
static GRAPH: Mutex<Option<(u64, Arc<GraphDto>)>> = Mutex::new(None);

pub struct Built {
    pub model: Model,
    pub roots: Vec<Root>,
}

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

pub fn changed_of_paths(diffs: &[DiffPath]) -> Vec<Changed> {
    diffs
        .iter()
        .map(|d| Changed {
            path: d.path.clone(),
            old_path: d.old_path.clone(),
            status: d.status,
            file: None,
            ranges: vec![],
        })
        .collect()
}

/// Runs of consecutive lines.
fn spans(lines: impl Iterator<Item = u32>) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = Vec::new();
    for l in lines {
        match out.last_mut() {
            Some((_, end)) if *end + 1 == l => *end = l,
            _ => out.push((l, l)),
        }
    }
    out
}

/// The model of `range`, whose changes are `changed`. `extra` files are unchanged in the
/// range but differ in the working copy, so they're read at `range.to`.
pub async fn build(
    jj: &Jj,
    root: &Path,
    repo: &str,
    range: &Range,
    changed: Vec<Changed>,
    extra: Vec<String>,
) -> anyhow::Result<Built> {
    let mut skip: HashSet<String> = extra.iter().cloned().collect();
    for c in &changed {
        skip.insert(c.path.clone());
        skip.extend(c.old_path.clone());
    }
    let wc = {
        let root = root.to_path_buf();
        tokio::task::spawn_blocking(move || scan::scan(&root, &skip))
    };
    // (side is new, commit, path, may be absent)
    let mut reads: Vec<(bool, Option<String>, String, bool)> = Vec::new();
    for c in &changed {
        let old = c.old_path.clone().unwrap_or_else(|| c.path.clone());
        if c.status != FileStatus::Added {
            reads.push((false, range.from.clone(), old, false));
        }
        if c.status != FileStatus::Removed {
            reads.push((true, Some(range.to.clone()), c.path.clone(), false));
        }
    }
    for p in &extra {
        reads.push((false, Some(range.to.clone()), p.clone(), true));
        reads.push((true, Some(range.to.clone()), p.clone(), true));
    }
    let repo = repo.to_string();
    let limit = tokio::sync::Semaphore::new(8);
    let read =
        futures::future::try_join_all(reads.iter().map(|(_, commit, path, optional)| async {
            let _permit = limit.acquire().await?;
            match imports_at(jj, commit.as_deref(), path).await {
                Ok(raw) => Ok(Some(raw)),
                // A file the review changes elsewhere may not exist at this commit.
                Err(_) if *optional => Ok(None),
                Err(e) => Err(e),
            }
        }));
    let (wc, raws) = tokio::join!(wc, read);
    let (wc, raws) = (wc?, raws?);
    Ok(tokio::task::spawn_blocking(move || {
        let mut base = (wc.paths.clone(), wc.imports.clone());
        let mut head = (wc.paths, wc.imports);
        for ((new, _, path, _), raw) in reads.into_iter().zip(raws) {
            let side = if new { &mut head } else { &mut base };
            let Some(raw) = raw else { continue };
            side.0.insert(path.clone());
            if let Some(raw) = raw {
                side.1.insert(path, raw);
            }
        }
        for c in &changed {
            // Non-source files aren't read, but still exist for resolving and roots.
            match c.status {
                FileStatus::Added => head.0.insert(c.path.clone()),
                FileStatus::Removed => base.0.insert(c.path.clone()),
                _ => {
                    base.0
                        .insert(c.old_path.clone().unwrap_or_else(|| c.path.clone()));
                    head.0.insert(c.path.clone())
                }
            };
            if c.status == FileStatus::Copied
                && let Some(old) = &c.old_path
            {
                head.0.insert(old.clone());
            }
        }
        let all: BTreeSet<String> = base.0.union(&head.0).cloned().collect();
        Built {
            roots: roots::detect(&all, &wc.manifests, &repo),
            model: Model {
                base: snapshot(&base.0, &base.1, &wc.manifests),
                head: snapshot(&head.0, &head.1, &wc.manifests),
                changed,
                ..Model::default()
            },
        }
    })
    .await?)
}

/// A file's imports at `commit`, `None` when it isn't source or is too large.
async fn imports_at(
    jj: &Jj,
    commit: Option<&str>,
    path: &str,
) -> anyhow::Result<Option<Arc<Vec<Raw>>>> {
    if !extract::is_source(path) {
        return Ok(None);
    }
    let Some(commit) = commit else {
        return Ok(Some(Arc::default()));
    };
    let key = (commit.to_string(), path.to_string());
    if let Some(hit) = CACHE.lock().unwrap().get(&key) {
        return Ok(Some(hit.clone()));
    }
    let bytes = jj.file(Some(commit), path).await?;
    if bytes.len() as u64 > crate::review::graph::scan::MAX_BYTES {
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&bytes);
    let raw = extract::imports(path, &text);
    let mut cache = CACHE.lock().unwrap();
    if cache.len() >= MAX_CACHED {
        cache.clear();
    }
    cache.insert(key, raw.clone());
    Ok(Some(raw))
}

pub(super) fn snapshot(
    paths: &BTreeSet<String>,
    raws: &HashMap<String, Arc<Vec<Raw>>>,
    manifests: &Manifests,
) -> Snapshot {
    let r = Resolver::new(paths, manifests);
    paths
        .iter()
        .filter(|p| extract::is_source(p))
        .map(|p| {
            let mut imports: Vec<Import> = raws
                .get(p)
                .into_iter()
                .flat_map(|raw| raw.iter())
                .flat_map(|raw| {
                    r.resolve(p, raw)
                        .into_iter()
                        .map(|to| Import { to, line: raw.line })
                })
                .collect();
            imports.sort_by(|a, b| (&a.to, a.line).cmp(&(&b.to, b.line)));
            imports.dedup();
            (p.clone(), imports)
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
