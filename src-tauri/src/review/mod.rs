//! A loaded review: its diff, file sides, anchors, and the marks and checks matched onto it.

mod build;
mod cache;
#[cfg(test)]
mod comment_tests;
mod comments;
mod matching;
mod progress;
mod rowmap;
#[cfg(test)]
pub(crate) mod tests;
mod views;
#[cfg(test)]
mod workflow_tests;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::anchor::{MatchConfig, Side};
use crate::config::Config;
use crate::diff::FileStatus;
use crate::jj::{FileDiff, Jj};
use crate::store::{Edit, FileCheck, Mark, MarkState, Session, Store};
use crate::watch;
use crate::workspace::Workspace;
use build::{FileDto, ReviewFile, Sides};
pub use comments::Ask;
use comments::CommentDto;
use matching::MatchedCheck;
pub use progress::ProgressDto;
pub use rowmap::RowMove;
pub use views::{ChangeDto, View};

/// Files larger than this load without contents: no highlighting, symbols, or context.
const MAX_FILE_BYTES: usize = 4 << 20;

/// Reports a workspace's watch events, with the workspace's name.
pub type Emit = Arc<dyn Fn(Option<String>, watch::Event) + Send + Sync>;

pub struct RepoCtx {
    /// The main workspace's root.
    pub root: PathBuf,
    pub main: Arc<Workspace>,
    /// The other workspaces, by name.
    pub(crate) workspaces: Mutex<HashMap<String, Arc<Workspace>>>,
    /// Set once the window exists. Workspaces start being watched from then on.
    pub emit: Mutex<Option<Emit>>,
    /// Where new workspaces go.
    pub workspace_base: Option<PathBuf>,
    pub store: Mutex<Store>,
    reviews: Mutex<HashMap<u32, Arc<Mutex<Review>>>>,
    next_id: AtomicU32,
    cache: Mutex<cache::FileCache>,
}

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewDto {
    pub id: u32,
    pub revset: String,
    pub view: View,
    pub from: Option<String>,
    pub to: String,
    /// True when the view is an interdiff, whose old side is `from` rebased, not `from`.
    pub interdiff: bool,
    pub files: Vec<FileDto>,
    pub state: StateDto,
    /// The review's changes, oldest first.
    pub changes: Vec<ChangeDto>,
    /// Changes in the previous session that aren't in the review any more.
    pub removed_changes: Vec<ChangeDto>,
    /// When the previous session started (Unix ms), if there was one.
    pub previous_session: Option<f64>,
    pub stale: Option<String>,
    /// Whether the revset includes the workspace's `@`, where an agent's changes land.
    pub has_working_copy: bool,
}

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RefreshDto {
    pub review: ReviewDto,
    /// Where rows of the review as it was went.
    pub moves: Vec<RowMove>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CheckState {
    Unchecked,
    Checked,
    /// Was checked, but the file now has changed rows the check didn't cover.
    ChangedSinceReviewed,
}

/// Everything marking and commenting can change.
#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StateDto {
    pub marks: Vec<MarkDto>,
    /// Per file.
    pub checks: Vec<CheckState>,
    /// Unreviewed rows that replaced a line whose mark was lost, or that a checked file
    /// didn't have when it was checked.
    pub amber: Vec<RowRef>,
    /// Comments on the review's files, in file and row order. Ones whose code is gone last.
    pub comments: Vec<CommentDto>,
}

pub struct Review {
    ws: Arc<Workspace>,
    revset: String,
    /// The source review recorded on marks and checks made here.
    label: String,
    view: View,
    cfg: MatchConfig,
    files: Vec<ReviewFile>,
    /// Matched mark per row.
    matched: HashMap<RowRef, Mark>,
    /// Matched check per file.
    checks: Vec<Option<MatchedCheck>>,
    amber: Vec<RowRef>,
    comments: Vec<CommentDto>,
    /// Marks this review has matched, in this session or the previous one. One that stops
    /// matching was lost, and the rows that replaced its line turn amber.
    baseline: HashSet<i64>,
    session: i64,
    /// The commits of the review's changes as last loaded, oldest first.
    commits: Vec<String>,
    prev: Option<Session>,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
}

