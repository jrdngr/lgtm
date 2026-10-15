//! What an agent in a terminal sees of a review and can do to it, through the MCP server.
//! Agents can read everything and add comments and walkthrough notes, but never mark rows.

use std::collections::BTreeMap;

use anyhow::{Context, bail};
use serde::Serialize;

use super::build::RowPos;
use super::comments::RowSpan;
use super::{RepoCtx, Review, RowRef, StateDto, now_ms};
use crate::diff::{FileStatus, RowKind};
use crate::settings::{self, marks};
use crate::store::{Author, Comment, MarkColor, Message, Workshop};

/// What comments and notes from a terminal agent record as their source review.
const BY_AGENT: &str = "terminal agent";

#[derive(Clone, Default, Serialize)]
pub struct Counts {
    /// Rows that count as done: marked in a done color, or in a checked or skipped file.
    pub done: u32,
    pub left: u32,
    /// Marked rows by the mark's label, or its color when it has none.
    pub marks: BTreeMap<String, u32>,
}

impl Counts {
    fn add(&mut self, other: &Counts) {
        self.done += other.done;
        self.left += other.left;
        for (k, n) in &other.marks {
            *self.marks.entry(k.clone()).or_default() += n;
        }
    }
}

/// What a changed row has: a mark, or none in a checked file, a skipped file, or neither.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RowState {
    Mark(MarkColor),
    Checked,
    Skipped,
    Unmarked,
}

impl RowState {
    pub(super) fn done(self, slots: &[marks::MarkSlot]) -> bool {
        match self {
            Self::Mark(color) => marks::is_done(slots, color),
            Self::Checked | Self::Skipped => true,
            Self::Unmarked => false,
        }
    }
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
    /// Line ranges of changed code that isn't done.
    pub left: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub revset: String,
    /// The diff's old and new commits.
    pub from: Option<String>,
    pub to: String,
    /// Changed rows by mark, across the review.
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
    /// Where the nearest change is, when `rows` (from `rows_for`) fell back to it because no
    /// changed row of `path` is in `start..=end`.
    fn change_outside(&self, path: &str, start: u32, end: u32, rows: &[RowRef]) -> Option<String> {
        let file = self.files.iter().find(|f| f.path == path)?;
        let removed = file.status == FileStatus::Removed;
        let line = |i: u32| {
            let r = &file.rows[i as usize];
            if removed { r.old_line } else { r.new_line }
        };
        let inside: Vec<u32> = (0..file.rows.len() as u32)
            .filter(|&i| line(i).is_some_and(|l| start <= l && l <= end))
            .collect();
        if let (Some(&lo), Some(&hi)) = (inside.first(), inside.last())
            && rows.iter().any(|r| lo <= r.row && r.row <= hi)
        {
            return None;
        }
        let row = rows.first()?.row;
        Some(match line(row) {
            Some(l) => format!("the nearest change is at line {l}"),
            None => format!(
                "the nearest change removes old line {}",
                file.rows[row as usize].old_line.unwrap_or_default()
            ),
        })
    }

