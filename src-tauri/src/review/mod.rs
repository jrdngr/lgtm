//! A loaded review: its diff, file sides, anchors, and the marks and checks matched onto it.

mod agent;
#[cfg(test)]
mod agent_tests;
pub(crate) mod arch;
mod at;
mod build;
mod cache;
mod chat;
#[cfg(test)]
mod comment_tests;
mod comments;
mod context;
mod edits;
mod github;
#[cfg(test)]
mod github_tests;
mod graph;
mod link;
pub use link::LinkThread;
mod load;
mod matching;
#[cfg(test)]
mod note_tests;
mod notes;
mod quotes;
mod replies;
mod rowmap;
#[cfg(test)]
pub(crate) mod tests;
mod threads;
mod untrusted;
mod views;
pub(crate) mod walkthrough;
#[cfg(test)]
mod workflow_tests;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::anchor::MatchConfig;
use crate::ctx::{RepoCtx, now_ms};
use crate::jj::Range;
use crate::store::{Edit, Mark, MarkColor, Session, Store};
use crate::workspace::Workspace;
pub use at::At;
use build::{FileDto, ReviewFile};
use comments::CommentDto;
pub use context::ContextLine;
pub use github::PulledThread;
pub use graph::GraphDto;
use load::Loaded;
use matching::MatchedCheck;
pub use notes::{NoteDto, NoteInput};
pub use quotes::quoted_reply;
pub use rowmap::RowMove;
pub use threads::{ThreadDto, pulled_stage};
pub use views::{ChangeDto, View};

/// The open reviews and what they share, kept on the [`RepoCtx`].
pub struct Reviews {
    open: Mutex<HashMap<u32, Arc<Mutex<Review>>>>,
    /// The review each workspace's last active review pane shows, by workspace key.
    current: Mutex<HashMap<String, u32>>,
    next_id: AtomicU32,
    cache: Mutex<cache::FileCache>,
    /// Terminals linked to a PR review, by terminal panel id.
    links: Mutex<HashMap<String, link::Link>>,
    /// The commits the last Assess prompt named, by workspace key and revset.
    assess_asked: Mutex<HashMap<(String, String), Range>>,
}

impl Default for Reviews {
    fn default() -> Self {
        Self {
            open: Mutex::default(),
            current: Mutex::default(),
            next_id: AtomicU32::new(1),
            cache: Mutex::default(),
            links: Mutex::default(),
            assess_asked: Mutex::default(),
        }
    }
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

impl RowRef {
    pub(super) fn new(file: usize, row: u32) -> Self {
        Self {
            file: file as u32,
            row,
        }
    }
}

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MarkDto {
    pub file: u32,
    pub row: u32,
    pub color: MarkColor,
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
#[derive(Clone, Default, Serialize, Type)]
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
    /// The review's walkthrough notes, in reading order. Ones whose code is gone have no rows.
    pub notes: Vec<NoteDto>,
    /// In a PR workspace, every thread and draft on the PR, in file order, then comments on
    /// the PR by time.
    pub threads: Vec<ThreadDto>,
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
    notes: Vec<NoteDto>,
    threads: Vec<ThreadDto>,
    /// Marks this review has matched, in this session or the previous one. One that stops
    /// matching was lost, and the rows that replaced its line turn amber.
    baseline: HashSet<i64>,
    session: i64,
    /// The diff's endpoints as last loaded.
    from: Option<String>,
    to: String,
    /// The revset's base and head as last loaded, whichever view is open.
    whole: Range,
    /// The commits of the review's changes as last loaded, oldest first.
    commits: Vec<String>,
    prev: Option<Session>,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    /// Counts refreshes, so one that a later refresh overtook doesn't swap in.
    load_gen: u64,
}

/// A refresh's error when a later one overtook it. The UI drops it.
pub const SUPERSEDED: &str = "superseded by a later refresh";

impl RepoCtx {
    fn review(&self, id: u32) -> anyhow::Result<Arc<Mutex<Review>>> {
        self.reviews
            .open
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .context("review is closed")
    }

    pub fn close(&self, id: u32) {
        self.reviews.open.lock().unwrap().remove(&id);
        self.reviews.current.lock().unwrap().retain(|_, r| *r != id);
    }

