//! Review marks on hunks and files, in the store the desktop app shares: Reviewed marks rows
//! green, Flagged red, and checking a file (R) is the app's file check.

use std::collections::HashMap;

use lgtm_core::anchor::{MatchConfig, Side};
use lgtm_core::review::RowRef;
use lgtm_core::review::edits::Marking;
use lgtm_core::review::file::ReviewFile;
use lgtm_core::review::matching::{self, MatchedCheck};
use lgtm_core::store::{Mark, MarkColor, Store};

/// How far along a hunk or file is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    None,
    Partial,
    Reviewed,
    /// Done, some of it by an agent.
    Agent,
    Flagged,
    AgentFlagged,
}

/// Whether a mark's color counts the row as done, as the app's default mark settings have it.
fn done(color: MarkColor) -> bool {
    matches!(
        color,
        MarkColor::Green | MarkColor::Gray | MarkColor::AgentReviewed
    )
}

fn flagged(color: MarkColor) -> bool {
    matches!(color, MarkColor::Red | MarkColor::AgentFlagged)
}

#[derive(Default)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub left: Vec<u32>,
    pub removed_left: Vec<u32>,
}

pub struct Marks {
    files: Vec<ReviewFile>,
    cfg: MatchConfig,
    label: String,
    matched: HashMap<RowRef, Mark>,
    checks: Vec<Option<MatchedCheck>>,
    /// Per file, the index of each hunk's first row among the file's rows.
    starts: Vec<Vec<u32>>,
}

impl Marks {
    /// `hunk_rows` lists each file's hunks' row counts.
    pub fn new(
        files: Vec<ReviewFile>,
        cfg: MatchConfig,
        label: String,
        hunk_rows: Vec<Vec<usize>>,
    ) -> Self {
        let starts = hunk_rows
            .into_iter()
            .map(|lens| {
                lens.iter()
                    .scan(0u32, |at, &n| {
                        let start = *at;
                        *at += n as u32;
                        Some(start)
                    })
                    .collect()
            })
            .collect();
        Self {
            checks: files.iter().map(|_| None).collect(),
            files,
            cfg,
            label,
            matched: HashMap::new(),
            starts,
        }
    }

    pub fn rematch(&mut self, store: &Store) -> anyhow::Result<()> {
        let paths = matching::paths(&self.files);
        let marks = store.marks_for_paths(&paths)?;
        let checks = store.checks_for_paths(&paths)?;
        self.matched = matching::match_marks(store, &self.files, self.cfg, &marks);
        self.checks = matching::match_checks(&self.files, &checks);
        Ok(())
    }

    /// The file's changed rows, or the hunk's. With `all` false, only rows that need review:
    /// a removed row replaced by added ones doesn't.
    fn rows(&self, file: usize, hunk: Option<usize>, all: bool) -> Vec<RowRef> {
        let Some(f) = self.files.get(file) else {
            return vec![];
        };
        let span = hunk.and_then(|h| {
            let starts = self.starts.get(file)?;
            let end = starts.get(h + 1).copied().unwrap_or(f.rows.len() as u32);
            Some(*starts.get(h)?..end)
        });
        f.keyed
            .iter()
            .filter(|k| span.as_ref().is_none_or(|s| s.contains(&k.row)))
            .filter(|k| all || !f.rows[k.row as usize].replaced)
            .map(|k| RowRef::new(file, k.row))
            .collect()
    }

    /// The file's current check: `Some(true)` when an agent's.
    fn checked(&self, file: usize) -> Option<bool> {
        let c = self.checks.get(file)?.as_ref().filter(|c| c.current)?;
        Some(c.check.agent)
    }

    pub fn state(&self, file: usize, hunk: Option<usize>) -> State {
        let colors: Vec<Option<MarkColor>> = self
            .rows(file, hunk, false)
            .iter()
            .map(|r| self.matched.get(r).map(|m| m.color))
            .collect();
        if colors.contains(&Some(MarkColor::Red)) {
            return State::Flagged;
        }
        if colors.contains(&Some(MarkColor::AgentFlagged)) {
            return State::AgentFlagged;
        }
        match self.checked(file) {
            Some(false) => return State::Reviewed,
            Some(true) => return State::Agent,
            None => {}
        }
        let done_rows = colors.iter().filter(|c| c.is_some_and(done)).count();
        let agent = colors.contains(&Some(MarkColor::AgentReviewed));
        match done_rows {
            0 => State::None,
            n if n < colors.len() => State::Partial,
            _ if agent => State::Agent,
            _ => State::Reviewed,
        }
    }

