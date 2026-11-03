//! Reviewing a commit or range: its diff, unified or side by side, beside its file list.

mod agent;
mod command;
mod comments;
mod draw;
mod edit;
mod list;
mod marking;
pub mod marks;
mod range;
mod reload;
pub mod rows;
mod threads;
mod tree;
mod wrap;

use std::collections::{HashMap, HashSet};

use lgtm_core::diff::RowKind;
use lgtm_core::jj::{Change, FileDiff};
use lgtm_core::store::Store;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::{Position, Rect};

use super::highlight::Sides;
use super::keybar::Hint;
use super::load::Target;
use super::settings::Settings;
use rows::Line;

pub enum Outcome {
    None,
    Back,
    /// Open the editor on this file, a path in the workspace, at this line.
    Edit(String, u32),
    /// Pull the PR's threads from GitHub.
    Pull,
}

pub struct Review {
    pub target: Target,
    /// What the review is called: its revset, or its versions' names.
    pub revset: String,
    /// Tells this review's loads apart from an earlier one's.
    pub id: u64,
    changes: Vec<Change>,
    files: Vec<FileDiff>,
    sides: Vec<Option<Sides>>,
    error: Option<String>,
    /// Folded files, by path, so they stay folded across reloads.
    folded: HashSet<String>,
    split: bool,
    lines: Vec<Line>,
    cursor: usize,
    top: usize,
    /// Whether the mouse put the cursor where it is, so the view stays put around it.
    pinned: bool,
    /// The file whose header is pinned over the diff's top line, when that line isn't one.
    sticky: Option<usize>,
    /// Where a selection of lines started, while there is one.
    anchor: Option<Line>,
    /// Whether keys move through the file list rather than the diff.
    in_list: bool,
    /// What's picked in the file list, when it isn't the cursor's file.
    list_pick: Option<list::Pick>,
    /// Whether reviewed files are set apart at the bottom of the list, and `{` `}` skip them.
    group_reviewed: bool,
    reviewed_open: bool,
    /// Folders closed in the file list, by path.
    closed_dirs: HashSet<String>,
    list_top: usize,
    /// Screen rows the diff had when last drawn.
    height: usize,
    /// Columns the diff had when last drawn, which notes wrap to.
    width: usize,
    /// Where the file list and the diff were last drawn, for the mouse.
    list_area: Rect,
    diff_area: Rect,
    marks: Option<marks::Marks>,
    /// Whether marks have been matched once, after which reviewed hunks stay as folded as
    /// the user left them.
    built_once: bool,
    /// Open comments for the agent, each with where it is.
    comments: Vec<comments::Placed>,
    input: Option<comments::Input>,
    cmd: Option<command::CmdLine>,
    confirm: Option<command::Confirm>,
    /// The PR whose threads show, as `(owner/name, number)`.
    pr: Option<(String, u32)>,
    threads: Vec<threads::Thread>,
    hide_pr: bool,
    /// Whether comments the agent resolved show too.
    show_resolved: bool,
    /// A failure to show until the next key.
    pub notice: Option<String>,
}

pub const KEYS: &[(&str, &str)] = &[
    ("j k", "line"),
    ("Ctrl-d Ctrl-u", "half page"),
    ("g G", "top, bottom"),
    ("[ ]", "hunk"),
    ("{ }", "file"),
    ("Enter", "fold file, edit comment"),
    ("Tab", "file list"),
    ("Enter", "open, close folder (file list)"),
    ("r f", "review, flag hunk"),
    ("R F", "review, flag file or folder"),
    ("h", "hide reviewed files"),
    ("J K", "select lines"),
    ("c", "comment for the agent, on the selected lines"),
    ("C", "next comment"),
    ("d", "delete comment"),
    ("e", "edit"),
    ("t", "side by side"),
    ("p", "pull PR comments"),
    ("H", "hide PR comments"),
    (":", "command (:? lists them)"),
    ("q", "back"),
];

impl Review {
    /// The toggles, to keep for the next review and run.
    pub fn settings(&self) -> Settings {
        Settings {
            hide_reviewed: self.group_reviewed,
            split: self.split,
            hide_pr: self.hide_pr,
            show_resolved: self.show_resolved,
        }
    }