/// A freshly loaded diff, before it's swapped into a review.
struct Loaded {
    files: Vec<ReviewFile>,
    dto: ReviewDto,
    commits: Vec<String>,
    cfg: MatchConfig,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

impl RepoCtx {
    pub fn new(root: PathBuf, jj: Jj, store: Store) -> Self {
        Self {
            main: Arc::new(Workspace::new(None, root.clone(), jj)),
            root,
            workspaces: Mutex::default(),
            emit: Mutex::default(),
            workspace_base: crate::workspaces::ws_base(),
            store: Mutex::new(store),
            reviews: Mutex::default(),
            next_id: AtomicU32::new(1),
            cache: Mutex::default(),
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

    /// Opens `revset` in the Combined view. Starts a review session, or with `resume`
    /// continues the revset's latest one, as when the same revset is loaded again.
    pub async fn open(
        &self,
        workspace: Option<&str>,
        revset: &str,
        resume: bool,
    ) -> anyhow::Result<ReviewDto> {
        let ws = self.workspace(workspace).await?;
        ws.snapshot().await?;
        let label = ws.label(revset);
        let (prev, session, seen) = {
            let store = self.store.lock().unwrap();
            match store.last_session(&label)?.filter(|_| resume) {
                Some(last) => (store.session_before(&label, last.id)?, last.id, last.seen),
                None => (
                    store.last_session(&label)?,
                    store.start_session(&label, ws.key(), revset, now_ms())?,
                    vec![],
                ),
            }
        };
        let loaded = self
            .load(&ws, revset, &View::Combined, prev.as_ref())
            .await?;
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut review = Review::new(ws, revset, label, loaded.cfg);
        review.baseline = prev
            .iter()
            .flat_map(|p| p.seen.iter().copied())
            .chain(seen)
            .collect();
        review.session = session;
        review.prev = prev;
        let dto = {
            let store = self.store.lock().unwrap();
            review.swap_in(&store, loaded, id)?.0
        };
        self.reviews
            .lock()
            .unwrap()
            .insert(id, Arc::new(Mutex::new(review)));
        Ok(dto)
    }

    /// Reloads a review after the repo changed, or in another view, keeping its marks' undo
    /// history.
    pub async fn refresh(&self, id: u32, view: Option<View>) -> anyhow::Result<RefreshDto> {
        let (ws, revset, view, prev) = {
            let review = self.review(id)?;
            let review = review.lock().unwrap();
            let view = view.unwrap_or_else(|| review.view.clone());
            (
                review.ws.clone(),
                review.revset.clone(),
                view,
                review.prev.clone(),
            )
        };
        let loaded = self.load(&ws, &revset, &view, prev.as_ref()).await?;
        let review = self.review(id)?;
        let mut review = review.lock().unwrap();
        review.view = view;
        let store = self.store.lock().unwrap();
        let (review, moves) = review.swap_in(&store, loaded, id)?;
        Ok(RefreshDto { review, moves })
    }

    async fn load(
        &self,
        ws: &Workspace,
        revset: &str,
        view: &View,
        prev: Option<&Session>,
    ) -> anyhow::Result<Loaded> {
        let config = Config::load(&ws.root)?;
        let stack = views::stack(&ws.jj, revset, prev).await?;
        let spec = views::spec(&ws.jj, revset, view, &stack).await?;
        let diffs = spec.diff(&ws.jj).await?;
        let sides = spec.sides();
        let cfg = config.matching;
        let (files, dtos) = self.build(sides, diffs, config).await?;
        let (from, to) = spec.endpoints();
        let has_working_copy = ws.jj.includes_working_copy(revset).await?;
        Ok(Loaded {
            files,
            commits: stack.changes.iter().map(|c| c.commit_id.clone()).collect(),
            cfg,
            dto: ReviewDto {
                id: 0,
                revset: revset.to_string(),
                view: view.clone(),
                from,
                to,
                interdiff: matches!(spec, views::Spec::Interdiff { .. }),
                files: dtos,
                state: StateDto {
                    marks: vec![],
                    checks: vec![],
                    amber: vec![],
                    comments: vec![],
                },
                changes: stack.changes,
                removed_changes: stack.removed,
                previous_session: prev.map(|p| p.started_at as f64),
                stale: ws.stale.lock().unwrap().clone(),
                has_working_copy,
            },
        })
    }

    /// Builds each file, reusing the last load's build of files whose diff is unchanged.
    async fn build(
        &self,
        sides: Sides,
        diffs: Vec<FileDiff>,
        config: Config,
    ) -> anyhow::Result<(Vec<ReviewFile>, Vec<FileDto>)> {
        let keys: Vec<cache::Key> = diffs
            .iter()
            .map(|d| cache::key(&sides, d, &config))
            .collect();
        let mut built: Vec<Option<(ReviewFile, FileDto)>> = {
            let mut cache = self.cache.lock().unwrap();
            keys.iter().map(|k| cache.take(k, &sides.new)).collect()
        };
        let misses: Vec<(usize, FileDiff)> = diffs
            .into_iter()
            .enumerate()
            .filter(|(i, _)| built[*i].is_none())
            .collect();
        let miss_diffs: Vec<&FileDiff> = misses.iter().map(|(_, d)| d).collect();
        let contents = self.load_contents(&sides, &miss_diffs).await?;
        let fresh = tokio::task::spawn_blocking(move || {
            misses
                .into_iter()
                .zip(contents)
                .map(|((i, d), (old, new))| (i, build::build_file(&sides, d, old, new, &config)))
                .collect::<Vec<_>>()
        })
        .await?;
        for (i, b) in fresh {
            built[i] = Some(b);
        }
        let built: Vec<(ReviewFile, FileDto)> = built.into_iter().map(Option::unwrap).collect();
        self.cache
            .lock()
            .unwrap()
            .put(keys.into_iter().zip(built.iter().cloned()));
        Ok(built.into_iter().unzip())
    }

    /// Old and new contents per file, `None` where a side is absent, binary, or too large.
    async fn load_contents(
        &self,
        sides: &Sides,
        diffs: &[&FileDiff],
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
                let bytes = self.main.jj.file(commit.as_deref(), &path).await?;
                Ok((bytes.len() <= MAX_FILE_BYTES)
                    .then(|| String::from_utf8(bytes).ok())
                    .flatten())
            }
        };
        futures::future::try_join_all(diffs.iter().map(|d| {
            let old_path = d.old_path.as_deref().unwrap_or(&d.path);
            let old_present = !d.binary && d.status != FileStatus::Added;
            let new_present = !d.binary && d.status != FileStatus::Removed;
            let old = load(sides.old.as_deref(), old_path, old_present);
            let new = load(Some(&sides.new), &d.path, new_present);
            async move { tokio::try_join!(old, new) }
        }))
        .await
    }

