//! A loaded review: its diff, file sides, anchors, and the marks matched onto its rows.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::anchor::{self, Anchor, Candidate, FileIndex, MarkRef, MatchConfig, Side};
use crate::diff::{FileStatus, Hunk, RowKind};
use crate::jj::{FileDiff, Jj, Range};
use crate::store::{Mark, MarkState, Store};
use crate::syntax::{self, CodeSymbol, Segment};

/// Files larger than this load without contents: no highlighting, symbols, or context.
const MAX_FILE_BYTES: usize = 4 << 20;

pub struct RepoCtx {
    pub root: std::path::PathBuf,
    pub jj: Jj,
    pub store: Mutex<Store>,
    reviews: Mutex<HashMap<u32, Arc<Mutex<Review>>>>,
    next_id: AtomicU32,
    pub match_config: MatchConfig,
}

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RowDto {
    pub kind: RowKind,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub text: String,
    /// Highlight segments; absent when the language is unknown or the file has no contents.
    pub hl: Option<Vec<Segment>>,
}

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HunkDto {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub section: String,
    pub rows: Vec<RowDto>,
}

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileDto {
    pub path: String,
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub binary: bool,
    pub conflict: bool,
    pub old_exec: bool,
    pub new_exec: bool,
    pub hunks: Vec<HunkDto>,
    pub old_symbols: Vec<CodeSymbol>,
    pub new_symbols: Vec<CodeSymbol>,
}

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewDto {
    pub id: u32,
    pub revset: String,
    pub from: Option<String>,
    pub to: String,
    pub files: Vec<FileDto>,
    pub marks: Vec<MarkDto>,
}

/// A row by file index and its index among the file's rows, counting across hunks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
pub struct RowRef {
    pub file: u32,
    pub row: u32,
}

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MarkDto {
    pub file: u32,
    pub row: u32,
    pub state: MarkState,
    pub review: String,
    pub created_at: f64,
}

struct FileSide {
    commit: Option<String>,
    path: String,
    index: Option<FileIndex>,
}

/// A changed row and the anchor a mark made on it would get.
struct Keyed {
    row: u32,
    side: Side,
    line: u32,
    anchor: Anchor,
}

struct ReviewFile {
    status: FileStatus,
    path: String,
    old_path: Option<String>,
    old: FileSide,
    new: FileSide,
    keyed: Vec<Keyed>,
}

/// One undoable change: marks removed and marks added, with their stored ids.
struct Op {
    removed: Vec<Mark>,
    added: Vec<Mark>,
}

pub struct Review {
    revset: String,
    files: Vec<ReviewFile>,
    /// Matched mark per row.
    matched: HashMap<RowRef, Mark>,
    undo: Vec<Op>,
    redo: Vec<Op>,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

impl RepoCtx {
    pub fn new(root: std::path::PathBuf, jj: Jj, store: Store) -> Self {
        Self {
            root,
            jj,
            store: Mutex::new(store),
            reviews: Mutex::default(),
            next_id: AtomicU32::new(1),
            match_config: MatchConfig::default(),
        }
    }

    fn review(&self, id: u32) -> anyhow::Result<Arc<Mutex<Review>>> {
        self.reviews
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .context("review is closed")
    }

    pub fn close(&self, id: u32) {
        self.reviews.lock().unwrap().remove(&id);
    }

    pub async fn open(&self, revset: &str) -> anyhow::Result<ReviewDto> {
        self.jj.snapshot().await?;
        let range = self.jj.resolve(revset).await?;
        let diffs = self.jj.diff(&range).await?;
        let contents = self.load_contents(&range, &diffs).await?;

        let (files, dtos) = tokio::task::spawn_blocking({
            let range = range.clone();
            move || build_files(&range, diffs, contents)
        })
        .await?;

        let mut review = Review {
            revset: revset.to_string(),
            files,
            matched: HashMap::new(),
            undo: vec![],
            redo: vec![],
        };
        let marks = {
            let store = self.store.lock().unwrap();
            review.rematch(&store, self.match_config)?;
            review.mark_dtos()
        };
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.reviews
            .lock()
            .unwrap()
            .insert(id, Arc::new(Mutex::new(review)));
        Ok(ReviewDto {
            id,
            revset: revset.to_string(),
            from: range.from,
            to: range.to,
            files: dtos,
            marks,
        })
    }

