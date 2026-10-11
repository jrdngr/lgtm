//! Comments on a review: placing them on rows, and adding, editing, and deleting them.

use std::collections::HashSet;

use anyhow::Context;
use serde::Serialize;
use specta::Type;

use super::matching::{Remaps, SideRows, pools};
use super::{RepoCtx, Review, RowRef, StateDto, now_ms};
use crate::anchor::{self, MarkRef};
use crate::store::{Comment, Message, Pin, Store, Workshop};

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
}

impl Review {
    pub(super) fn rematch_comments(&mut self, store: &Store) -> anyhow::Result<()> {
        let comments = store.comments_for_paths(&self.paths())?;
        let mut remaps = Remaps::default();
        self.comments = comments
            .into_iter()
            .filter_map(|c| self.place(store, &mut remaps, c))
            .collect();
        self.comments.sort_by_key(|c| {
            let at = c.rows.map_or(u32::MAX, |r| r.end);
            (c.file, at, c.created_at as i64)
        });
        Ok(())
    }

    /// Where a comment goes, or `None` when none of the review's files is its file.
    fn place(&self, store: &Store, remaps: &mut Remaps, c: Comment) -> Option<CommentDto> {
        let (file, rows) = self.placement(store, remaps, &c.path, &c.pins)?;
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
            messages: c.workshop.messages,
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

    /// The changed row a pin matches best, trying the files whose path pools hold `path`.
    fn locate(&self, store: &Store, remaps: &mut Remaps, path: &str, pin: &Pin) -> Option<RowRef> {
        let removed = self.removed_paths();
        let anchor = pin.anchor(path);
        for (file_idx, file) in self.files.iter().enumerate() {
            let Some(rows) = SideRows::new(file_idx, file, pin.side) else {
                continue;
            };
            if !pools(file, pin.side, &removed)
                .iter()
                .any(|p| p.contains(&path))
            {
                continue;
            }
            let mark = MarkRef {
                id: 0,
                anchor: &anchor,
                created_at: 0,
                remap: remaps.row(store, &pin.commit, path, pin.line, &rows),
            };
            if let Some((&row, _)) = anchor::match_marks(&[mark], &rows.candidates, self.cfg)
                .iter()
                .next()
            {
                return Some(RowRef::new(file_idx, row as u32));
            }
        }
        None
    }

    /// Pins for the first and last of `rows` that can be anchored, all in the last one's file.
    pub(super) fn pins(
        &self,
        store: &Store,
        rows: &[RowRef],
    ) -> anyhow::Result<(String, Vec<Pin>)> {
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
            pins.push(Pin::new(&k.anchor, commit, k.line));
        }
        if pins.len() > 2 {
            pins.drain(1..pins.len() - 1);
        }
        anyhow::ensure!(!pins.is_empty(), "these rows can't hold a comment");
        let path = file.side(pins[pins.len() - 1].side).path.clone();
        Ok((path, pins))
    }
}

impl RepoCtx {
    pub(super) fn comment_edit(
        &self,
        id: u32,
        f: impl FnOnce(&Review, &Store) -> anyhow::Result<()>,
    ) -> anyhow::Result<StateDto> {
        let review = self.review(id)?;
        let mut review = review.lock().unwrap();
        let store = self.store.lock().unwrap();
        f(&review, &store)?;
        review.rematch_comments(&store)?;
        Ok(review.state())
    }

    /// Comments on `rows`, the changed rows of a selection. `snippet` is the code as shown.
    pub fn add_comment(
        &self,
        id: u32,
        rows: &[RowRef],
        body: &str,
        snippet: &str,
    ) -> anyhow::Result<StateDto> {
        self.comment_edit(id, |review, store| {
            let (path, pins) = review.pins(store, rows)?;
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
                workshop: Workshop::default(),
            })?;
            Ok(())
        })
    }

    pub fn edit_comment(&self, id: u32, comment: u32, body: &str) -> anyhow::Result<StateDto> {
        self.comment_edit(id, |_, store| {
            Ok(store.set_comment_body(comment as i64, body, now_ms())?)
        })
    }

    pub fn delete_comment(&self, id: u32, comment: u32) -> anyhow::Result<StateDto> {
        self.comment_edit(id, |_, store| Ok(store.delete_comment(comment as i64)?))
    }
}