    /// Marks changed rows, or clears their marks when `state` is `None`. `check` lists files
    /// to check in the same step, for a whole-file selection marked reviewed.
    pub fn mark(
        &self,
        id: u32,
        rows: &[RowRef],
        state: Option<MarkState>,
        check: &[u32],
    ) -> anyhow::Result<StateDto> {
        self.edit(id, |review, store| {
            let mut edit = review.mark_edit(store, rows, state)?;
            let checks = review.check_edit(check, true);
            edit.checks_removed = checks.checks_removed;
            edit.checks_added = checks.checks_added;
            Ok(edit)
        })
    }

    pub fn set_checked(&self, id: u32, files: &[u32], checked: bool) -> anyhow::Result<StateDto> {
        self.edit(id, |review, _| Ok(review.check_edit(files, checked)))
    }

    /// Removes the marks and checks this review's revset created.
    pub fn reset(&self, id: u32) -> anyhow::Result<StateDto> {
        self.edit(id, |review, store| {
            Ok(Edit {
                marks_removed: store.marks_for_review(&review.label)?,
                checks_removed: store.checks_for_review(&review.label)?,
                ..Edit::default()
            })
        })
    }

    fn edit(
        &self,
        id: u32,
        make: impl FnOnce(&Review, &Store) -> anyhow::Result<Edit>,
    ) -> anyhow::Result<StateDto> {
        let review = self.review(id)?;
        let mut review = review.lock().unwrap();
        let mut store = self.store.lock().unwrap();
        let edit = make(&review, &store)?;
        if !edit.is_empty() {
            let applied = store.apply(&edit)?;
            review.undo.push(applied);
            review.redo.clear();
            review.rematch(&store)?;
            review.save_session(&store)?;
        }
        Ok(review.state())
    }

    pub fn undo(&self, id: u32) -> anyhow::Result<StateDto> {
        self.step(id, true)
    }

    pub fn redo(&self, id: u32) -> anyhow::Result<StateDto> {
        self.step(id, false)
    }