    /// A row's state; `None` for rows that don't need review: context and replaced rows.
    /// A checked file's rows count as checked whatever their marks.
    pub(super) fn row_state(&self, fi: usize, row: usize) -> Option<RowState> {
        let file = &self.files[fi];
        let pos = &file.rows[row];
        if pos.kind == RowKind::Context || pos.replaced {
            return None;
        }
        if self.checks[fi].as_ref().is_some_and(|c| c.current) {
            return Some(RowState::Checked);
        }
        Some(match self.matched.get(&RowRef::new(fi, row as u32)) {
            Some(m) => RowState::Mark(m.color),
            None if file.skipped => RowState::Skipped,
            None => RowState::Unmarked,
        })
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
        let slots = settings::get().marks;
        let mut total = Counts::default();
        let files = self
            .files
            .iter()
            .enumerate()
            .map(|(fi, f)| {
                let mut counts = Counts::default();
                let mut left = Vec::new();
                let mut run: Option<(usize, usize)> = None;
                for row in 0..f.rows.len() {
                    let Some(state) = self.row_state(fi, row) else {
                        if let Some((a, b)) = run.take() {
                            left.push(self.lines(fi, a, b));
                        }
                        continue;
                    };
                    if let RowState::Mark(color) = state {
                        *counts.marks.entry(marks::label(&slots, color)).or_default() += 1;
                    }
                    if state.done(&slots) {
                        counts.done += 1;
                        if let Some((a, b)) = run.take() {
                            left.push(self.lines(fi, a, b));
                        }
                    } else {
                        counts.left += 1;
                        run = Some(run.map_or((row, row), |(a, _)| (a, row)));
                    }
                }
                if let Some((a, b)) = run {
                    left.push(self.lines(fi, a, b));
                }
                total.add(&counts);
                FileSummary {
                    path: f.path.clone(),
                    old_path: f.old_path.clone(),
                    status: f.status,
                    checked: self.checks[fi].as_ref().is_some_and(|c| c.current),
                    rows: counts,
                    left,
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
            .filter(|c| c.thread.is_none())
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

    /// The code of `rows` and the context between them, as a unified diff with a `@@` header
    /// wherever the line numbers jump.
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
        let rows = (start..=last.row).map(|r| (f.rows[r as usize], f.texts[r as usize].as_str()));
        unified(rows)
    }
}

/// Rows as unified diff hunks: a `@@ -old +new @@` header wherever the line numbers jump,
/// then each row with its sign.
fn unified<'a>(rows: impl Iterator<Item = (RowPos, &'a str)>) -> String {
    let mut out: Vec<String> = vec![];
    let mut hunk: Option<HunkHead> = None;
    for (pos, text) in rows {
        if !hunk.as_ref().is_some_and(|h| h.follows(&pos)) {
            if let Some(h) = hunk.take() {
                out[h.at] = h.header();
            }
            out.push(String::new());
            hunk = Some(HunkHead {
                at: out.len() - 1,
                ..HunkHead::default()
            });
        }
        hunk.as_mut().unwrap().add(&pos);
        let sign = match pos.kind {
            RowKind::Added => '+',
            RowKind::Removed => '-',
            RowKind::Context => ' ',
        };
        out.push(format!("{sign}{text}"));
    }
    if let Some(h) = hunk {
        out[h.at] = h.header();
    }
    out.join("\n")
}

/// A hunk being written: where its header goes, and each side's first line and line count.
#[derive(Default)]
struct HunkHead {
    at: usize,
    old: (u32, u32),
    new: (u32, u32),
}

impl HunkHead {
    fn add(&mut self, pos: &RowPos) {
        for (side, line) in [(&mut self.old, pos.old_line), (&mut self.new, pos.new_line)] {
            if let Some(l) = line {
                if side.1 == 0 {
                    side.0 = l;
                }
                side.1 += 1;
            }
        }
    }

    /// Whether `pos` comes right after the hunk's last line on each side it's on.
    fn follows(&self, pos: &RowPos) -> bool {
        let next = |(start, count): (u32, u32), line: Option<u32>| {
            count == 0 || line.is_none_or(|l| l == start + count)
        };
        next(self.old, pos.old_line) && next(self.new, pos.new_line)
    }

    fn header(&self) -> String {
        format!(
            "@@ -{},{} +{},{} @@",
            self.old.0, self.old.1, self.new.0, self.new.1
        )
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
            let (lo, hi) = (start.min(end), start.max(end));
            let rows = review
                .rows_for(path, lo, hi)
                .with_context(|| format!("{path} isn't one of the review's changed files"))?;
            if let Some(near) = review.change_outside(path, lo, hi, &rows) {
                bail!("lines {lo}-{hi} of {path} have no changes in the diff; {near}");
            }
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
                thread: None,
                done: false,
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
                updated: false,
                compose: false,
                before: None,
            });
            Ok(store.set_workshop(c.id, &c.workshop)?)
        })?;
        Ok((id, state))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(kind: RowKind, old: Option<u32>, new: Option<u32>) -> RowPos {
        RowPos {
            kind,
            old_line: old,
            new_line: new,
            replaced: false,
        }
    }

    #[test]
    fn unified_starts_a_hunk_where_line_numbers_jump() {
        let rows = [
            (row(RowKind::Context, Some(4), Some(4)), "a"),
            (row(RowKind::Removed, Some(5), None), "b"),
            (row(RowKind::Added, None, Some(5)), "B"),
            (row(RowKind::Added, None, Some(6)), "B2"),
            (row(RowKind::Context, Some(20), Some(21)), "z"),
        ];
        assert_eq!(
            unified(rows.into_iter()),
            "@@ -4,2 +4,3 @@\n a\n-b\n+B\n+B2\n@@ -20,1 +21,1 @@\n z"
        );
    }
}
