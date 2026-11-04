//! Comments for the agent (C1, C2, …) on a review, the same ones the terminal app shows:
//! placing them on rows, and writing, editing, resolving and deleting them as undo steps.

use lgtm_core::review::comments::{self as core, RowText, Step};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::json;
use specta::Type;

use super::build::ReviewFile;
use super::{RepoCtx, Review, StateDto};
use crate::store::{AgentComment, Store};

pub use lgtm_core::review::place::RowSpan;

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentCommentDto {
    /// Shown as `C<number>`.
    pub number: u32,
    pub path: String,
    /// The review file it's on.
    pub file: u32,
    /// The rows it covers. Absent when its code is gone, and then it shows atop its file.
    pub rows: Option<RowSpan>,
    pub body: String,
    pub done: bool,
    pub created_at: f64,
}

/// A file's rows by index, as placing and listing comments see them.
fn rows(file: &ReviewFile) -> impl Iterator<Item = (u32, RowText<'_>)> + Clone {
    file.rows
        .iter()
        .zip(&file.texts)
        .enumerate()
        .map(|(i, (r, text))| {
            let row = RowText {
                kind: r.kind,
                old_line: r.old_line,
                new_line: r.new_line,
                text,
            };
            (i as u32, row)
        })
}

fn row(file: &ReviewFile, i: u32) -> RowText<'_> {
    let r = &file.rows[i as usize];
    RowText {
        kind: r.kind,
        old_line: r.old_line,
        new_line: r.new_line,
        text: &file.texts[i as usize],
    }
}

impl Review {
    fn file_of(&self, c: &AgentComment) -> Option<usize> {
        self.files
            .iter()
            .position(|f| f.path == c.path || f.old_path.as_deref() == Some(&c.path))
    }

    /// The rows `c` covers in file `f`: its last row, and its first when it's on a range
    /// that's still there.
    fn span(&self, f: usize, c: &AgentComment) -> Option<RowSpan> {
        let file = &self.files[f];
        let end = core::place(rows(file), c)?;
        let start = core::start_of(c)
            .and_then(|s| core::place(rows(file), &s))
            .filter(|&s| s <= end)
            .unwrap_or(end);
        Some(RowSpan { start, end })
    }

    /// Reads the comments on the review's files from the store.
    pub(super) fn rematch_agent_comments(&mut self, store: &Store) -> anyhow::Result<()> {
        let found = store.agent_comments_for_paths(&self.paths())?;
        self.agent_comments = found
            .iter()
            .filter_map(|c| {
                let f = self.file_of(c)?;
                let rows = self.span(f, c);
                let kind = rows.map(|r| self.files[f].rows[r.end as usize].kind);
                core::shows_in(c, &self.revset, kind).then(|| AgentCommentDto {
                    number: c.number as u32,
                    path: c.path.clone(),
                    file: f as u32,
                    rows,
                    body: c.body.clone(),
                    done: c.done,
                    created_at: c.created_at as f64,
                })
            })
            .collect();
        self.agent_comments
            .sort_by_key(|n| (n.file, n.rows.map_or(0, |r| r.end), n.number));
        self.next_agent_comment = store.next_agent_comment_number()? as u32;
        Ok(())
    }

    /// The comments as `get_comments` lists them for an agent: open ones first.
    pub(super) fn agent_comments_listing(
        &self,
        store: &Store,
    ) -> anyhow::Result<serde_json::Value> {
        let found = store.agent_comments_for_paths(&self.paths())?;
        let mut shown: Vec<(&AgentComment, &AgentCommentDto)> = self
            .agent_comments
            .iter()
            .filter_map(|n| Some((found.iter().find(|c| c.number == n.number as i64)?, n)))
            .collect();
        shown.sort_by_key(|(c, _)| (c.done, c.number));
        let listed: Vec<_> = shown
            .into_iter()
            .map(|(c, n)| {
                let file = &self.files[n.file as usize];
                let line = n.rows.and_then(|r| {
                    let row = &file.rows[r.end as usize];
                    match c.side {
                        crate::anchor::Side::New => row.new_line,
                        crate::anchor::Side::Old => row.old_line,
                    }
                });
                let range = n.rows.filter(|_| c.start.is_some()).and_then(|r| {
                    let span: Vec<_> = (r.start..=r.end).map(|i| row(file, i)).collect();
                    core::range_code(&span)
                });
                core::listing(c, line, range)
            })
            .collect();
        Ok(json!(listed))
    }
}

