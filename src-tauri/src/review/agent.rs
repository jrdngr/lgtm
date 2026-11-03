//! What an agent in a terminal sees of a review and can do to it, through the MCP server.
//! Agents can read everything; what they can change is in `agent_edits`.

use std::collections::BTreeMap;

use serde::Serialize;

use super::{RepoCtx, Review, RowRef};
use crate::diff::{FileStatus, RowKind};
use crate::settings::{self, marks};
use crate::store::MarkColor;

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
    /// The check is an agent's, not the reviewer's.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub checked_by_agent: bool,
    /// The reviewer checked the file, but it changed since, so it's no longer checked.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub changed_since_checked: bool,
    pub rows: Counts,
    /// Line ranges of changed code that isn't done.
    pub left: Vec<String>,
    /// Line ranges of marked rows, by the mark's label. Empty when marks aren't in use.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub marked: BTreeMap<String, Vec<String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// Whether the reviewer marks lines. Without marks, files are reviewed only by checking
    /// them, and no row counts as marked.
    pub marks: bool,
    pub revset: String,
    /// The diff's old and new commits.
    pub from: Option<String>,
    pub to: String,
    /// Changed rows by mark, across the review.
    pub rows: Counts,
    pub files: Vec<FileSummary>,
}

impl Review {
    /// A row's state; `None` for rows that don't need review: context and replaced rows.
    /// A checked file's rows count as checked whatever their marks, and without `marks` in
    /// use no row has one.
    pub(super) fn row_state(&self, fi: usize, row: usize, marks: bool) -> Option<RowState> {
        let file = &self.files[fi];
        let pos = &file.rows[row];
        if pos.kind == RowKind::Context || pos.replaced {
            return None;
        }
        if self.checks[fi].as_ref().is_some_and(|c| c.current) {
            return Some(RowState::Checked);
        }
        let matched = marks.then(|| self.matched.get(&RowRef::new(fi, row as u32)));
        Some(match matched.flatten() {
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

    /// The review's state, as an agent sees it, with `marks` in use or not.
    pub(super) fn summary(&self, marks: bool) -> Summary {
        let slots = settings::get().marks;
        let mut total = Counts::default();
        let files = self
            .files
            .iter()
            .enumerate()
            .map(|(fi, f)| {
                let mut counts = Counts::default();
                let mut left = Vec::new();
                let mut marked: BTreeMap<String, Vec<String>> = BTreeMap::new();
                let mut run: Option<(usize, usize)> = None;
                let mut mark_run: Option<(MarkColor, usize, usize)> = None;
                for row in 0..f.rows.len() {
                    let state = self.row_state(fi, row, marks);
                    let color = match state {
                        Some(RowState::Mark(c)) => Some(c),
                        _ => None,
                    };
                    // Rows that don't need review keep a mark's run going.
                    if state.is_some() && mark_run.is_some_and(|(c, ..)| Some(c) != color) {
                        let (c, a, b) = mark_run.take().unwrap();
                        let label = marks::label(&slots, c);
                        marked.entry(label).or_default().push(self.lines(fi, a, b));
                    }
                    if let Some(c) = color {
                        mark_run = Some(mark_run.map_or((c, row, row), |(c, a, _)| (c, a, row)));
                    }
                    let Some(state) = state else {
                        if let Some((a, b)) = run.take() {
                            left.push(self.lines(fi, a, b));
                        }
                        continue;
                    };
                    if let Some(c) = color {
                        *counts.marks.entry(marks::label(&slots, c)).or_default() += 1;
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
                if let Some((c, a, b)) = mark_run {
                    let label = marks::label(&slots, c);
                    marked.entry(label).or_default().push(self.lines(fi, a, b));
                }
                total.add(&counts);
                let check = self.checks[fi].as_ref();
                FileSummary {
                    path: f.path.clone(),
                    old_path: f.old_path.clone(),
                    status: f.status,
                    checked: check.is_some_and(|c| c.current),
                    checked_by_agent: check.is_some_and(|c| c.check.agent),
                    changed_since_checked: check.is_some_and(|c| !c.current),
                    rows: counts,
                    left,
                    marked,
                }
            })
            .collect();
        Summary {
            marks,
            revset: self.revset.clone(),
            from: self.from.clone(),
            to: self.to.clone(),
            rows: total,
            files,
        }
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

    /// The current review's state, with marks in use as the settings say.
    pub fn summary(&self, workspace: Option<&str>) -> anyhow::Result<Summary> {
        self.summary_with(workspace, settings::get().review_marks)
    }

    pub fn summary_with(&self, workspace: Option<&str>, marks: bool) -> anyhow::Result<Summary> {
        Ok(self.with_current(workspace, |r| r.summary(marks))?.1)
    }
}