    /// Done rows over rows that need review, across files the repo config doesn't skip.
    pub fn percent(&self) -> Option<u32> {
        let (mut done_rows, mut total) = (0, 0);
        for (i, f) in self.files.iter().enumerate() {
            if f.skipped {
                continue;
            }
            let rows = self.rows(i, None, false);
            total += rows.len();
            let checked = self.checked(i).is_some();
            done_rows += rows
                .iter()
                .filter(|r| match self.matched.get(r).map(|m| m.color) {
                    Some(c) => done(c) || checked && !flagged(c),
                    None => checked,
                })
                .count();
        }
        (total > 0).then(|| (done_rows * 100 / total) as u32)
    }

    /// The file's index by its path, or its path before a rename.
    pub fn file_index(&self, path: &str) -> Option<usize> {
        self.files
            .iter()
            .position(|f| f.path == path || f.old_path.as_deref() == Some(path))
    }

    pub fn path(&self, file: usize) -> &str {
        &self.files[file].path
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// How far along the file is: done rows, rows that need review, and the new-side and
    /// old-side lines still left.
    pub fn progress(&self, file: usize) -> Progress {
        let checked = self.checked(file).is_some();
        let mut p = Progress::default();
        for r in self.rows(file, None, false) {
            p.total += 1;
            let done_row = match self.matched.get(&r).map(|m| m.color) {
                Some(c) => done(c) || checked && !flagged(c),
                None => checked,
            };
            if done_row {
                p.done += 1;
                continue;
            }
            let k = self.files[file]
                .keyed
                .iter()
                .find(|k| k.row == r.row)
                .expect("rows come from keyed");
            match k.side {
                Side::New => p.left.push(k.line),
                Side::Old => p.removed_left.push(k.line),
            }
        }
        p
    }

    /// Checks or unchecks files as an agent. The user's checks stay as they are.
    pub fn agent_check(
        &mut self,
        store: &mut Store,
        files: &[u32],
        checked: bool,
    ) -> anyhow::Result<()> {
        let edit = self.marking(now_ms()).check_edit(files, checked, true);
        store.apply(&edit)?;
        self.rematch(store)
    }

    /// Marks the file's changed rows on `lines` (new side) and `removed` (old side) with an
    /// agent's `color`, or clears the agent's marks there. Returns how many rows that was.
    pub fn agent_mark(
        &mut self,
        store: &mut Store,
        file: usize,
        lines: &[u32],
        removed: &[u32],
        color: Option<MarkColor>,
    ) -> anyhow::Result<usize> {
        let rows: Vec<RowRef> = self.files[file]
            .keyed
            .iter()
            .filter(|k| match k.side {
                Side::New => lines.contains(&k.line),
                Side::Old => removed.contains(&k.line),
            })
            .map(|k| RowRef::new(file, k.row))
            .collect();
        let edit = self
            .marking(now_ms())
            .mark_edit(store, &rows, color, true)?;
        store.apply(&edit)?;
        self.rematch(store)?;
        Ok(rows.len())
    }

    fn marking(&self, now: i64) -> Marking<'_> {
        Marking {
            files: &self.files,
            matched: &self.matched,
            checks: &self.checks,
            label: &self.label,
            now,
        }
    }

    /// Marks the hunk's rows, or the file's, `color`, or clears them when they all have it.
    pub fn toggle(
        &mut self,
        store: &mut Store,
        file: usize,
        hunk: Option<usize>,
        color: MarkColor,
    ) -> anyhow::Result<()> {
        let rows = self.rows(file, hunk, true);
        let all = !rows.is_empty()
            && rows
                .iter()
                .all(|r| self.matched.get(r).is_some_and(|m| m.color == color));
        let edit =
            self.marking(now_ms())
                .mark_edit(store, &rows, (!all).then_some(color), false)?;
        store.apply(&edit)?;
        self.rematch(store)
    }