    pub fn apply(&mut self, s: &Settings) {
        self.group_reviewed = s.hide_reviewed;
        self.split = s.split;
        self.hide_pr = s.hide_pr;
        self.show_resolved = s.show_resolved;
    }

    pub fn new(id: u64, target: Target) -> Self {
        Self {
            revset: target.name(),
            target,
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
            pinned: false,
            sticky: None,
            anchor: None,
            in_list: false,
            list_pick: None,
            group_reviewed: false,
            reviewed_open: false,
            closed_dirs: HashSet::new(),
            list_top: 0,
            height: 0,
            width: 80,
            list_area: Rect::default(),
            diff_area: Rect::default(),
            marks: None,
            built_once: false,
            comments: vec![],
            input: None,
            cmd: None,
            confirm: None,
            pr: None,
            threads: vec![],
            hide_pr: false,
            show_resolved: false,
            notice: None,
        }
    }

    /// Takes the review's changes and diff, first or after a reload. A reload keeps the
    /// cursor on the same line of the same file, folded hunks folded, and each file's colors
    /// until its new ones come.
    pub fn loaded(&mut self, result: anyhow::Result<(Vec<Change>, Vec<FileDiff>)>) {
        let (changes, files) = match result {
            Ok(loaded) => loaded,
            Err(e) => {
                self.error = Some(format!("{e:#}"));
                return;
            }
        };
        let at = self.current().map(|l| reload::Spot::of(&self.files, l));
        let mut old_sides: HashMap<String, Sides> = self
            .files
            .iter()
            .zip(std::mem::take(&mut self.sides))
            .filter_map(|(f, s)| Some((f.path.clone(), s?)))
            .collect();
        self.sides = files.iter().map(|f| old_sides.remove(&f.path)).collect();
        self.changes = changes;
        self.files = files;
        self.error = None;
        self.lines = self.layout();
        self.cursor = at.map_or(0, |at| at.find(&self.files, &self.lines));
    }

    /// The PR whose threads show, as `(owner/name, number)`.
    pub fn pr(&self) -> Option<&(String, u32)> {
        self.pr.as_ref()
    }

    pub fn set_pr(&mut self, pr: (String, u32)) {
        self.pr = Some(pr);
    }

    /// The review's bookmarks, newest commit first.
    pub fn bookmarks(&self) -> Vec<String> {
        self.changes
            .iter()
            .rev()
            .flat_map(|c| &c.bookmarks)
            .map(|b| b.trim_end_matches('*').to_string())
            .collect()
    }

    /// Whether the review shows the working copy, whose edits it follows.
    pub fn working_copy(&self) -> bool {
        self.changes.iter().any(|c| c.working_copy)
    }

    pub fn highlighted(&mut self, file: usize, sides: Sides) {
        if let Some(slot) = self.sides.get_mut(file) {
            *slot = Some(sides);
        }
    }

    fn layout(&self) -> Vec<Line> {
        let folded: HashSet<usize> = (0..self.files.len())
            .filter(|&f| self.folded.contains(&self.files[f].path))
            .collect();
        rows::lines_with(&self.files, &folded, self.split, &self.notes())
    }

