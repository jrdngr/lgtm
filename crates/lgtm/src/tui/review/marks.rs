//! Review marks on hunks and files, in the repo's store: Reviewed marks rows green, Flagged
//! red, and R checks a file.

use std::collections::HashMap;

use lgtm_core::anchor::MatchConfig;
use lgtm_core::review::RowRef;
use lgtm_core::review::edits::Marking;
use lgtm_core::review::file::ReviewFile;
use lgtm_core::review::matching::{self, MatchedCheck};
use lgtm_core::review::place::Placer;
use lgtm_core::review::view::ReviewView;
use lgtm_core::store::{Edit, Mark, MarkColor, Store};

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

/// Whether a mark's color counts the row as done.
fn done(color: MarkColor) -> bool {
    matches!(
        color,
        MarkColor::Green | MarkColor::Gray | MarkColor::AgentReviewed
    )
}

fn flagged(color: MarkColor) -> bool {
    matches!(color, MarkColor::Red | MarkColor::AgentFlagged)
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

    pub fn view(&self) -> ReviewView<'_> {
        ReviewView {
            files: &self.files,
            matched: &self.matched,
            checks: &self.checks,
        }
    }

    /// The file's current check: `Some(true)` when an agent's.
    pub(super) fn checked(&self, file: usize) -> Option<bool> {
        self.view().checked(file)
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
        // The user's check outranks an agent's flags; an agent's check doesn't.
        if self.checked(file) == Some(false) {
            return State::Reviewed;
        }
        if colors.contains(&Some(MarkColor::AgentFlagged)) {
            return State::AgentFlagged;
        }
        if self.checked(file) == Some(true) {
            return State::Agent;
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
            let checked = self.checked(i);
            done_rows += rows
                .iter()
                .filter(|r| match self.matched.get(r).map(|m| m.color) {
                    // The user's check covers an agent's flags too.
                    Some(c) => match checked {
                        Some(false) => c != MarkColor::Red,
                        Some(true) => !flagged(c),
                        None => done(c),
                    },
                    None => checked.is_some(),
                })
                .count();
        }
        (total > 0).then(|| (done_rows * 100 / total) as u32)
    }

    pub fn path(&self, file: usize) -> &str {
        &self.files[file].path
    }

    pub fn placer(&self) -> Placer<'_> {
        Placer {
            files: &self.files,
            cfg: self.cfg,
        }
    }

    /// The hunk a row of the file is in, and its index among the hunk's rows.
    pub fn hunk_row(&self, file: usize, row: u32) -> Option<(usize, usize)> {
        let starts = self.starts.get(file)?;
        let hunk = starts.iter().rposition(|&s| s <= row)?;
        Some((hunk, (row - starts[hunk]) as usize))
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
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

    /// Marks the file's changed rows on `lines` (new side) and `removed` (old side) ranges
    /// with an agent's `color`, or clears the agent's marks there. Returns how many rows that
    /// was.
    pub fn agent_mark(
        &mut self,
        store: &mut Store,
        file: usize,
        lines: &[(u32, u32)],
        removed: &[(u32, u32)],
        color: Option<MarkColor>,
    ) -> anyhow::Result<usize> {
        let rows = self.view().rows_on(file, lines, removed);
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
    ) -> anyhow::Result<Edit> {
        let rows = self.rows(file, hunk, true);
        self.toggle_rows(store, rows, color)
    }

    /// Marks every row of the files `color`, or clears them when they all have it.
    pub fn toggle_files(
        &mut self,
        store: &mut Store,
        files: &[usize],
        color: MarkColor,
    ) -> anyhow::Result<Edit> {
        let rows = files
            .iter()
            .flat_map(|&f| self.rows(f, None, true))
            .collect();
        self.toggle_rows(store, rows, color)
    }

    fn toggle_rows(
        &mut self,
        store: &mut Store,
        rows: Vec<RowRef>,
        color: MarkColor,
    ) -> anyhow::Result<Edit> {
        let all = !rows.is_empty()
            && rows
                .iter()
                .all(|r| self.matched.get(r).is_some_and(|m| m.color == color));
        self.set_rows(store, &rows, (!all).then_some(color))
    }

    fn set_rows(
        &mut self,
        store: &mut Store,
        rows: &[RowRef],
        color: Option<MarkColor>,
    ) -> anyhow::Result<Edit> {
        let edit = self
            .marking(now_ms())
            .mark_edit(store, rows, color, false)?;
        self.apply(store, &edit)
    }

    /// Applies the user's `edit`. Returns it as stored, for undo.
    fn apply(&mut self, store: &mut Store, edit: &Edit) -> anyhow::Result<Edit> {
        let done = store.apply(edit)?;
        self.rematch(store)?;
        Ok(done)
    }

    /// Marks every row of the files `color`.
    pub fn mark_files(
        &mut self,
        store: &mut Store,
        files: &[usize],
        color: MarkColor,
    ) -> anyhow::Result<Edit> {
        let rows: Vec<RowRef> = files
            .iter()
            .flat_map(|&f| self.rows(f, None, true))
            .collect();
        self.set_rows(store, &rows, Some(color))
    }

    /// Checks the files, or unchecks them when the user's check is current on each.
    pub fn toggle_checks(&mut self, store: &mut Store, files: &[usize]) -> anyhow::Result<Edit> {
        let all = files.iter().all(|&f| self.checked(f) == Some(false));
        self.set_checks(store, files, !all)
    }

    pub fn set_checks(
        &mut self,
        store: &mut Store,
        files: &[usize],
        checked: bool,
    ) -> anyhow::Result<Edit> {
        let files: Vec<u32> = files.iter().map(|&f| f as u32).collect();
        let edit = self.marking(now_ms()).check_edit(&files, checked, false);
        self.apply(store, &edit)
    }

    /// Clears every mark and check on the files, the agent's too.
    pub fn clear(&mut self, store: &mut Store, files: &[usize]) -> anyhow::Result<Edit> {
        let rows: Vec<RowRef> = files
            .iter()
            .flat_map(|&f| self.rows(f, None, true))
            .collect();
        let marking = self.marking(now_ms());
        let mut edit = marking.mark_edit(store, &rows, None, false)?;
        let ids: Vec<u32> = files.iter().map(|&f| f as u32).collect();
        edit.checks_removed = marking.check_edit(&ids, false, false).checks_removed;
        self.apply(store, &edit)
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

#[cfg(test)]
pub(super) mod tests {
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
    pub(in super::super) fn marks() -> Marks {
        marks_for(&["a.rs"])
    }

    /// The same changes in each of `paths`.
    pub(in super::super) fn marks_for(paths: &[&str]) -> Marks {
        let diff = |path: &str| FileDiff {
            path: path.into(),
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
        let file = |path: &str| {
            ReviewFile::build(
                &Sides {
                    old: Some("aaaa".into()),
                    new: "bbbb".into(),
                },
                &diff(path),
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
            )
        };
        Marks::new(
            paths.iter().map(|p| file(p)).collect(),
            MatchConfig::default(),
            "@".into(),
            paths.iter().map(|_| vec![2, 2]).collect(),
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
        m.toggle_checks(&mut store, &[0]).unwrap();
        assert_eq!(m.state(0, Some(1)), State::Reviewed);
        assert_eq!(m.percent(), Some(100));
        m.toggle(&mut store, 0, Some(1), MarkColor::Red).unwrap();
        assert_eq!(m.state(0, Some(1)), State::Flagged);
        assert_eq!(m.state(0, None), State::Flagged);
        assert_eq!(m.percent(), Some(50));
        m.toggle_checks(&mut store, &[0]).unwrap();
        assert_eq!(m.state(0, Some(0)), State::None);
        assert_eq!(m.percent(), Some(0));
    }

    #[test]
    fn a_marks_step_undoes_and_redoes_a_check_over_a_flag() {
        use super::super::undo::Step;
        let mut store = Store::in_memory();
        let mut m = marks();
        m.toggle(&mut store, 0, Some(1), MarkColor::Red).unwrap();
        let checked = m.toggle(&mut store, 0, Some(1), MarkColor::Green).unwrap();
        assert_eq!(m.state(0, Some(1)), State::Reviewed);
        let step = Step::Marks(checked).run(&mut store, true).unwrap();
        m.rematch(&store).unwrap();
        assert_eq!(m.state(0, Some(1)), State::Flagged);
        let step = step.run(&mut store, false).unwrap();
        m.rematch(&store).unwrap();
        assert_eq!(m.state(0, Some(1)), State::Reviewed);
        step.run(&mut store, true).unwrap();
        m.rematch(&store).unwrap();
        assert_eq!(m.state(0, Some(1)), State::Flagged);
    }

    #[test]
    fn the_users_check_outranks_an_agents_flags() {
        let mut store = Store::in_memory();
        let mut m = marks();
        m.agent_mark(&mut store, 0, &[(2, 2)], &[], Some(MarkColor::AgentFlagged))
            .unwrap();
        assert_eq!(m.state(0, None), State::AgentFlagged);
        m.agent_check(&mut store, &[0], true).unwrap();
        assert_eq!(m.state(0, None), State::AgentFlagged);
        m.toggle_checks(&mut store, &[0]).unwrap();
        assert_eq!(m.state(0, None), State::Reviewed);
        assert_eq!(m.percent(), Some(100));
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