    /// Checks the file, or unchecks it when the user's check is current.
    pub fn toggle_check(&mut self, store: &mut Store, file: usize) -> anyhow::Result<()> {
        let checked = self.checked(file) == Some(false);
        let edit = self
            .marking(now_ms())
            .check_edit(&[file as u32], !checked, false);
        store.apply(&edit)?;
        self.rematch(store)
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lgtm_core::diff::{FileStatus, Hunk, Row, RowKind};
    use lgtm_core::jj::FileDiff;
    use lgtm_core::review::file::{Sides, Text};

    fn row(kind: RowKind, old: Option<u32>, new: Option<u32>, text: &str) -> Row {
        Row {
            kind,
            old_line: old,
            new_line: new,
            text: text.into(),
        }
    }

    /// `a.rs` with two hunks: one adding a line, one replacing a line.
    fn marks() -> Marks {
        let diff = FileDiff {
            path: "a.rs".into(),
            old_path: None,
            status: FileStatus::Modified,
            binary: false,
            conflict: false,
            old_exec: false,
            new_exec: false,
            blobs: None,
            hunks: vec![
                Hunk {
                    old_start: 1,
                    old_lines: 1,
                    new_start: 1,
                    new_lines: 2,
                    section: String::new(),
                    rows: vec![
                        row(RowKind::Context, Some(1), Some(1), "fn a() {}"),
                        row(RowKind::Added, None, Some(2), "fn b() {}"),
                    ],
                },
                Hunk {
                    old_start: 9,
                    old_lines: 1,
                    new_start: 10,
                    new_lines: 1,
                    section: String::new(),
                    rows: vec![
                        row(RowKind::Removed, Some(9), None, "let x = 1;"),
                        row(RowKind::Added, None, Some(10), "let x = 2;"),
                    ],
                },
            ],
        };
        let old: String = (1..=12).map(|i| format!("line {i}\n")).collect();
        let new: String = (1..=12).map(|i| format!("line {i} new\n")).collect();
        let file = ReviewFile::build(
            &Sides {
                old: Some("aaaa".into()),
                new: "bbbb".into(),
            },
            &diff,
            Text {
                text: Some(&old),
                parsed: None,
            },
            Text {
                text: Some(&new),
                parsed: None,
            },
            MatchConfig::default(),
            false,
        );
        Marks::new(
            vec![file],
            MatchConfig::default(),
            "@".into(),
            vec![vec![2, 2]],
        )
    }

    #[test]
    fn reviewing_a_hunk_marks_it_and_reviewing_it_again_clears_it() {
        let mut store = Store::in_memory();
        let mut m = marks();
        m.toggle(&mut store, 0, Some(0), MarkColor::Green).unwrap();
        assert_eq!(m.state(0, Some(0)), State::Reviewed);
        assert_eq!(m.state(0, Some(1)), State::None);
        assert_eq!(m.state(0, None), State::Partial);
        // The replaced row doesn't need review, so one of two rows is done.
        assert_eq!(m.percent(), Some(50));
        m.toggle(&mut store, 0, Some(0), MarkColor::Green).unwrap();
        assert_eq!(m.state(0, Some(0)), State::None);
        assert_eq!(m.percent(), Some(0));
    }

    #[test]
    fn a_flagged_row_outranks_a_check_and_flags_are_not_done() {
        let mut store = Store::in_memory();
        let mut m = marks();
        m.toggle_check(&mut store, 0).unwrap();
        assert_eq!(m.state(0, Some(1)), State::Reviewed);
        assert_eq!(m.percent(), Some(100));
        m.toggle(&mut store, 0, Some(1), MarkColor::Red).unwrap();
        assert_eq!(m.state(0, Some(1)), State::Flagged);
        assert_eq!(m.state(0, None), State::Flagged);
        assert_eq!(m.percent(), Some(50));
        m.toggle_check(&mut store, 0).unwrap();
        assert_eq!(m.state(0, Some(0)), State::None);
        assert_eq!(m.percent(), Some(0));
    }

    #[test]
    fn marks_made_elsewhere_show_up_on_rematch() {
        let mut store = Store::in_memory();
        let mut other = marks();
        other
            .toggle(&mut store, 0, None, MarkColor::AgentReviewed)
            .unwrap();
        let mut m = marks();
        assert_eq!(m.state(0, None), State::None);
        m.rematch(&store).unwrap();
        assert_eq!(m.state(0, None), State::Agent);
        assert_eq!(m.percent(), Some(100));
    }
}