    /// Records the review a workspace's active review pane shows, for agents to act on.
    pub fn focus(&self, workspace: Option<&str>, id: u32) {
        let key = workspace.unwrap_or("").to_string();
        self.reviews.current.lock().unwrap().insert(key, id);
    }

    pub fn current_review(&self, workspace: Option<&str>) -> anyhow::Result<u32> {
        self.reviews
            .current
            .lock()
            .unwrap()
            .get(workspace.unwrap_or(""))
            .copied()
            .context("no review is open in lgtm for this workspace")
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
        let (prev, resumed, seen) = {
            let store = self.store.lock().unwrap();
            match store.last_session(&label)? {
                Some(last) if resume => (
                    store.session_before(&label, last.id)?,
                    Some(last.id),
                    last.seen,
                ),
                prev => (prev, None, vec![]),
            }
        };
        let loaded = self
            .load(&ws, revset, &View::Combined, prev.as_ref())
            .await?;
        // Started only once the load succeeds, so a revset that fails leaves no session.
        let session = match resumed {
            Some(id) => id,
            None => self
                .store
                .lock()
                .unwrap()
                .start_session(&label, ws.key(), revset, now_ms())?,
        };
        let id = self.reviews.next_id.fetch_add(1, Ordering::Relaxed);
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
            .open
            .lock()
            .unwrap()
            .insert(id, Arc::new(Mutex::new(review)));
        Ok(dto)
    }

    /// Ends the review's session and starts another, so the next load compares against what
    /// it showed until now. For when commits are about to be replaced, as a PR refetch does.
    pub fn seal_session(&self, id: u32) -> anyhow::Result<()> {
        let review = self.review(id)?;
        let mut review = review.lock().unwrap();
        let store = self.store.lock().unwrap();
        review.save_session(&store)?;
        review.prev = store.last_session(&review.label)?;
        review.session =
            store.start_session(&review.label, review.ws.key(), &review.revset, now_ms())?;
        Ok(())
    }

    /// Whether the review shows `revset` whole, in the Combined view.
    pub fn shows_whole(&self, id: u32, revset: &str) -> anyhow::Result<bool> {
        let review = self.review(id)?;
        let review = review.lock().unwrap();
        Ok(review.revset == revset && review.view == View::Combined)
    }

    /// The workspace a review is in.
    pub fn review_workspace(&self, id: u32) -> anyhow::Result<Arc<Workspace>> {
        Ok(self.review(id)?.lock().unwrap().ws.clone())
    }

    /// Reloads a review after the repo changed, or in another view, keeping its marks' undo
    /// history. Fails with [`SUPERSEDED`] when a later refresh started before it finished.
    pub async fn refresh(&self, id: u32, view: Option<View>) -> anyhow::Result<RefreshDto> {
        let (ws, revset, view, prev, load_gen) = {
            let review = self.review(id)?;
            let mut review = review.lock().unwrap();
            // Set now, so a refresh that starts before this one ends loads this view too.
            if let Some(view) = view {
                review.view = view;
            }
            review.load_gen += 1;
            (
                review.ws.clone(),
                review.revset.clone(),
                review.view.clone(),
                review.prev.clone(),
                review.load_gen,
            )
        };
        let loaded = self.load(&ws, &revset, &view, prev.as_ref()).await?;
        let review = self.review(id)?;
        let mut review = review.lock().unwrap();
        anyhow::ensure!(review.load_gen == load_gen, SUPERSEDED);
        review.view = loaded.dto.view.clone();
        let store = self.store.lock().unwrap();
        let (review, moves) = review.swap_in(&store, loaded, id)?;
        Ok(RefreshDto { review, moves })
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
            notes: vec![],
            threads: vec![],
            baseline: HashSet::new(),
            session: 0,
            from: None,
            to: String::new(),
            whole: Range {
                from: None,
                to: String::new(),
            },
            commits: vec![],
            prev: None,
            undo: vec![],
            redo: vec![],
            load_gen: 0,
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
        self.from = loaded.dto.from.clone();
        self.to = loaded.dto.to.clone();
        self.whole = loaded.whole;
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
                color: m.color,
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
            notes: self.notes.clone(),
            threads: self.threads.clone(),
        }
    }
}
