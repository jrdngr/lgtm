//! Reviewing a commit or range: its diff, unified or side by side, beside its file list.

mod draw;
mod marking;
pub mod marks;
pub mod rows;

use std::collections::HashSet;

use lgtm_core::jj::{Change, FileDiff};
use lgtm_core::store::Store;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::highlight::Sides;
use super::keybar::Hint;
use rows::Line;

pub enum Outcome {
    None,
    Back,
}

pub struct Review {
    pub revset: String,
    /// Tells this review's loads apart from an earlier one's.
    pub id: u64,
    changes: Vec<Change>,
    files: Vec<FileDiff>,
    sides: Vec<Option<Sides>>,
    error: Option<String>,
    folded: HashSet<(usize, usize)>,
    split: bool,
    lines: Vec<Line>,
    cursor: usize,
    top: usize,
    /// Whether keys move through the file list rather than the diff.
    in_list: bool,
    list_top: usize,
    /// Screen rows the diff had when last drawn.
    height: usize,
    marks: Option<marks::Marks>,
    /// A failure to show until the next key.
    notice: Option<String>,
}

pub const KEYS: &[(&str, &str)] = &[
    ("j k", "line"),
    ("Ctrl-d Ctrl-u", "half page"),
    ("g G", "top, bottom"),
    ("[ ]", "hunk"),
    ("{ }", "file"),
    ("Enter", "fold hunk"),
    ("Tab", "file list"),
    ("r f", "review, flag hunk"),
    ("R F", "review, flag file"),
    ("n", "next not reviewed"),
    ("t", "side by side"),
    ("q", "back"),
];

impl Review {
    pub fn new(id: u64, revset: String) -> Self {
        Self {
            revset,
            id,
            changes: vec![],
            files: vec![],
            sides: vec![],
            error: None,
            folded: HashSet::new(),
            split: false,
            lines: vec![],
            cursor: 0,
            top: 0,
            in_list: false,
            list_top: 0,
            height: 0,
            marks: None,
            notice: None,
        }
    }

    pub fn loaded(&mut self, result: anyhow::Result<(Vec<Change>, Vec<FileDiff>)>) {
        match result {
            Ok((changes, files)) => {
                self.changes = changes;
                self.sides = files.iter().map(|_| None).collect();
                self.files = files;
                self.error = None;
                self.relayout();
            }
            Err(e) => self.error = Some(format!("{e:#}")),
        }
    }

    pub fn highlighted(&mut self, file: usize, sides: Sides) {
        if let Some(slot) = self.sides.get_mut(file) {
            *slot = Some(sides);
        }
    }

    fn relayout(&mut self) {
        let spot = self.lines.get(self.cursor).map(|l| l.spot());
        self.lines = rows::lines(&self.files, &self.folded, self.split);
        self.cursor = spot.map_or(0, |s| rows::find(&self.lines, s));
    }

    fn current(&self) -> Option<Line> {
        self.lines.get(self.cursor).copied()
    }

    /// Moves the cursor to the next or previous line `is` accepts.
    fn jump(&mut self, forward: bool, is: impl Fn(&Line) -> bool) {
        let found = if forward {
            self.lines
                .iter()
                .skip(self.cursor + 1)
                .position(&is)
                .map(|i| self.cursor + 1 + i)
        } else {
            self.lines[..self.cursor].iter().rposition(&is)
        };
        if let Some(i) = found {
            self.cursor = i;
        }
    }

    fn by(&mut self, n: isize) {
        let last = self.lines.len().saturating_sub(1) as isize;
        self.cursor = (self.cursor as isize + n).clamp(0, last.max(0)) as usize;
    }

    /// Moves the cursor to the header of the file `n` files on from the cursor's.
    fn go_to_file(&mut self, n: isize) {
        let Some(file) = self.current().map(Line::file) else {
            return;
        };
        let last = self.files.len().saturating_sub(1) as isize;
        let to = (file as isize + n).clamp(0, last) as usize;
        self.cursor = rows::find(&self.lines, (to, None, None));
    }

    fn toggle_fold(&mut self) {
        let Some(line) = self.current() else { return };
        let Some(hunk) = line.hunk() else { return };
        let key = (line.file(), hunk);
        if !self.folded.remove(&key) {
            self.folded.insert(key);
        }
        self.lines = rows::lines(&self.files, &self.folded, self.split);
        self.cursor = rows::find(&self.lines, (key.0, Some(key.1), None));
    }

