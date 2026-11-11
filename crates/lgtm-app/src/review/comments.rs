//! Comments on a review: placing them on rows, and adding, editing, and deleting them.

use std::sync::Mutex;

use anyhow::Context;
use serde::Serialize;
use specta::Type;

use lgtm_core::review::place::Placer;

use super::matching::Remaps;
use super::threads::after_edit;
use super::{RepoCtx, Review, RowRef, StateDto, now_ms};
use crate::store::{Comment, Conversation, GhLink, Message, Pin, Store, ThreadState};

pub use lgtm_core::review::place::RowSpan;

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CommentDto {
    pub id: u32,
    pub path: String,
    /// The review file it belongs to.
    pub file: u32,
    /// Absent when its code can't be found any more.
    pub rows: Option<RowSpan>,
    pub body: String,
    /// The code as it was when the comment was made.
    pub snippet: String,
    pub review: String,
    pub created_at: f64,
    pub updated_at: f64,
    pub messages: Vec<Message>,
    /// A reply a synced terminal agent suggested.
    pub suggestion: Option<String>,
    /// The GitHub thread it's on, whose replies show above it.
    pub github: Option<GhLink>,
    /// Set on GitHub threads and drafts of them.
    pub thread: Option<ThreadState>,
}

impl Review {
    pub(super) fn rematch_comments(&mut self, store: &Store) -> anyhow::Result<()> {
        let comments = store.comments_for_paths(&self.paths())?;
        let pr = self.pr(store)?;
        // GitHub threads and drafts show only in their own PR's workspace. Stores from before
        // threads were the only comments may still hold notes, which never show.
        let ours = |c: &Comment| {
            c.thread
                .as_ref()
                .is_some_and(|t| pr.as_ref().is_some_and(|(r, n)| t.repo == *r && t.pr == *n))
        };
        let mut remaps = Remaps::default();
        self.comments = comments
            .into_iter()
            .filter(ours)
            .filter_map(|c| self.place(store, &mut remaps, c))
            .collect();
        self.comments.sort_by_key(|c| {
            let at = c.rows.map_or(u32::MAX, |r| r.end);
            (c.file, at, c.created_at as i64)
        });
        self.rematch_threads(store)
    }

    /// Where a comment goes, or `None` when none of the review's files is its file.
    fn place(&self, store: &Store, remaps: &mut Remaps, c: Comment) -> Option<CommentDto> {
        let (mut file, mut rows) = self.placement(store, remaps, &c.path, &c.pins)?;
        if rows.is_none()
            && c.github.as_ref().is_some_and(|g| !g.outdated)
            && let Some(r) = self.near_last_pin(store, remaps, &c.path, &c.pins)
        {
            file = r.file;
            rows = Some(RowSpan {
                start: r.row,
                end: r.row,
            });
        }
        Some(CommentDto {
            id: c.id as u32,
            path: c.path,
            file,
            rows,
            body: c.body,
            snippet: c.snippet,
            review: c.review,
            created_at: c.created_at as f64,
            updated_at: c.updated_at as f64,
            messages: c.conversation.messages,
            suggestion: c.conversation.suggestion,
            github: c.github,
            thread: c.thread,
        })
    }

    /// The file and rows that pins on `path` match. See [`Placer::placement`].
    pub(super) fn placement(
        &self,
        store: &Store,
        remaps: &mut Remaps,
        path: &str,
        pins: &[Pin],
    ) -> Option<(u32, Option<RowSpan>)> {
        self.placer().placement(store, remaps, path, pins)
    }

    fn near_last_pin(
        &self,
        store: &Store,
        remaps: &mut Remaps,
        path: &str,
        pins: &[Pin],
    ) -> Option<RowRef> {
        self.placer().near_last_pin(store, remaps, path, pins)
    }

    pub(super) fn pins(
        &self,
        store: &Store,
        rows: &[RowRef],
    ) -> anyhow::Result<(String, Vec<Pin>)> {
        self.placer().pins(store, rows)
    }

    pub(super) fn placer(&self) -> Placer<'_> {
        Placer {
            files: &self.files,
            cfg: self.cfg,
        }
    }
}

impl RepoCtx {
    pub(crate) fn comment_edit(
        &self,
        id: u32,
        f: impl FnOnce(&Review, &Store) -> anyhow::Result<()>,
    ) -> anyhow::Result<StateDto> {
        self.comment_edit_on(&*self.review(id)?, f)
    }

    /// [`Self::comment_edit`] on a review held since before a long run, which makes the edit
    /// even when the review was closed in the meantime.
    pub(super) fn comment_edit_on(
        &self,
        review: &Mutex<Review>,
        f: impl FnOnce(&Review, &Store) -> anyhow::Result<()>,
    ) -> anyhow::Result<StateDto> {
        let mut review = review.lock().unwrap();
        let store = self.store.lock().unwrap();
        f(&review, &store)?;
        review.rematch_comments(&store)?;
        Ok(review.state())
    }

    /// Starts a draft of a GitHub thread on the workspace's PR on `rows`, the changed rows of a
    /// selection. `snippet` is the code as shown.
    pub fn add_comment(
        &self,
        id: u32,
        rows: &[RowRef],
        body: &str,
        snippet: &str,
    ) -> anyhow::Result<StateDto> {
        self.comment_edit(id, |review, store| {
            let (path, pins) = review.pins(store, rows)?;
            let thread = Some(review.draft_state(store, body)?);
            let now = now_ms();
            store.add_comment(&Comment {
                id: 0,
                path,
                pins,
                body: body.to_string(),
                snippet: snippet.to_string(),
                review: review.label.clone(),
                created_at: now,
                updated_at: now,
                conversation: Conversation::default(),
                github: None,
                thread,
            })?;
            Ok(())
        })
    }

    /// Saves a comment's text. A thread's reply moves its stage along.
    pub fn edit_comment(&self, id: u32, comment: u32, body: &str) -> anyhow::Result<StateDto> {
        self.comment_edit(id, |_, store| {
            let cid = comment as i64;
            let c = store.comment(cid)?.context("the comment is gone")?;
            let draft = c.is_draft();
            if let Some(mut t) = c.thread
                && c.body != body
            {
                t.stage = after_edit(t.stage, draft, body);
                store.set_thread(cid, &Some(t))?;
            }
            Ok(store.set_comment_body(cid, body, now_ms())?)
        })
    }

    pub fn delete_comment(&self, id: u32, comment: u32) -> anyhow::Result<StateDto> {
        self.comment_edit(id, |_, store| Ok(store.delete_comment(comment as i64)?))
    }
}