    /// Old and new contents per file, `None` where a side is absent, binary, or too large.
    async fn load_contents(
        &self,
        range: &Range,
        diffs: &[FileDiff],
    ) -> anyhow::Result<Vec<(Option<String>, Option<String>)>> {
        let limit = tokio::sync::Semaphore::new(8);
        let load = |commit: Option<&'_ str>, path: &'_ str, present: bool| {
            let limit = &limit;
            let (commit, path) = (commit.map(str::to_string), path.to_string());
            async move {
                if !present {
                    return Ok::<_, anyhow::Error>(None);
                }
                let _permit = limit.acquire().await?;
                let bytes = self.jj.file(commit.as_deref(), &path).await?;
                Ok((bytes.len() <= MAX_FILE_BYTES)
                    .then(|| String::from_utf8(bytes).ok())
                    .flatten())
            }
        };
        futures::future::try_join_all(diffs.iter().map(|d| {
            let old_path = d.old_path.as_deref().unwrap_or(&d.path);
            let old_present = !d.binary && d.status != FileStatus::Added;
            let new_present = !d.binary && d.status != FileStatus::Removed;
            let old = load(range.from.as_deref(), old_path, old_present);
            let new = load(Some(&range.to), &d.path, new_present);
            async move { tokio::try_join!(old, new) }
        }))
        .await
    }

    pub fn mark(
        &self,
        id: u32,
        rows: &[RowRef],
        state: Option<MarkState>,
    ) -> anyhow::Result<Vec<MarkDto>> {
        let review = self.review(id)?;
        let mut review = review.lock().unwrap();
        let mut store = self.store.lock().unwrap();
        let created_at = now_ms();
        let mut op = Op {
            removed: vec![],
            added: vec![],
        };
        let wanted: HashSet<RowRef> = rows.iter().copied().collect();

        for (file_idx, file) in review.files.iter().enumerate() {
            for k in &file.keyed {
                let rref = RowRef {
                    file: file_idx as u32,
                    row: k.row,
                };
                if !wanted.contains(&rref) {
                    continue;
                }
                let existing = review.matched.get(&rref);
                if existing.map(|m| m.state) == state {
                    continue;
                }
                op.removed.extend(existing.cloned());
                let Some(state) = state else { continue };
                let side = if k.side == Side::Old {
                    &file.old
                } else {
                    &file.new
                };
                let Some(commit) = side.commit.clone() else {
                    continue;
                };
                if let Some(index) = &side.index {
                    store.save_line_hashes(&commit, &side.path, &index.hashes)?;
                }
                op.added.push(Mark {
                    id: 0,
                    anchor: k.anchor.clone(),
                    commit,
                    line: k.line,
                    state,
                    review: review.revset.clone(),
                    created_at,
                });
            }
        }
        review.apply(&mut store, op, self.match_config)
    }

    pub fn reset(&self, id: u32) -> anyhow::Result<Vec<MarkDto>> {
        let review = self.review(id)?;
        let mut review = review.lock().unwrap();
        let mut store = self.store.lock().unwrap();
        let removed = store.marks_for_review(&review.revset)?;
        review.apply(
            &mut store,
            Op {
                removed,
                added: vec![],
            },
            self.match_config,
        )
    }

    pub fn undo(&self, id: u32) -> anyhow::Result<Vec<MarkDto>> {
        self.step(id, true)
    }

    pub fn redo(&self, id: u32) -> anyhow::Result<Vec<MarkDto>> {
        self.step(id, false)
    }

