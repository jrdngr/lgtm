//! A review's rows as marks and checks leave them: which count as done, which are left, and
//! the summary agents read. The desktop and terminal apps build one over their own state.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

use super::RowRef;
use super::file::ReviewFile;
use super::matching::MatchedCheck;
use super::slots::{self, MarkSlot};
use crate::anchor::Side;
use crate::diff::{FileStatus, RowKind};
use crate::store::{Mark, MarkColor};

/// A review's files and what's matched onto them.
pub struct ReviewView<'a> {
    pub files: &'a [ReviewFile],
    pub matched: &'a HashMap<RowRef, Mark>,
    pub checks: &'a [Option<MatchedCheck>],
}

/// What a changed row has: a mark, or none in a checked file, a skipped file, or neither.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowState {
    Mark(MarkColor),
    Checked,
    Skipped,
    Unmarked,
}

impl RowState {
    pub fn done(self, slots: &[MarkSlot]) -> bool {
        match self {
            Self::Mark(color) => slots::is_done(slots, color),
            Self::Checked | Self::Skipped => true,
            Self::Unmarked => false,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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

/// What a summary says about the review besides its rows.
pub struct About<'a> {
    pub marks: bool,
    pub revset: &'a str,
    pub from: Option<&'a str>,
    pub to: &'a str,
}

impl ReviewView<'_> {
    /// The file's current check: `Some(true)` when an agent's.
    pub fn checked(&self, file: usize) -> Option<bool> {
        let c = self.checks.get(file)?.as_ref().filter(|c| c.current)?;
        Some(c.check.agent)
    }

    /// The index of the file at `path`, under its new name or its old one.
    pub fn file_index(&self, path: &str) -> Option<usize> {
        self.files
            .iter()
            .position(|f| f.path == path || f.old_path.as_deref() == Some(path))
    }

    /// A row's state; `None` for rows that don't need review: context and replaced rows.
    /// A checked file's rows count as checked whatever their marks, and without `marks` in
    /// use no row has one.
    pub fn row_state(&self, fi: usize, row: usize, marks: bool) -> Option<RowState> {
        let file = &self.files[fi];
        let pos = &file.rows[row];
        if pos.kind == RowKind::Context || pos.replaced {
            return None;
        }
        if self.checked(fi).is_some() {
            return Some(RowState::Checked);
        }
        let matched = marks.then(|| self.matched.get(&RowRef::new(fi, row as u32)));
        Some(match matched.flatten() {
            Some(m) => RowState::Mark(m.color),
            None if file.skipped => RowState::Skipped,
            None => RowState::Unmarked,
        })
    }

    /// The file's changed rows on new-side `lines` and old-side `removed` lines, as ranges
    /// from [`parse_range`].
    pub fn rows_on(
        &self,
        file: usize,
        lines: &[(u32, u32)],
        removed: &[(u32, u32)],
    ) -> Vec<RowRef> {
        let within =
            |ranges: &[(u32, u32)], line| ranges.iter().any(|&(a, b)| (a..=b).contains(&line));
        self.files[file]
            .keyed
            .iter()
            .filter(|k| match k.side {
                Side::New => within(lines, k.line),
                Side::Old => within(removed, k.line),
            })
            .map(|k| RowRef::new(file, k.row))
            .collect()
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

    /// The review as an agent sees it: each file and the counts across them.
    pub fn summary(&self, about: About, slots: &[MarkSlot]) -> Summary {
        let mut rows = Counts::default();
        let files = (0..self.files.len())
            .map(|fi| {
                let file = self.file_summary(fi, about.marks, slots);
                rows.add(&file.rows);
                file
            })
            .collect();
        Summary {
            marks: about.marks,
            revset: about.revset.to_string(),
            from: about.from.map(str::to_string),
            to: about.to.to_string(),
            rows,
            files,
        }
    }

    fn file_summary(&self, fi: usize, marks: bool, slots: &[MarkSlot]) -> FileSummary {
        let f = &self.files[fi];
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
                marked
                    .entry(slots::label(slots, c))
                    .or_default()
                    .push(self.lines(fi, a, b));
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
                *counts.marks.entry(slots::label(slots, c)).or_default() += 1;
            }
            if state.done(slots) {
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
            marked
                .entry(slots::label(slots, c))
                .or_default()
                .push(self.lines(fi, a, b));
        }
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
    }
}

/// A line range as get_review writes them: `12` or `12-20`.
pub fn parse_range(range: &str) -> anyhow::Result<(u32, u32)> {
    let bad = || format!("`{range}` isn't a line or a range like `12-20`");
    let (a, b) = range.split_once('-').unwrap_or((range, range));
    let a: u32 = a.trim().parse().map_err(|_| anyhow::anyhow!(bad()))?;
    let b: u32 = b.trim().parse().map_err(|_| anyhow::anyhow!(bad()))?;
    anyhow::ensure!(a > 0 && a <= b, bad());
    Ok((a, b))
}