impl RepoCtx {
    /// Writes comment C<next> saying `body` on rows `start..=end` of review file `file`.
    pub fn add_agent_comment(
        &self,
        id: u32,
        file: u32,
        start: u32,
        end: u32,
        body: &str,
    ) -> anyhow::Result<StateDto> {
        self.agent_comment_step(id, |review, store| {
            let f = review
                .files
                .get(file as usize)
                .ok_or_else(|| anyhow::anyhow!("no file {file} in the review"))?;
            if end as usize >= f.rows.len() || start > end {
                anyhow::bail!("no rows {start}-{end} in {}", f.path);
            }
            let first = (start < end).then(|| row(f, start));
            let c = core::new_comment(
                &f.path,
                f.old_path.as_deref(),
                first,
                row(f, end),
                body,
                &review.revset,
            );
            Ok(Step::Added(store.add_agent_comment(&c)?))
        })
    }

    pub fn edit_agent_comment(&self, id: u32, number: u32, body: &str) -> anyhow::Result<StateDto> {
        self.agent_comment_step(id, |_, store| {
            let c = find(store, number)?;
            store.set_agent_comment_body(c.number, body)?;
            Ok(Step::Edited {
                number: c.number,
                before: c.body,
                after: body.to_string(),
            })
        })
    }

    pub fn set_agent_comment_done(
        &self,
        id: u32,
        number: u32,
        done: bool,
    ) -> anyhow::Result<StateDto> {
        self.agent_comment_step(id, |_, store| {
            let c = find(store, number)?;
            store.set_agent_comment_done(c.number, done)?;
            Ok(Step::Resolved {
                number: c.number,
                done,
            })
        })
    }

    pub fn delete_agent_comment(&self, id: u32, number: u32) -> anyhow::Result<StateDto> {
        self.agent_comment_step(id, |_, store| {
            let c = find(store, number)?;
            store.delete_agent_comment(c.number)?;
            Ok(Step::Deleted(c))
        })
    }

    /// Makes a change to the comments as one undo step, and rereads them.
    fn agent_comment_step(
        &self,
        id: u32,
        make: impl FnOnce(&Review, &Store) -> anyhow::Result<Step>,
    ) -> anyhow::Result<StateDto> {
        let review = self.review(id)?;
        let mut review = review.lock().unwrap();
        let store = self.store.lock().unwrap();
        let step = make(&review, &store)?;
        review.undo.push(step);
        review.redo.clear();
        review.rematch_agent_comments(&store)?;
        Ok(review.state())
    }

    /// Matches every open review again after another program, like the terminal app, wrote
    /// to the store, and shows the new states.
    pub fn reread_store(&self) {
        let open: Vec<(u32, Arc<Mutex<Review>>)> = self
            .reviews()
            .open
            .lock()
            .unwrap()
            .iter()
            .map(|(id, r)| (*id, r.clone()))
            .collect();
        for (id, review) in open {
            let state = {
                let mut review = review.lock().unwrap();
                let store = self.store.lock().unwrap();
                if let Err(e) = review.rematch(&store) {
                    tracing::warn!("reread review {id}: {e:#}");
                    continue;
                }
                review.state()
            };
            self.state_changed(id, &state);
        }
    }

    /// The current review's comments as an agent sees them.
    pub fn agent_comments_listing(
        &self,
        workspace: Option<&str>,
    ) -> anyhow::Result<serde_json::Value> {
        let store = self.store.lock().unwrap();
        self.with_current(workspace, |r| r.agent_comments_listing(&store))?
            .1
    }

    /// Resolves or reopens comment `number` for an agent in `workspace`'s terminal, as an undo
    /// step on the current review.
    pub fn agent_set_comment_done(
        &self,
        workspace: Option<&str>,
        number: u32,
        done: bool,
    ) -> anyhow::Result<()> {
        let id = self.current_review(workspace)?;
        let state = self.set_agent_comment_done(id, number, done)?;
        self.state_changed(id, &state);
        Ok(())
    }
}

fn find(store: &Store, number: u32) -> anyhow::Result<AgentComment> {
    store
        .agent_comment(number as i64)?
        .ok_or_else(|| anyhow::anyhow!("there's no comment C{number}"))
}
