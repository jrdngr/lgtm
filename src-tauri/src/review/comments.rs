//! Comments on a review: placing them on rows, and adding, editing, and deleting them.

use std::collections::HashSet;
use std::sync::Mutex;

use anyhow::Context;
use serde::Serialize;
use specta::Type;

use super::matching::{Remaps, SideRows, pools};
use super::threads::after_edit;
use super::{RepoCtx, Review, RowRef, StateDto, now_ms};
use crate::anchor::{self, MarkRef};
use crate::store::{Comment, Conversation, GhLink, Message, Pin, Store, ThreadState};

/// The first and last rows a comment covers, by index among its file's rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
pub struct RowSpan {
    pub start: u32,
    pub end: u32,
}

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
    /// The GitHub thread it's on, whose replies show above it.
    pub github: Option<GhLink>,
    /// Set on GitHub threads and drafts of them; notes have none.
    pub thread: Option<ThreadState>,
    /// A note the user is done with.
    pub done: bool,
}

impl Review {
    pub(super) fn rematch_comments(&mut self, store: &Store) -> anyhow::Result<()> {
        let comments = store.comments_for_paths(&self.paths())?;
        let pr = self.pr(store)?;
        // GitHub threads and drafts show only in their own PR's workspace.
        let ours = |c: &Comment| {
            c.thread
                .as_ref()
                .is_none_or(|t| pr.as_ref().is_some_and(|(r, n)| t.repo == *r && t.pr == *n))
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
        if rows.is_none() && c.github.as_ref().is_some_and(|g| !g.outdated) {
            if let Some(r) = self.near_last_pin(store, remaps, &c.path, &c.pins) {
                file = r.file;
                rows = Some(RowSpan {
                    start: r.row,
                    end: r.row,
                });
            }
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
            github: c.github,
            thread: c.thread,
            done: c.done,
        })
    }

    /// The file and rows that pins on `path` match, or `None` when none of the review's files
    /// is that file. The rows are absent when the code can't be found any more.
    pub(super) fn placement(
        &self,
        store: &Store,
        remaps: &mut Remaps,
        path: &str,
        pins: &[Pin],
    ) -> Option<(u32, Option<RowSpan>)> {
        let found: Vec<RowRef> = pins
            .iter()
            .filter_map(|p| self.locate(store, remaps, path, p))
            .collect();
        let end = found.last().copied();
        let rows = end.map(|end| {
            let same: Vec<u32> = found
                .iter()
                .filter(|r| r.file == end.file)
                .map(|r| r.row)
                .collect();
            let (lo, hi) = (*same.iter().min().unwrap(), *same.iter().max().unwrap());
            (end.file, RowSpan { start: lo, end: hi })
        });
        let file = match rows {
            Some((f, _)) => f,
            None => self
                .files
                .iter()
                .position(|f| f.path == path || f.old_path.as_deref() == Some(path))?
                as u32,
        };
        Some((file, rows.map(|(_, span)| span)))
    }

    /// The changed row a pin matches best, trying the files whose path pools hold its path:
    /// its own, or else `path`, its comment's. A pin whose line was edited lands on a line that
    /// replaced it and shares some of its context.
    fn locate(&self, store: &Store, remaps: &mut Remaps, path: &str, pin: &Pin) -> Option<RowRef> {
        let path = pin.path.as_deref().unwrap_or(path);
        let anchor = pin.anchor(path);
        let sides = self.pin_sides(path, pin);
        for rows in &sides {
            let mark = MarkRef {
                id: 0,
                anchor: &anchor,
                created_at: 0,
                remap: remaps.row(store, &pin.commit, path, pin.line, rows),
            };
            if let Some((&row, _)) = anchor::match_marks(&[mark], &rows.candidates, self.cfg)
                .iter()
                .next()
            {
                return Some(RowRef::new(rows.file_idx, row as u32));
            }
        }
        // The line was edited: take the line that replaced it whose context fits best, if any
        // of its context is still there.
        sides.iter().find_map(|rows| {
            let edited = remaps.edited(store, &pin.commit, path, pin.line, rows);
            let (score, best) = edited
                .iter()
                .rev()
                .map(|c| (anchor::score(&anchor, c), c))
                .max_by_key(|(s, _)| *s)?;
            (score > 0).then(|| RowRef::new(rows.file_idx, best.id as u32))
        })
    }

    /// The changed rows of each file side a pin on `path` may be on.
    fn pin_sides(&self, path: &str, pin: &Pin) -> Vec<SideRows<'_>> {
        let removed = self.removed_paths();
        self.files
            .iter()
            .enumerate()
            .filter(|(_, f)| {
                pools(f, pin.side, &removed)
                    .iter()
                    .any(|p| p.contains(&path))
            })
            .filter_map(|(i, f)| SideRows::new(i, f, pin.side))
            .collect()
    }

    /// The changed row nearest where the last pin's line is now, for a GitHub thread whose code
    /// can't be found: it stays on a line near the one GitHub shows it on.
    fn near_last_pin(
        &self,
        store: &Store,
        remaps: &mut Remaps,
        path: &str,
        pins: &[Pin],
    ) -> Option<RowRef> {
        let pin = pins.last()?;
        let path = pin.path.as_deref().unwrap_or(path);
        self.pin_sides(path, pin).iter().find_map(|rows| {
            let row = remaps.nearest(store, &pin.commit, path, pin.line, rows)?;
            Some(RowRef::new(rows.file_idx, row as u32))
        })
    }

    /// Pins for the first and last of `rows` that can be anchored, all in the last one's file.
    pub(super) fn pins(
        &self,
        store: &Store,
        rows: &[RowRef],
    ) -> anyhow::Result<(String, Vec<Pin>)> {
        self.try_pins(store, rows)?
            .context("these rows can't hold a comment")
    }

    /// Like [`Self::pins`], but `None` when none of `rows` can be anchored, as removed rows
    /// of an interdiff can't.
    pub(super) fn try_pins(
        &self,
        store: &Store,
        rows: &[RowRef],
    ) -> anyhow::Result<Option<(String, Vec<Pin>)>> {
        let wanted: HashSet<RowRef> = rows.iter().copied().collect();
        let file_idx = rows.last().context("nothing to comment on")?.file;
        let file = self.files.get(file_idx as usize).context("no such file")?;
        let mut pins = Vec::new();
        for k in &file.keyed {
            if !wanted.contains(&RowRef {
                file: file_idx,
                row: k.row,
            }) {
                continue;
            }
            let Some(commit) = file.side(k.side).anchor_commit(store)? else {
                continue;
            };
            let path = file.side(k.side).path.clone();
            pins.push(Pin::new(&k.anchor, commit, k.line, Some(path)));
        }
        if pins.len() > 2 {
            pins.drain(1..pins.len() - 1);
        }
        let Some(last) = pins.last() else {
            return Ok(None);
        };
        let path = last.path.clone().unwrap_or_default();
        Ok(Some((path, pins)))
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

    /// Comments on `rows`, the changed rows of a selection. `snippet` is the code as shown.
    /// `draft` makes it a draft of a GitHub thread on the workspace's PR instead of a note.
    pub fn add_comment(
        &self,
        id: u32,
        rows: &[RowRef],
        body: &str,
        snippet: &str,
        draft: bool,
    ) -> anyhow::Result<StateDto> {
        self.comment_edit(id, |review, store| {
            let (path, pins) = review.pins(store, rows)?;
            let thread = match draft {
                true => Some(review.draft_state(store, body)?),
                false => None,
            };
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
                done: false,
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