    fn step(&self, id: u32, undo: bool) -> anyhow::Result<StateDto> {
        let review = self.review(id)?;
        let mut review = review.lock().unwrap();
        let mut store = self.store.lock().unwrap();
        let popped = if undo {
            review.undo.pop()
        } else {
            review.redo.pop()
        };
        if let Some(edit) = popped {
            if undo {
                store.apply(&edit.inverse())?;
                review.redo.push(edit);
            } else {
                store.apply(&edit)?;
                review.undo.push(edit);
            }
            review.rematch(&store)?;
            review.save_session(&store)?;
        }
        Ok(review.state())
    }
}

impl Review {
    fn new(ws: Arc<Workspace>, revset: &str, label: String, cfg: MatchConfig) -> Self {
        Self {
            ws,
            revset: revset.to_string(),
            label,
            view: View::Combined,
            cfg,
            files: vec![],
            matched: HashMap::new(),
            checks: vec![],
            amber: vec![],
            comments: vec![],
            baseline: HashSet::new(),
            session: 0,
            commits: vec![],
            prev: None,
            undo: vec![],
            redo: vec![],
        }
    }

    /// Replaces the review's files with `loaded`. Returns the DTO and where old rows went.
    fn swap_in(
        &mut self,
        store: &Store,
        loaded: Loaded,
        id: u32,
    ) -> anyhow::Result<(ReviewDto, Vec<RowMove>)> {
        let moves = rowmap::map_rows(&self.files, &loaded.files);
        self.files = loaded.files;
        self.cfg = loaded.cfg;
        self.commits = loaded.commits;
        self.rematch(store)?;
        self.save_session(store)?;
        let mut dto = loaded.dto;
        dto.id = id;
        dto.state = self.state();
        Ok((dto, moves))
    }

    fn save_session(&self, store: &Store) -> anyhow::Result<()> {
        let mut seen: Vec<i64> = self.baseline.iter().copied().collect();
        seen.sort();
        store.save_session(self.session, &self.commits, &seen)?;
        Ok(())
    }

    fn state(&self) -> StateDto {
        let mut marks: Vec<MarkDto> = self
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
        marks.sort_by_key(|m| (m.file, m.row));
        let checks = self
            .checks
            .iter()
            .map(|c| match c {
                None => CheckState::Unchecked,
                Some(c) if c.current => CheckState::Checked,
                Some(_) => CheckState::ChangedSinceReviewed,
            })
            .collect();
        StateDto {
            marks,
            checks,
            amber: self.amber.clone(),
            comments: self.comments.clone(),
        }
    }

    fn mark_edit(
        &self,
        store: &Store,
        rows: &[RowRef],
        state: Option<MarkState>,
    ) -> anyhow::Result<Edit> {
        let created_at = now_ms();
        let mut edit = Edit::default();
        let wanted: HashSet<RowRef> = rows.iter().copied().collect();
        for (file_idx, file) in self.files.iter().enumerate() {
            for k in &file.keyed {
                let rref = RowRef {
                    file: file_idx as u32,
                    row: k.row,
                };
                if !wanted.contains(&rref) {
                    continue;
                }
                let existing = self.matched.get(&rref);
                if existing.map(|m| m.state) == state {
                    continue;
                }
                let side = file.side(k.side);
                let Some(state) = state else {
                    edit.marks_removed.extend(existing.cloned());
                    continue;
                };
                // An interdiff's old side isn't a commit, so its rows can't be anchored.
                let Some(commit) = side.commit.clone() else {
                    continue;
                };
                edit.marks_removed.extend(existing.cloned());
                if let Some(index) = &side.index {
                    store.save_line_hashes(&commit, &side.path, &index.hashes)?;
                }
                edit.marks_added.push(Mark {
                    id: 0,
                    anchor: k.anchor.clone(),
                    commit,
                    line: k.line,
                    state,
                    review: self.label.clone(),
                    created_at,
                });
            }
        }
        Ok(edit)
    }

    /// Checking replaces any older check with one covering the file's changed rows now.
    fn check_edit(&self, files: &[u32], checked: bool) -> Edit {
        let mut edit = Edit::default();
        for &f in files {
            let (Some(file), Some(existing)) =
                (self.files.get(f as usize), self.checks.get(f as usize))
            else {
                continue;
            };
            if checked && existing.as_ref().is_some_and(|c| c.current) {
                continue;
            }
            edit.checks_removed
                .extend(existing.as_ref().map(|c| c.check.clone()));
            if checked {
                let hashes = |side| {
                    file.keyed
                        .iter()
                        .filter(|k| k.side == side)
                        .map(|k| k.anchor.line_hash)
                        .collect()
                };
                edit.checks_added.push(FileCheck {
                    id: 0,
                    path: file.path.clone(),
                    old_hashes: hashes(Side::Old),
                    new_hashes: hashes(Side::New),
                    review: self.label.clone(),
                    created_at: now_ms(),
                });
            }
        }
        edit
    }
}