    fn step(&self, id: u32, undo: bool) -> anyhow::Result<Vec<MarkDto>> {
        let review = self.review(id)?;
        let mut review = review.lock().unwrap();
        let mut store = self.store.lock().unwrap();
        let popped = if undo {
            review.undo.pop()
        } else {
            review.redo.pop()
        };
        if let Some(op) = popped {
            let (remove, insert) = if undo {
                (&op.added, &op.removed)
            } else {
                (&op.removed, &op.added)
            };
            let ids: Vec<i64> = remove.iter().map(|m| m.id).collect();
            store.replace(&ids, insert)?;
            if undo {
                review.redo.push(op)
            } else {
                review.undo.push(op)
            }
            review.rematch(&store, self.match_config)?;
        }
        Ok(review.mark_dtos())
    }
}

impl Review {
    fn apply(
        &mut self,
        store: &mut Store,
        mut op: Op,
        cfg: MatchConfig,
    ) -> anyhow::Result<Vec<MarkDto>> {
        if !op.removed.is_empty() || !op.added.is_empty() {
            let ids: Vec<i64> = op.removed.iter().map(|m| m.id).collect();
            op.added = store.replace(&ids, &op.added)?;
            self.undo.push(op);
            self.redo.clear();
            self.rematch(store, cfg)?;
        }
        Ok(self.mark_dtos())
    }

    fn mark_dtos(&self) -> Vec<MarkDto> {
        let mut out: Vec<MarkDto> = self
            .matched
            .iter()
            .map(|(r, m)| MarkDto {
                file: r.file,
                row: r.row,
                state: m.state,
                review: m.review.clone(),
                created_at: m.created_at as f64,
            })
            .collect();
        out.sort_by_key(|m| (m.file, m.row));
        out
    }

