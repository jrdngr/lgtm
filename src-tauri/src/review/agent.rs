//! What an agent in a terminal sees of a review and can do to it, through the MCP server.
//! Agents can read everything and add comments and walkthrough notes, but never mark rows.

use anyhow::Context;
use serde::Serialize;

use super::comments::RowSpan;
use super::{RepoCtx, Review, RowRef, StateDto, now_ms};
use crate::diff::{FileStatus, RowKind};
use crate::store::{Author, Comment, MarkState, Message, Workshop};

/// What comments and notes from a terminal agent record as their source review.
const BY_AGENT: &str = "terminal agent";

#[derive(Clone, Copy, Default, Serialize)]
pub struct Counts {
    pub reviewed: u32,
    pub skipped: u32,
    pub flagged: u32,
    pub unreviewed: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSummary {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    pub status: FileStatus,
    /// The reviewer ticked the whole file off.
    pub checked: bool,
    pub rows: Counts,
    /// Line ranges of changed code the reviewer hasn't reviewed, skipped or flagged.
    pub unreviewed: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub revset: String,
    /// The diff's old and new commits.
    pub from: Option<String>,
    pub to: String,
    /// Changed rows by state, across the review.
    pub rows: Counts,
    pub files: Vec<FileSummary>,
}

#[derive(Serialize)]
pub struct AgentMessage {
    pub author: Author,
    pub text: String,
}

#[derive(Serialize)]
pub struct AgentComment {
    pub id: u32,
    pub path: String,
    /// Absent when its code is gone.
    pub lines: Option<String>,
    pub body: String,
    /// The review it was made in, and "terminal agent" when an agent made it.
    pub by: String,
    pub conversation: Vec<AgentMessage>,
}

#[derive(Serialize)]
pub struct AgentNote {
    pub path: String,
    pub lines: Option<String>,
    pub category: crate::store::Category,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<crate::store::Level>,
    pub title: String,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<crate::store::FlowStep>,
}

impl Review {
    /// A row's state; `None` for rows that don't need review: context and replaced rows.
    fn row_state(&self, fi: usize, row: usize) -> Option<Option<MarkState>> {
        let file = &self.files[fi];
        let pos = &file.rows[row];
        if pos.kind == RowKind::Context || pos.replaced {
            return None;
        }
        if self.checks[fi].as_ref().is_some_and(|c| c.current) {
            return Some(Some(MarkState::Reviewed));
        }
        let mark = self.matched.get(&RowRef::new(fi, row as u32));
        Some(
            mark.map(|m| m.state)
                .or(file.skipped.then_some(MarkState::Skipped)),
        )
    }

    /// The lines rows `start..=end` of file `fi` are on: new-side lines when there are any.
    fn lines(&self, fi: usize, start: usize, end: usize) -> String {
        let rows = &self.files[fi].rows[start..=end];
        let range = |lines: Vec<u32>| match (lines.iter().min(), lines.iter().max()) {
            (Some(a), Some(b)) if a == b => Some(format!("{a}")),
            (Some(a), Some(b)) => Some(format!("{a}-{b}")),
            _ => None,
        };
        let added: Vec<u32> = rows
            .iter()
            .filter(|r| r.kind != RowKind::Removed)
            .filter_map(|r| r.new_line)
            .collect();
        let removed: Vec<u32> = rows
            .iter()
            .filter(|r| r.kind == RowKind::Removed)
            .filter_map(|r| r.old_line)
            .collect();
        match (range(added), range(removed)) {
            (Some(new), Some(old)) => format!("{new} (and removed old lines {old})"),
            (Some(new), None) => new,
            (None, Some(old)) => format!("removed old lines {old}"),
            (None, None) => String::new(),
        }
    }

    fn span_lines(&self, fi: u32, rows: Option<RowSpan>) -> Option<String> {
        rows.map(|r| self.lines(fi as usize, r.start as usize, r.end as usize))
    }

    /// The revset and the diff's endpoints.
    pub fn target(&self) -> (String, Option<String>, String) {
        (self.revset.clone(), self.from.clone(), self.to.clone())
    }

    pub(super) fn summary(&self) -> Summary {
        let mut total = Counts::default();
        let files = self
            .files
            .iter()
            .enumerate()
            .map(|(fi, f)| {
                let mut counts = Counts::default();
                let mut unreviewed = Vec::new();
                let mut run: Option<(usize, usize)> = None;
                for row in 0..f.rows.len() {
                    let state = self.row_state(fi, row);
                    let count = match state {
                        None => None,
                        Some(Some(MarkState::Reviewed)) => Some(&mut counts.reviewed),
                        Some(Some(MarkState::Skipped)) => Some(&mut counts.skipped),
                        Some(Some(MarkState::Flagged)) => Some(&mut counts.flagged),
                        Some(None) => Some(&mut counts.unreviewed),
                    };
                    if let Some(c) = count {
                        *c += 1;
                    }
                    run = match (state, run) {
                        (Some(None), Some((a, _))) => Some((a, row)),
                        (Some(None), None) => Some((row, row)),
                        (_, Some((a, b))) => {
                            unreviewed.push(self.lines(fi, a, b));
                            None
                        }
                        (_, None) => None,
                    };
                }
                if let Some((a, b)) = run {
                    unreviewed.push(self.lines(fi, a, b));
                }
                total.reviewed += counts.reviewed;
                total.skipped += counts.skipped;
                total.flagged += counts.flagged;
                total.unreviewed += counts.unreviewed;
                FileSummary {
                    path: f.path.clone(),
                    old_path: f.old_path.clone(),
                    status: f.status,
                    checked: self.checks[fi].as_ref().is_some_and(|c| c.current),
                    rows: counts,
                    unreviewed,
                }
            })
            .collect();
        Summary {
            revset: self.revset.clone(),
            from: self.from.clone(),
            to: self.to.clone(),
            rows: total,
            files,
        }
    }