    fn relayout(&mut self) {
        let spot = self.lines.get(self.cursor).map(|l| l.spot());
        self.lines = self.layout();
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

    /// Moves to the first line of the next open comment or PR thread, wrapping around.
    fn next_comment(&mut self) {
        let starts: Vec<usize> = (0..self.lines.len())
            .filter(|&i| {
                matches!(
                    self.lines[i],
                    Line::Note {
                        key: rows::NoteKey::Comment(_) | rows::NoteKey::Thread(_),
                        part: 0,
                        ..
                    }
                )
            })
            .collect();
        let next = starts.iter().find(|&&i| i > self.cursor).or(starts.first());
        if let Some(&i) = next {
            self.cursor = i;
        }
    }

    /// Moves to the next or previous file's header, past files set apart as reviewed.
    fn jump_file(&mut self, forward: bool) {
        let skipped: HashSet<usize> = (0..self.files.len()).filter(|&f| self.skipped(f)).collect();
        self.jump(
            forward,
            |l| matches!(l, Line::File(f) if !skipped.contains(f)),
        );
    }

    fn by(&mut self, n: isize) {
        let last = self.lines.len().saturating_sub(1) as isize;
        self.cursor = (self.cursor as isize + n).clamp(0, last.max(0)) as usize;
    }

    /// The hunk `line` is in. A file with one hunk has no divider, so its header stands for it.
    fn hunk_at(&self, line: Line) -> Option<usize> {
        line.hunk().or(match line {
            Line::File(f) if self.files[f].hunks.len() == 1 => Some(0),
            _ => None,
        })
    }

    /// Where each hunk starts on screen: its first changed line. Folded files have none.
    fn hunk_starts(&self) -> Vec<usize> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for (i, &line) in self.lines.iter().enumerate() {
            let start = match line {
                Line::Code {
                    file,
                    hunk,
                    old,
                    new,
                } => {
                    let rows = &self.files[file].hunks[hunk].rows;
                    let changed = [old, new]
                        .into_iter()
                        .flatten()
                        .any(|r| rows[r].kind != RowKind::Context);
                    changed.then_some((file, hunk))
                }
                Line::File(_) | Line::Hunk(..) | Line::Note { .. } => None,
            };
            if let Some(key) = start
                && seen.insert(key)
            {
                out.push(i);
            }
        }
        out
    }

    /// Moves the cursor to the next or previous hunk's start.
    fn jump_hunk(&mut self, forward: bool) {
        let starts = self.hunk_starts();
        let found = if forward {
            starts.into_iter().find(|&i| i > self.cursor)
        } else {
            starts.into_iter().rev().find(|&i| i < self.cursor)
        };
        if let Some(i) = found {
            self.cursor = i;
        }
    }

    pub(super) fn toggle_file(&mut self, file: usize) {
        let path = self.files[file].path.clone();
        if !self.folded.remove(&path) {
            self.folded.insert(path);
        }
        self.lines = self.layout();
        self.cursor = rows::find(&self.lines, (file, None, None));
    }

    pub(super) fn set_folded(&mut self, file: usize, folded: bool) {
        let path = self.files[file].path.clone();
        if folded {
            self.folded.insert(path);
        } else {
            self.folded.remove(&path);
        }
        self.relayout();
    }