    /// Matches every stored mark that could land in this review onto its rows.
    fn rematch(&mut self, store: &Store, cfg: MatchConfig) -> anyhow::Result<()> {
        let removed_paths: Vec<&str> = self
            .files
            .iter()
            .filter(|f| f.status == FileStatus::Removed)
            .map(|f| f.path.as_str())
            .collect();
        let mut paths: Vec<&str> = removed_paths.clone();
        for f in &self.files {
            paths.push(&f.path);
            paths.extend(f.old_path.as_deref());
        }
        let marks = store.marks_for_paths(&paths)?;
        let mut remaps: HashMap<(String, String), Option<HashMap<u32, u32>>> = HashMap::new();
        let mut matched = HashMap::new();

        for (file_idx, file) in self.files.iter().enumerate() {
            for side in [Side::Old, Side::New] {
                let keyed: Vec<&Keyed> = file.keyed.iter().filter(|k| k.side == side).collect();
                if keyed.is_empty() {
                    continue;
                }
                let current = if side == Side::Old {
                    &file.old
                } else {
                    &file.new
                };
                let candidates: Vec<Candidate> = keyed
                    .iter()
                    .map(|k| Candidate {
                        id: k.row as usize,
                        line_hash: k.anchor.line_hash,
                        above: k.anchor.above.clone(),
                        below: k.anchor.below.clone(),
                    })
                    .collect();
                let row_by_line: HashMap<u32, usize> =
                    keyed.iter().map(|k| (k.line, k.row as usize)).collect();

                let mut pool_paths = vec![current.path.as_str()];
                if side == Side::New {
                    pool_paths.extend(file.old_path.as_deref());
                }
                let mut pools = vec![pool_paths];
                if side == Side::New && file.status == FileStatus::Added {
                    // Renames past jj's similarity cutoff show up as a delete plus an add.
                    pools.push(removed_paths.clone());
                }

                let mut taken: HashMap<usize, i64> = HashMap::new();
                for pool in pools {
                    let refs: Vec<MarkRef> = marks
                        .iter()
                        .filter(|m| m.anchor.side == side && pool.contains(&m.anchor.path.as_str()))
                        .filter(|m| !taken.values().any(|id| *id == m.id))
                        .map(|m| {
                            let map = remaps
                                .entry((m.commit.clone(), m.anchor.path.clone()))
                                .or_insert_with(|| {
                                    let old =
                                        store.line_hashes(&m.commit, &m.anchor.path).ok()??;
                                    Some(anchor::remap_lines(&old, &current.index.as_ref()?.hashes))
                                });
                            let remap = map
                                .as_ref()
                                .and_then(|map| row_by_line.get(map.get(&m.line)?).copied());
                            MarkRef {
                                id: m.id,
                                anchor: &m.anchor,
                                created_at: m.created_at,
                                remap,
                            }
                        })
                        .collect();
                    let open: Vec<Candidate> = candidates
                        .iter()
                        .filter(|c| !taken.contains_key(&c.id))
                        .cloned()
                        .collect();
                    taken.extend(anchor::match_marks(&refs, &open, cfg));
                }
                let by_id: HashMap<i64, &Mark> = marks.iter().map(|m| (m.id, m)).collect();
                for (row, mark_id) in taken {
                    matched.insert(
                        RowRef {
                            file: file_idx as u32,
                            row: row as u32,
                        },
                        by_id[&mark_id].clone(),
                    );
                }
            }
        }
        self.matched = matched;
        Ok(())
    }
}

/// Builds per-file state and the UI's view of each file. CPU-bound: parsing and highlighting.
fn build_files(
    range: &Range,
    diffs: Vec<FileDiff>,
    contents: Vec<(Option<String>, Option<String>)>,
) -> (Vec<ReviewFile>, Vec<FileDto>) {
    let cfg = MatchConfig::default();
    diffs
        .into_iter()
        .zip(contents)
        .map(|(d, (old_text, new_text))| {
            let old_path = d.old_path.clone().unwrap_or_else(|| d.path.clone());
            let old_norm = anchor::norm_for(&old_path);
            let new_norm = anchor::norm_for(&d.path);
            let old_parsed = old_text
                .as_deref()
                .and_then(|t| syntax::parse(&old_path, t));
            let new_parsed = new_text.as_deref().and_then(|t| syntax::parse(&d.path, t));

            let rows = d.hunks.iter().flat_map(|h| &h.rows);
            let old_lines: Vec<u32> = rows
                .clone()
                .filter(|r| r.kind == RowKind::Removed)
                .filter_map(|r| r.old_line)
                .collect();
            let new_lines: Vec<u32> = rows.clone().filter_map(|r| r.new_line).collect();
            let old_hl = old_parsed
                .as_ref()
                .map(|p| p.highlight(old_text.as_deref().unwrap(), &old_lines));
            let new_hl = new_parsed
                .as_ref()
                .map(|p| p.highlight(new_text.as_deref().unwrap(), &new_lines));

            let old = FileSide {
                commit: range.from.clone(),
                path: old_path.clone(),
                index: old_text.as_deref().map(|t| FileIndex::new(t, old_norm)),
            };
            let new = FileSide {
                commit: Some(range.to.clone()),
                path: d.path.clone(),
                index: new_text.as_deref().map(|t| FileIndex::new(t, new_norm)),
            };

            let keyed = rows
                .enumerate()
                .filter_map(|(i, r)| {
                    let (side, line, fs, norm) = match r.kind {
                        RowKind::Context => return None,
                        RowKind::Removed => (Side::Old, r.old_line?, &old, old_norm),
                        RowKind::Added => (Side::New, r.new_line?, &new, new_norm),
                    };
                    let (above, below) = fs
                        .index
                        .as_ref()
                        .map(|ix| ix.context(line, cfg.window))
                        .unwrap_or_default();
                    let anchor = Anchor {
                        path: fs.path.clone(),
                        side,
                        line_hash: anchor::hash_line(&r.text, norm),
                        above,
                        below,
                    };
                    Some(Keyed {
                        row: i as u32,
                        side,
                        line,
                        anchor,
                    })
                })
                .collect();

            let hunks = d
                .hunks
                .iter()
                .map(|h| hunk_dto(h, old_hl.as_ref(), new_hl.as_ref()))
                .collect();
            let dto = FileDto {
                path: d.path.clone(),
                old_path: d.old_path.clone(),
                status: d.status,
                binary: d.binary,
                conflict: d.conflict,
                old_exec: d.old_exec,
                new_exec: d.new_exec,
                hunks,
                old_symbols: old_parsed
                    .map(|p| p.symbols(old_text.as_deref().unwrap()))
                    .unwrap_or_default(),
                new_symbols: new_parsed
                    .map(|p| p.symbols(new_text.as_deref().unwrap()))
                    .unwrap_or_default(),
            };
            let file = ReviewFile {
                status: d.status,
                path: d.path,
                old_path: d.old_path,
                old,
                new,
                keyed,
            };
            (file, dto)
        })
        .unzip()
}

fn hunk_dto(
    h: &Hunk,
    old_hl: Option<&HashMap<u32, Vec<Segment>>>,
    new_hl: Option<&HashMap<u32, Vec<Segment>>>,
) -> HunkDto {
    let rows = h
        .rows
        .iter()
        .map(|r| {
            let hl = match r.kind {
                RowKind::Removed => old_hl.zip(r.old_line).and_then(|(m, l)| m.get(&l)),
                _ => new_hl.zip(r.new_line).and_then(|(m, l)| m.get(&l)),
            };
            RowDto {
                kind: r.kind,
                old_line: r.old_line,
                new_line: r.new_line,
                text: r.text.clone(),
                hl: hl.cloned(),
            }
        })
        .collect();
    HunkDto {
        old_start: h.old_start,
        old_lines: h.old_lines,
        new_start: h.new_start,
        new_lines: h.new_lines,
        section: h.section.clone(),
        rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::process::Command;

    fn jj(dir: &Path, args: &[&str]) {
        let out = Command::new("jj")
            .arg("-R")
            .arg(dir)
            .args(args)
            .output()
            .expect("jj on PATH");
        assert!(
            out.status.success(),
            "jj {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn repo() -> (tempfile::TempDir, RepoCtx) {
        let dir = tempfile::tempdir().unwrap();
        let out = Command::new("jj")
            .args(["git", "init"])
            .arg(dir.path())
            .output()
            .unwrap();
        assert!(out.status.success());
        jj(dir.path(), &["config", "set", "--repo", "user.name", "t"]);
        jj(
            dir.path(),
            &["config", "set", "--repo", "user.email", "t@t"],
        );
        let ctx = RepoCtx::new(dir.path().into(), Jj::new(dir.path()), Store::in_memory());
        (dir, ctx)
    }

    fn write(dir: &Path, path: &str, text: &str) {
        std::fs::write(dir.join(path), text).unwrap();
    }

    /// (file, text) of each marked row with its state.
    fn marked(review: &ReviewDto, marks: &[MarkDto]) -> Vec<(String, MarkState)> {
        marks
            .iter()
            .map(|m| {
                let f = &review.files[m.file as usize];
                let row = f
                    .hunks
                    .iter()
                    .flat_map(|h| &h.rows)
                    .nth(m.row as usize)
                    .unwrap();
                (row.text.clone(), m.state)
            })
            .collect()
    }

    fn row_of(review: &ReviewDto, file: usize, text: &str, nth: usize) -> RowRef {
        let rows = review.files[file].hunks.iter().flat_map(|h| &h.rows);
        let idx = rows
            .enumerate()
            .filter(|(_, r)| r.text == text && r.kind != RowKind::Context)
            .nth(nth)
            .unwrap_or_else(|| panic!("no changed row {text:?}"))
            .0;
        RowRef {
            file: file as u32,
            row: idx as u32,
        }
    }

    #[tokio::test]
    async fn marks_follow_code_through_edits_and_rewrites() {
        let (dir, ctx) = repo();
        let d = dir.path();
        write(
            d,
            "a.rs",
            "fn a() {\n    x();\n}\n\nfn b() {\n    x();\n}\n",
        );
        jj(d, &["describe", "-m", "add a"]);

        let review = ctx.open("@").await.unwrap();
        let in_b = row_of(&review, 0, "    x();", 1);
        let marks = ctx
            .mark(review.id, &[in_b], Some(MarkState::Flagged))
            .unwrap();
        assert_eq!(
            marked(&review, &marks),
            [("    x();".into(), MarkState::Flagged)]
        );

        // Insert a function above both and reformat: the mark stays on the copy in `b`.
        write(
            d,
            "a.rs",
            "fn z() {}\n\nfn a() {\n    x();\n}\n\nfn b() {\n      x();\n}\n",
        );
        let review = ctx.open("@").await.unwrap();
        let b_row = row_of(&review, 0, "      x();", 0);
        assert_eq!(review.marks.len(), 1);
        assert_eq!(
            (review.marks[0].file, review.marks[0].row),
            (b_row.file, b_row.row)
        );

        // A new commit on top doesn't change the reviewed stack's code.
        jj(d, &["new", "-m", "next"]);
        let review = ctx.open("@-").await.unwrap();
        assert_eq!(review.marks.len(), 1);
    }

    #[tokio::test]
    async fn undo_and_redo_step_through_mark_changes() {
        let (dir, ctx) = repo();
        let d = dir.path();
        write(d, "a.py", "def f():\n    return 1\n");
        let review = ctx.open("@").await.unwrap();
        let row = row_of(&review, 0, "    return 1", 0);
        ctx.mark(review.id, &[row], Some(MarkState::Reviewed))
            .unwrap();
        let marks = ctx
            .mark(review.id, &[row], Some(MarkState::Skipped))
            .unwrap();
        assert_eq!(marks[0].state, MarkState::Skipped);

        assert_eq!(ctx.undo(review.id).unwrap()[0].state, MarkState::Reviewed);
        assert!(ctx.undo(review.id).unwrap().is_empty());
        assert_eq!(ctx.redo(review.id).unwrap()[0].state, MarkState::Reviewed);
    }

    #[tokio::test]
    async fn marks_survive_a_rename_added_to_the_reviewed_change() {
        let (dir, ctx) = repo();
        let d = dir.path();
        write(d, "old.py", "def f():\n    return 1\n");
        jj(d, &["new", "-m", "change"]);
        write(d, "old.py", "def f():\n    log()\n    return 1\n");
        let review = ctx.open("@").await.unwrap();
        let row = row_of(&review, 0, "    log()", 0);
        ctx.mark(review.id, &[row], Some(MarkState::Reviewed))
            .unwrap();

        std::fs::rename(d.join("old.py"), d.join("new.py")).unwrap();
        let review = ctx.open("@").await.unwrap();
        assert_eq!(review.files[0].old_path.as_deref(), Some("old.py"));
        assert_eq!(
            marked(&review, &review.marks),
            [("    log()".into(), MarkState::Reviewed)]
        );
    }

    #[tokio::test]
    async fn marks_follow_a_heavily_edited_rename_reported_as_delete_and_add() {
        let (dir, ctx) = repo();
        let d = dir.path();
        write(d, "old.rs", "fn a() {}\nfn b() {}\nfn c() {}\n");
        jj(d, &["new", "-m", "change"]);
        write(
            d,
            "old.rs",
            "fn a() {}\nfn b() {}\nfn c() {}\nlet marked = 1;\n",
        );
        let review = ctx.open("@").await.unwrap();
        let file = review
            .files
            .iter()
            .position(|f| f.path == "old.rs")
            .unwrap();
        let row = row_of(&review, file, "let marked = 1;", 0);
        ctx.mark(review.id, &[row], Some(MarkState::Flagged))
            .unwrap();

        std::fs::remove_file(d.join("old.rs")).unwrap();
        write(
            d,
            "new.rs",
            "fn x() {}\nfn y() {}\nfn z() {}\nfn w() {}\nlet marked = 1;\n",
        );
        let review = ctx.open("@").await.unwrap();
        assert!(review.files.iter().all(|f| f.old_path.is_none()));
        assert_eq!(
            marked(&review, &review.marks),
            [("let marked = 1;".into(), MarkState::Flagged)]
        );
    }

    #[tokio::test]
    async fn reapplying_same_state_is_a_no_op_and_reset_removes_review_marks() {
        let (dir, ctx) = repo();
        let d = dir.path();
        write(d, "a.go", "package a\n\nfunc F() {}\n");
        let review = ctx.open("@").await.unwrap();
        let row = row_of(&review, 0, "func F() {}", 0);
        ctx.mark(review.id, &[row], Some(MarkState::Reviewed))
            .unwrap();
        ctx.mark(review.id, &[row], Some(MarkState::Reviewed))
            .unwrap();
        // One undo step undoes the only real change.
        assert!(ctx.undo(review.id).unwrap().is_empty());
        ctx.redo(review.id).unwrap();
        assert!(ctx.reset(review.id).unwrap().is_empty());
    }

    #[tokio::test]
    async fn review_with_two_heads_is_rejected() {
        let (dir, ctx) = repo();
        let d = dir.path();
        jj(d, &["new", "root()", "-m", "a"]);
        jj(d, &["new", "root()", "-m", "b"]);
        let err = ctx.open("root()+").await.err().unwrap();
        assert!(format!("{err:#}").contains("2 heads"), "{err:#}");
    }
}