    pub(super) fn agent_comments(&self) -> Vec<AgentComment> {
        self.comments
            .iter()
            .map(|c| AgentComment {
                id: c.id,
                path: c.path.clone(),
                lines: self.span_lines(c.file, c.rows),
                body: c.body.clone(),
                by: c.review.clone(),
                conversation: c
                    .messages
                    .iter()
                    .map(|m| AgentMessage {
                        author: m.author,
                        text: m.text.clone(),
                    })
                    .collect(),
            })
            .collect()
    }

    pub(super) fn agent_notes(&self) -> Vec<AgentNote> {
        self.notes
            .iter()
            .map(|n| AgentNote {
                path: n.path.clone(),
                lines: self.span_lines(n.file, n.rows),
                category: n.category,
                level: n.level,
                title: n.title.clone(),
                body: n.body.clone(),
                flow: n.flow.clone(),
            })
            .collect()
    }

    /// The code of `rows` and the context between them, as a diff.
    pub(super) fn snippet(&self, rows: &[RowRef]) -> String {
        let (Some(first), Some(last)) = (rows.first(), rows.last()) else {
            return String::new();
        };
        let f = &self.files[last.file as usize];
        let start = if first.file == last.file {
            first.row
        } else {
            last.row
        };
        (start..=last.row)
            .map(|r| {
                let sign = match f.rows[r as usize].kind {
                    RowKind::Added => '+',
                    RowKind::Removed => '-',
                    RowKind::Context => ' ',
                };
                format!("{sign}{}", f.texts[r as usize])
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl RepoCtx {
    /// Runs `f` on the review open in `workspace`'s current review pane.
    pub fn with_current<T>(
        &self,
        workspace: Option<&str>,
        f: impl FnOnce(&Review) -> T,
    ) -> anyhow::Result<(u32, T)> {
        let id = self.current_review(workspace)?;
        let review = self.review(id)?;
        let review = review.lock().unwrap();
        Ok((id, f(&review)))
    }

    pub fn summary(&self, workspace: Option<&str>) -> anyhow::Result<Summary> {
        Ok(self.with_current(workspace, Review::summary)?.1)
    }

    pub fn agent_comments(&self, workspace: Option<&str>) -> anyhow::Result<Vec<AgentComment>> {
        Ok(self.with_current(workspace, Review::agent_comments)?.1)
    }

    pub fn agent_notes(&self, workspace: Option<&str>) -> anyhow::Result<Vec<AgentNote>> {
        Ok(self.with_current(workspace, Review::agent_notes)?.1)
    }

    /// Comments as a terminal agent on lines `start..=end` of `path` (new side). Returns the
    /// review and its new state.
    pub fn agent_comment(
        &self,
        workspace: Option<&str>,
        path: &str,
        start: u32,
        end: u32,
        body: &str,
    ) -> anyhow::Result<(u32, StateDto)> {
        let id = self.current_review(workspace)?;
        let state = self.comment_edit(id, |review, store| {
            let rows = review
                .rows_for(path, start.min(end), start.max(end))
                .with_context(|| format!("{path} isn't one of the review's changed files"))?;
            let (path, pins) = review.pins(store, &rows)?;
            let now = now_ms();
            store.add_comment(&Comment {
                id: 0,
                path,
                pins,
                body: body.to_string(),
                snippet: review.snippet(&rows),
                review: format!("{} · {BY_AGENT}", review.label),
                created_at: now,
                updated_at: now,
                workshop: Workshop::default(),
                github: None,
            })?;
            Ok(())
        })?;
        Ok((id, state))
    }

    /// Adds a terminal agent's reply to a comment's conversation.
    pub fn agent_reply(
        &self,
        workspace: Option<&str>,
        comment: u32,
        text: &str,
    ) -> anyhow::Result<(u32, StateDto)> {
        let id = self.current_review(workspace)?;
        let state = self.comment_edit(id, |_, store| {
            let mut c = store
                .comment(comment as i64)?
                .with_context(|| format!("no comment {comment}"))?;
            c.workshop.messages.push(Message {
                author: Author::Agent,
                text: text.to_string(),
                change: false,
            });
            Ok(store.set_workshop(c.id, &c.workshop)?)
        })?;
        Ok((id, state))
    }
}