    pub fn key(&mut self, key: KeyEvent, mut store: Option<&mut Store>) -> Outcome {
        let half = (self.height / 2).max(1) as isize;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        self.notice = None;
        self.pinned = false;
        if self.cmd.is_some() || self.confirm.is_some() {
            return self.command_key(key, store);
        }
        if self.typing() {
            self.input_key(key, store.as_deref());
            return Outcome::None;
        }
        if !matches!(key.code, KeyCode::Char('J' | 'K' | 'c')) {
            self.anchor = None;
        }
        if let KeyCode::Char(c) = key.code
            && !ctrl
            && !self.in_list
            && self.mark_key(c, store.as_deref_mut())
        {
            return Outcome::None;
        }
        match key.code {
            KeyCode::Char('q') => return Outcome::Back,
            KeyCode::Char(':') => self.start_command(),
            KeyCode::Tab => self.set_in_list(!self.in_list),
            KeyCode::Enter if self.in_list && self.list_pick.is_some() => {
                match self.list_pick.clone() {
                    Some(list::Pick::Dir(dir)) => self.toggle_dir(&dir),
                    _ => self.toggle_reviewed_open(),
                }
            }
            KeyCode::Esc | KeyCode::Enter if self.in_list => self.set_in_list(false),
            KeyCode::Char('j') | KeyCode::Down if self.in_list => self.list_step(1),
            KeyCode::Char('k') | KeyCode::Up if self.in_list => self.list_step(-1),
            KeyCode::Char(c @ ('R' | 'F')) if self.in_list => self.list_mark(c, store),
            KeyCode::Char('J') if !self.in_list => self.extend(1),
            KeyCode::Char('K') if !self.in_list => self.extend(-1),
            KeyCode::Char('j') | KeyCode::Down => self.by(1),
            KeyCode::Char('k') | KeyCode::Up => self.by(-1),
            KeyCode::Char('d') if ctrl => self.by(half),
            KeyCode::Char('u') if ctrl => self.by(-half),
            KeyCode::PageDown => self.by(self.height.max(1) as isize),
            KeyCode::PageUp => self.by(-(self.height.max(1) as isize)),
            KeyCode::Char('g') | KeyCode::Home => self.cursor = 0,
            KeyCode::Char('G') | KeyCode::End => self.by(isize::MAX / 2),
            KeyCode::Char(']') => self.jump_hunk(true),
            KeyCode::Char('[') => self.jump_hunk(false),
            KeyCode::Char('}') => self.jump_file(true),
            KeyCode::Char('{') => self.jump_file(false),
            KeyCode::Char('h') => self.toggle_group(),
            KeyCode::Enter => match self.current() {
                Some(Line::File(f)) => self.toggle_file(f),
                Some(Line::Note {
                    key: rows::NoteKey::Comment(n),
                    ..
                }) => self.edit_comment(n),
                _ => {}
            },
            KeyCode::Char('c') if !ctrl => self.start_comment(store.as_deref()),
            KeyCode::Char('C') => self.next_comment(),
            KeyCode::Char('d') if !ctrl => self.delete_comment(store.as_deref()),
            KeyCode::Char('e') => {
                if let Some((path, line)) = self.edit_target() {
                    return Outcome::Edit(path, line);
                }
            }
            KeyCode::Char('p') => return Outcome::Pull,
            KeyCode::Char('H') => self.toggle_pr(),
            KeyCode::Char('t') => {
                self.split = !self.split;
                self.relayout();
            }
            _ => {}
        }
        Outcome::None
    }

    /// A click on a file in the list goes to it, a click on a file's header folds or unfolds
    /// it, a click elsewhere in the diff moves the cursor there, a drag selects lines, and the
    /// wheel scrolls.
    pub fn mouse(&mut self, ev: MouseEvent) {
        let at = Position::new(ev.column, ev.row);
        self.pinned = true;
        if matches!(ev.kind, MouseEventKind::Down(_)) {
            self.anchor = None;
        }
        match ev.kind {
            MouseEventKind::ScrollDown => self.scroll(3),
            MouseEventKind::ScrollUp => self.scroll(-3),
            MouseEventKind::Down(MouseButton::Left) if self.list_area.contains(at) => {
                if let Some(entry) = self.list_entry_at(ev.row) {
                    self.in_list = !matches!(entry, tree::Entry::File { .. });
                    self.pick_entry(&entry);
                    match &entry {
                        tree::Entry::Reviewed { .. } => self.toggle_reviewed_open(),
                        tree::Entry::Dir { path, .. } => self.toggle_dir(path),
                        tree::Entry::File { .. } => {}
                    }
                }
            }
            MouseEventKind::Down(MouseButton::Left) if self.diff_area.contains(at) => {
                let i = self.top + (ev.row - self.diff_area.y) as usize;
                let line = match self.sticky {
                    Some(f) if ev.row == self.diff_area.y => Some(&Line::File(f)),
                    _ => self.lines.get(i),
                };
                match line {
                    Some(&Line::File(f)) => self.toggle_file(f),
                    Some(&l) => {
                        self.cursor = i;
                        self.anchor = Some(l);
                    }
                    None => {}
                }
                self.set_in_list(false);
            }
            // Dragging selects lines, scrolling at the diff's edges.
            MouseEventKind::Drag(MouseButton::Left) if self.anchor.is_some() => {
                let area = self.diff_area;
                if ev.row < area.y + 1 {
                    self.scroll(-1);
                } else if ev.row >= area.bottom().saturating_sub(1) {
                    self.scroll(1);
                }
                let row = ev.row.clamp(area.y, area.bottom().saturating_sub(1));
                let i = self.top + (row - area.y) as usize;
                self.cursor = i.min(self.lines.len().saturating_sub(1));
            }
            // A click without a drag selects nothing.
            MouseEventKind::Up(MouseButton::Left) if self.anchor == self.current() => {
                self.anchor = None;
            }
            _ => {}
        }
    }