    pub fn key(&mut self, key: KeyEvent, store: Option<&mut Store>) -> Outcome {
        let half = (self.height / 2).max(1) as isize;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        self.notice = None;
        if let KeyCode::Char(c) = key.code
            && !ctrl
            && !self.in_list
            && self.mark_key(c, store)
        {
            return Outcome::None;
        }
        match key.code {
            KeyCode::Char('q') => return Outcome::Back,
            KeyCode::Tab => self.in_list = !self.in_list,
            KeyCode::Esc | KeyCode::Enter if self.in_list => self.in_list = false,
            KeyCode::Char('j') | KeyCode::Down if self.in_list => self.go_to_file(1),
            KeyCode::Char('k') | KeyCode::Up if self.in_list => self.go_to_file(-1),
            KeyCode::Char('j') | KeyCode::Down => self.by(1),
            KeyCode::Char('k') | KeyCode::Up => self.by(-1),
            KeyCode::Char('d') if ctrl => self.by(half),
            KeyCode::Char('u') if ctrl => self.by(-half),
            KeyCode::PageDown => self.by(self.height.max(1) as isize),
            KeyCode::PageUp => self.by(-(self.height.max(1) as isize)),
            KeyCode::Char('g') | KeyCode::Home => self.cursor = 0,
            KeyCode::Char('G') | KeyCode::End => self.by(isize::MAX / 2),
            KeyCode::Char(']') => self.jump(true, |l| matches!(l, Line::Hunk(..))),
            KeyCode::Char('[') => self.jump(false, |l| matches!(l, Line::Hunk(..))),
            KeyCode::Char('}') => self.jump(true, |l| matches!(l, Line::File(_))),
            KeyCode::Char('{') => self.jump(false, |l| matches!(l, Line::File(_))),
            KeyCode::Enter => self.toggle_fold(),
            KeyCode::Char('t') => {
                self.split = !self.split;
                self.relayout();
            }
            _ => {}
        }
        Outcome::None
    }

    /// The key bar. With Shift held, r and f become R and F.
    pub fn hints(&self, shift: bool) -> Vec<Hint<'static>> {
        let (review, flag) = if shift {
            (("R", "review file", false), ("F", "flag file", false))
        } else {
            (("r", "review hunk", false), ("f", "flag hunk", false))
        };
        vec![
            ("[ ]", "hunk", false),
            ("{ }", "file", false),
            review,
            flag,
            ("n", "next", false),
            ("t", "split", self.split),
            ("?", "keys", false),
            ("q", "back", false),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lgtm_core::diff::{FileStatus, Hunk, Row, RowKind};

    fn file(path: &str, hunks: usize) -> FileDiff {
        let row = |kind| Row {
            kind,
            old_line: Some(1),
            new_line: Some(1),
            text: String::new(),
        };
        FileDiff {
            path: path.into(),
            old_path: None,
            status: FileStatus::Modified,
            binary: false,
            conflict: false,
            old_exec: false,
            new_exec: false,
            blobs: None,
            hunks: (0..hunks)
                .map(|_| Hunk {
                    old_start: 1,
                    old_lines: 1,
                    new_start: 1,
                    new_lines: 1,
                    section: String::new(),
                    rows: vec![row(RowKind::Removed), row(RowKind::Added)],
                })
                .collect(),
        }
    }

    fn press(r: &mut Review, code: KeyCode) {
        r.key(KeyEvent::new(code, KeyModifiers::NONE), None);
    }

    fn review() -> Review {
        let mut r = Review::new(1, "@".into());
        r.loaded(Ok((vec![], vec![file("a.rs", 2), file("b.rs", 1)])));
        r
    }

    #[test]
    fn brackets_move_between_hunks_and_braces_between_files() {
        let mut r = review();
        press(&mut r, KeyCode::Char(']'));
        press(&mut r, KeyCode::Char(']'));
        assert_eq!(r.current(), Some(Line::Hunk(0, 1)));
        press(&mut r, KeyCode::Char('}'));
        assert_eq!(r.current(), Some(Line::File(1)));
        press(&mut r, KeyCode::Char('['));
        assert_eq!(r.current(), Some(Line::Hunk(0, 1)));
        press(&mut r, KeyCode::Char('{'));
        assert_eq!(r.current(), Some(Line::File(0)));
    }

    #[test]
    fn enter_folds_the_hunk_under_the_cursor_and_unfolds_it_again() {
        let mut r = review();
        press(&mut r, KeyCode::Char(']'));
        press(&mut r, KeyCode::Down);
        press(&mut r, KeyCode::Enter);
        assert_eq!(r.current(), Some(Line::Hunk(0, 0)));
        press(&mut r, KeyCode::Down);
        assert_eq!(r.current(), Some(Line::Hunk(0, 1)));
        press(&mut r, KeyCode::Up);
        press(&mut r, KeyCode::Enter);
        press(&mut r, KeyCode::Down);
        assert!(matches!(r.current(), Some(Line::Code { hunk: 0, .. })));
    }

    #[test]
    fn the_file_list_moves_a_file_at_a_time() {
        let mut r = review();
        press(&mut r, KeyCode::Char(']'));
        press(&mut r, KeyCode::Tab);
        press(&mut r, KeyCode::Down);
        assert_eq!(r.current(), Some(Line::File(1)));
        press(&mut r, KeyCode::Up);
        assert_eq!(r.current(), Some(Line::File(0)));
        press(&mut r, KeyCode::Enter);
        press(&mut r, KeyCode::Down);
        assert_eq!(r.current(), Some(Line::Hunk(0, 0)));
    }
}
