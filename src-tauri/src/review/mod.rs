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
mod fixes;
#[cfg(test)]
mod fixes_tests;
mod github;
#[cfg(test)]
mod github_tests;
mod graph;
pub(crate) mod guided;
mod link;
pub use link::LinkThread;
mod load;
mod matching;
mod open;
mod quotes;
mod replies;
mod rowmap;
#[cfg(test)]
pub(crate) mod tests;
mod threads;
mod untrusted;
mod views;
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
use build::{FileDto, ReviewFile};
use comments::CommentDto;
pub use context::ContextLine;
pub use github::PulledThread;
use load::{Listed, Loaded};
use matching::MatchedCheck;
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
    evologs: views::Evologs,
    configs: crate::config::ConfigTexts,
    history: arch::history::HistoryCache,
    /// Terminals linked to a PR review, by terminal panel id.
    links: Mutex<HashMap<String, link::Link>>,
    /// The brief each terminal's agent session last got, by terminal panel id.
    briefed: Mutex<HashMap<String, String>>,
}

impl Default for Reviews {
    fn default() -> Self {
        Self {
            open: Mutex::default(),
            current: Mutex::default(),
            next_id: AtomicU32::new(1),
            cache: Mutex::default(),
            evologs: Default::default(),
            configs: Default::default(),
            history: Default::default(),
            links: Mutex::default(),
            briefed: Mutex::default(),
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
    /// The whole review's head, whichever view is open.
    pub head: String,
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
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RefreshDto {
    Loaded {
        review: Box<ReviewDto>,
        /// Where rows of the review as it was went.
        moves: Vec<RowMove>,
    },
    /// The review still shows the same commits in the same view, so its files are as they
    /// were.
    Same {
        changes: Vec<ChangeDto>,
        state: StateDto,
        stale: Option<String>,
    },
}

#[cfg(test)]
impl RefreshDto {
    /// The reloaded review and where its rows went.
    pub fn loaded(self) -> (ReviewDto, Vec<RowMove>) {
        match self {
            Self::Loaded { review, moves } => (*review, moves),
            Self::Same { .. } => panic!("the refresh kept the review as it was"),
        }
    }
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
    changes: Vec<ChangeDto>,
    /// The view the files were loaded in. `None` until the first load, and after a session
    /// change, so the next refresh loads them again.
    shown: Option<View>,
    prev: Option<Session>,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    /// Counts refreshes, so one that a later refresh overtook doesn't swap in.
    load_gen: u64,
}

/// A refresh's error when a later one overtook it. The UI drops it.
pub const SUPERSEDED: &str = "superseded by a later refresh";

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
            changes: vec![],
            shown: None,
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
        self.changes = loaded.dto.changes.clone();
        self.shown = Some(loaded.dto.view.clone());
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
            threads: self.threads.clone(),
        }
    }
}