    fn set_in_list(&mut self, on: bool) {
        self.in_list = on;
        if !on {
            self.list_pick = None;
        }
    }

    /// Scrolls the diff `n` lines, the cursor moving with it to stay where it is on screen.
    fn scroll(&mut self, n: isize) {
        let max_top = self.lines.len().saturating_sub(self.height);
        let top = (self.top as isize + n).clamp(0, max_top as isize) as usize;
        let moved = top as isize - self.top as isize;
        self.top = top;
        let last = self.lines.len().saturating_sub(1);
        self.cursor = (self.cursor as isize + moved).clamp(0, last as isize) as usize;
    }

    /// The key bar. With Shift held, r and f become R and F.
    pub fn hints(&self, shift: bool) -> Vec<Hint<'static>> {
        if self.cmd.is_some() {
            return vec![
                ("Tab", "complete", false),
                ("Enter", "run", false),
                ("Esc", "cancel", false),
            ];
        }
        if self.typing() {
            return vec![("Enter", "save", false), ("Esc", "cancel", false)];
        }
        let (review, flag) = if shift {
            (("R", "review file", false), ("F", "flag file", false))
        } else {
            (("r", "review hunk", false), ("f", "flag hunk", false))
        };
        let mut hints = vec![
            ("[ ]", "hunk", false),
            ("{ }", "file", false),
            review,
            flag,
            ("c", "comment", false),
            ("e", "edit", false),
            ("t", "split", self.split),
            ("h", "hide reviewed", self.group_reviewed),
        ];
        if self.pr.is_some() {
            hints.push(("H", "hide PR", self.hide_pr));
        }
        hints.extend([("?", "keys", false), ("q", "back", false)]);
        hints
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
        let mut r = Review::new(1, Target::Revset("@".into()));
        r.loaded(Ok((vec![], vec![file("a.rs", 2), file("b.rs", 1)])));
        r
    }

    fn on_change(file: usize, hunk: usize) -> Option<Line> {
        Some(Line::Code {
            file,
            hunk,
            old: Some(0),
            new: None,
        })
    }

    #[test]
    fn brackets_move_between_first_changed_lines_and_braces_between_files() {
        let mut r = review();
        press(&mut r, KeyCode::Char(']'));
        assert_eq!(r.current(), on_change(0, 0));
        press(&mut r, KeyCode::Char(']'));
        assert_eq!(r.current(), on_change(0, 1));
        press(&mut r, KeyCode::Char('}'));
        assert_eq!(r.current(), Some(Line::File(1)));
        press(&mut r, KeyCode::Char('['));
        assert_eq!(r.current(), on_change(0, 1));
        press(&mut r, KeyCode::Char('{'));
        assert_eq!(r.current(), Some(Line::File(0)));
    }

    #[test]
    fn enter_on_a_file_header_folds_the_file_and_unfolds_it_again() {
        let mut r = review();
        press(&mut r, KeyCode::Char(']'));
        // Enter on code does nothing.
        press(&mut r, KeyCode::Enter);
        assert_eq!(r.current(), on_change(0, 0));
        press(&mut r, KeyCode::Char('{'));
        press(&mut r, KeyCode::Enter);
        assert_eq!(r.current(), Some(Line::File(0)));
        press(&mut r, KeyCode::Down);
        assert_eq!(r.current(), Some(Line::File(1)));
        // Brackets skip the folded file.
        press(&mut r, KeyCode::Char('{'));
        press(&mut r, KeyCode::Char(']'));
        assert_eq!(r.current(), on_change(1, 0));
        press(&mut r, KeyCode::Char('{'));
        press(&mut r, KeyCode::Char('{'));
        press(&mut r, KeyCode::Enter);
        press(&mut r, KeyCode::Down);
        assert_eq!(r.current(), Some(Line::Hunk(0, 0)));
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
