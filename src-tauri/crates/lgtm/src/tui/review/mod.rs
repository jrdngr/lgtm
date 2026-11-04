//! Reviewing a commit or range: its diff, unified or side by side, beside its file list.

mod agent;
mod command;
mod comments;
mod draw;
mod edit;
mod list;
mod marking;
pub mod marks;
mod mode;
mod range;
mod reload;
mod resolve;
pub mod rows;
mod search;
mod select;
mod threads;
mod tree;
mod undo;
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
use crate::edit::keymap::Pick;
use draw::{CODE_X, NUM};
use rows::Line;

pub enum Outcome {
    None,
    Back,
    Quit,
    /// Go to the editor, on this file, a path in the workspace, at this line, else on the
    /// file it last had.
    Editor(Option<(String, u32)>),
    /// Go to the editor and pick from a list.
    Pick(Pick),
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
    /// Whether code rows wrap, rather than scroll sideways.
    wrap: bool,
    /// Whether the file list shows, when there's room.
    show_list: bool,
    /// Columns of code scrolled off to the left.
    left: usize,
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
    /// Whether reviewed files are set apart at the bottom of the list.
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
    /// Whether `space` was pressed, for its menu.
    space: bool,
    /// Files with edits the editor hasn't saved.
    pub unsaved: HashSet<String>,
    /// Where to put the cursor once the diff loads.
    goto: Option<reload::Spot>,
    /// Text being selected with `v`.
    selecting: Option<select::Selecting>,
    history: undo::History,
    /// A comment resolved while resolved ones are hidden, shown until the cursor leaves it.
    lingering: Option<i64>,
    /// The line held at the top of the screen after a jump, until the next key.
    snap: Option<usize>,
    /// Where the cursor was when last drawn, for which way it's moving.
    drawn_cursor: usize,
}

pub const KEYS: &[(&str, &str)] = &[
    ("j k", "line"),
    ("Ctrl-d Ctrl-u", "half page"),
    ("g G", "top, bottom"),
    ("n m", "next, previous file"),
    ("N M", "next, previous hunk"),
    ("h l", "scroll left, right"),
    ("Enter", "fold file, edit comment"),
    ("Tab", "file list"),
    ("Enter", "open, close folder (file list)"),
    ("r f", "review, flag hunk"),
    ("R F", "review, flag file or folder"),
    ("u U", "undo, redo marks and comments"),
    ("z", "hide reviewed files"),
    ("J K", "select lines"),
    ("v", "select text"),
    ("y", "copy the selection"),
    ("c", "comment for the agent, on the selected lines"),
    ("] [", "next, previous comment"),
    ("C", "resolve, reopen comment"),
    ("d", "delete comment"),
    ("space", "files, buffers, search, wrap, editor"),
    ("t", "side by side"),
    ("p", "pull PR comments"),
    ("P", "hide PR comments"),
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
            wrap: self.wrap,
            files: self.show_list,
        }
    }

    pub fn apply(&mut self, s: &Settings) {
        self.group_reviewed = s.hide_reviewed;
        self.split = s.split;
        self.hide_pr = s.hide_pr;
        self.show_resolved = s.show_resolved;
        self.wrap = s.wrap;
        self.show_list = s.files;
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
            wrap: false,
            show_list: true,
            left: 0,
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
            space: false,
            unsaved: HashSet::new(),
            goto: None,
            selecting: None,
            history: undo::History::default(),
            lingering: None,
            snap: None,
            drawn_cursor: 0,
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
        self.selecting = None;
        self.cursor = at.map_or(0, |at| at.find(&self.files, &self.lines));
        if let Some(spot) = self.goto.take() {
            self.go_to(spot);
        }
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
        let wrap = self.wrap.then(|| self.code_width());
        // Files in the list's order; reviewed ones set apart show only while their heading is
        // open.
        let order = self.list_order();
        rows::lines_with(
            &self.files,
            &order,
            &folded,
            self.split,
            &self.notes(),
            wrap,
        )
    }

    fn relayout(&mut self) {
        let spot = self.lines.get(self.cursor).map(|l| l.spot());
        self.lines = self.layout();
        self.keep_selection();
        self.cursor = match &self.selecting {
            Some(s) => s.buf.line_of(s.buf.sel.head),
            None => spot.map_or(0, |s| rows::find(&self.lines, s)),
        };
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

    /// Moves to the first line of the next or previous comment or PR thread, wrapping around.
    fn next_comment(&mut self, forward: bool) {
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
        let next = if forward {
            starts.iter().find(|&&i| i > self.cursor).or(starts.first())
        } else {
            starts
                .iter()
                .rev()
                .find(|&&i| i < self.cursor)
                .or(starts.last())
        };
        if let Some(&i) = next {
            self.cursor = i;
        }
    }

    /// Moves to the next or previous file's header.
    fn jump_file(&mut self, forward: bool) {
        self.jump(forward, |l| matches!(l, Line::File(_)));
        self.snap = Some(self.cursor);
    }

    /// Columns of code a line shows: one side's in split view.
    fn code_width(&self) -> usize {
        if self.split {
            (self.width.saturating_sub(3) / 2).saturating_sub(NUM as usize + 1)
        } else {
            self.width.saturating_sub(CODE_X as usize)
        }
        .max(4)
    }

    /// Scrolls code `n` columns sideways, no further than the widest row needs.
    fn scroll_side(&mut self, n: isize) {
        if self.wrap {
            return;
        }
        let widest = self
            .lines
            .iter()
            .filter_map(|l| match *l {
                Line::Code {
                    file,
                    hunk,
                    old,
                    new,
                    ..
                } => Some((file, hunk, [old, new])),
                _ => None,
            })
            .flat_map(|(f, h, rows)| {
                let hunk = &self.files[f].hunks[h];
                rows.into_iter()
                    .flatten()
                    .map(move |r| hunk.rows[r].text.chars().map(wrap::cols).sum::<usize>())
            })
            .max()
            .unwrap_or(0);
        let most = widest.saturating_sub(self.code_width()) as isize;
        self.left = (self.left as isize + n).clamp(0, most.max(0)) as usize;
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
                    ..
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

    /// Moves the cursor to the next or previous hunk's start, or folded file's header.
    fn jump_hunk(&mut self, forward: bool) {
        let mut starts = self.hunk_starts();
        starts.extend((0..self.lines.len()).filter(|&i| match self.lines[i] {
            Line::File(f) => self.folded.contains(&self.files[f].path),
            _ => false,
        }));
        starts.sort_unstable();
        let found = if forward {
            starts.into_iter().find(|&i| i > self.cursor)
        } else {
            starts.into_iter().rev().find(|&i| i < self.cursor)
        };
        if let Some(i) = found {
            self.cursor = i;
            self.snap = Some(self.hunk_top(i));
        }
    }

    /// The line to put at the top of the screen for the hunk on line `i`: the one above its
    /// first, which the pinned file header covers, so the hunk shows from its start.
    fn hunk_top(&self, i: usize) -> usize {
        let line = self.lines[i];
        let first = match line {
            Line::Code { file, hunk, .. } => self.lines.iter().position(|l| {
                l.file() == file
                    && l.hunk() == Some(hunk)
                    && matches!(l, Line::Hunk(..) | Line::Code { .. })
            }),
            _ => None,
        };
        first.map_or(i, |f| f.saturating_sub(1))
    }

    pub(super) fn toggle_file(&mut self, file: usize) {
        let path = self.files[file].path.clone();
        if !self.folded.remove(&path) {
            self.folded.insert(path);
        }
        self.lines = self.layout();
        self.selecting = None;
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

    pub fn key(&mut self, key: KeyEvent, store: Option<&mut Store>) -> Outcome {
        let out = self.handle_key(key, store);
        self.let_go();
        out
    }

    fn handle_key(&mut self, key: KeyEvent, mut store: Option<&mut Store>) -> Outcome {
        let half = (self.height / 2).max(1) as isize;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        self.notice = None;
        self.pinned = false;
        self.snap = None;
        if self.cmd.is_some() || self.confirm.is_some() {
            return self.command_key(key, store);
        }
        if self.typing() {
            self.input_key(key, store.as_deref());
            return Outcome::None;
        }
        if self.selecting.is_some() {
            self.select_key(key);
            return Outcome::None;
        }
        if std::mem::take(&mut self.space) {
            return match key.code {
                KeyCode::Char('f') => Outcome::Pick(Pick::Files),
                KeyCode::Char('o') => Outcome::Pick(Pick::OpenFiles),
                KeyCode::Char('b') => {
                    self.show_list = !self.show_list;
                    self.set_in_list(false);
                    Outcome::None
                }
                KeyCode::Char('/') => Outcome::Pick(Pick::Grep),
                KeyCode::Char('r') => Outcome::Editor(self.edit_target()),
                KeyCode::Char('w') => {
                    self.wrap = !self.wrap;
                    self.left = 0;
                    self.relayout();
                    Outcome::None
                }
                _ => Outcome::None,
            };
        }
        if key.code == KeyCode::Char(' ') && !self.in_list {
            self.space = true;
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
            KeyCode::Char(c @ ('r' | 'F')) if self.in_list => self.list_mark(c, store),
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
            KeyCode::Char('n') => self.jump_file(true),
            KeyCode::Char('m') => self.jump_file(false),
            KeyCode::Char('N') => self.jump_hunk(true),
            KeyCode::Char('M') => self.jump_hunk(false),
            KeyCode::Char('h') | KeyCode::Left => self.scroll_side(-4),
            KeyCode::Char('l') | KeyCode::Right => self.scroll_side(4),
            KeyCode::Char('u') => self.undo(store),
            KeyCode::Char('U') => self.redo(store),
            KeyCode::Char('v') if !self.in_list => self.start_select(),
            KeyCode::Char('/') if !self.in_list => self.start_search(""),
            KeyCode::Char('z') => self.toggle_group(),
            KeyCode::Enter => match self.current() {
                Some(Line::File(f)) => self.toggle_file(f),
                Some(Line::Note {
                    key: rows::NoteKey::Comment(n),
                    ..
                }) => self.edit_comment(n),
                _ => {}
            },
            KeyCode::Char('c') if !ctrl => self.start_comment(store.as_deref()),
            KeyCode::Char(']') => self.next_comment(true),
            KeyCode::Char('[') => self.next_comment(false),
            KeyCode::Char('C') => self.toggle_resolved(store.as_deref()),
            KeyCode::Char('d') if !ctrl => self.delete_comment(store.as_deref()),
            KeyCode::Char('p') => return Outcome::Pull,
            KeyCode::Char('P') => self.toggle_pr(),
            KeyCode::Char('t') => {
                self.split = !self.split;
                self.relayout();
            }
            _ => {}
        }
        Outcome::None
    }

    /// A click on a file in the list goes to it, a click on a file header's checkbox checks
    /// the file, a click elsewhere on the header folds or unfolds it, a click elsewhere in the diff moves the cursor there, a drag selects lines, and the
    /// wheel scrolls.
    pub fn mouse(&mut self, ev: MouseEvent, store: Option<&mut Store>) {
        let at = Position::new(ev.column, ev.row);
        self.pinned = true;
        self.snap = None;
        if matches!(ev.kind, MouseEventKind::Down(_)) {
            self.anchor = None;
            self.selecting = None;
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
                let on_check = draw::CHECK_X.contains(&(ev.column - self.diff_area.x));
                match line {
                    Some(&Line::File(f)) if on_check => self.click_check(f, store),
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
        self.let_go();
    }

    fn set_in_list(&mut self, on: bool) {
        self.in_list = on;
        if !on {
            self.list_pick = None;
        }
    }

    /// Scrolls the diff `n` lines, the cursor moving with it to stay where it is on screen.
    /// Once the view can't scroll further the cursor keeps going, to the first or last line.
    fn scroll(&mut self, n: isize) {
        let max_top = self.lines.len().saturating_sub(self.height);
        self.top = (self.top as isize + n).clamp(0, max_top as isize) as usize;
        let last = self.lines.len().saturating_sub(1);
        self.cursor = (self.cursor as isize + n).clamp(0, last as isize) as usize;
    }

    /// The key bar. With Shift held, r and f become R and F.
    pub fn hints(&self, shift: bool) -> Vec<Hint<'static>> {
        if self.cmd.as_ref().is_some_and(|c| c.search) {
            return vec![("Enter", "search", false), ("Esc", "cancel", false)];
        }
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
        if self.selecting.is_some() {
            return vec![
                ("y", "copy", false),
                ("/", "search", false),
                ("n N", "match", false),
                ("Esc", "cancel", false),
            ];
        }
        let (review, flag) = if shift {
            (("R", "review hunk", false), ("F", "flag file", false))
        } else {
            (("r", "review file", false), ("f", "flag hunk", false))
        };
        if let Some(c) = self.comment_at_cursor().and_then(|n| self.comment(n)) {
            let resolve = if c.done { "reopen" } else { "resolve" };
            let mut hints = vec![("Enter", "edit", false)];
            hints.truncate(usize::from(!c.done));
            hints.extend([
                ("d", "delete", false),
                ("C", resolve, false),
                ("u U", "undo", false),
                ("] [", "comment", false),
                ("?", "keys", false),
                ("q", "back", false),
            ]);
            return hints;
        }
        let mut hints = vec![
            ("space", "menu", false),
            ("n m", "file", false),
            ("N M", "hunk", false),
            review,
            flag,
            ("c", "comment", false),
            ("t", "split", self.split),
            ("u U", "undo", false),
            ("v", "select", false),
            ("z", "hide reviewed", self.group_reviewed),
        ];
        if self.pr.is_some() {
            hints.push(("P", "hide PR", self.hide_pr));
        }
        hints.extend([("?", "keys", false), ("q", "back", false)]);
        hints
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lgtm_core::diff::{FileStatus, Hunk, Row, RowKind};

    pub(super) fn file(path: &str, hunks: usize) -> FileDiff {
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

    pub(super) fn press(r: &mut Review, code: KeyCode) {
        r.key(KeyEvent::new(code, KeyModifiers::NONE), None);
    }

    pub(super) fn review() -> Review {
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
            part: 0,
        })
    }

    #[test]
    fn a_click_on_a_headers_checkbox_checks_the_file_and_one_on_its_path_only_folds() {
        let mut store = Store::in_memory();
        let mut r = Review::new(1, Target::Revset("@".into()));
        r.loaded(Ok((vec![], vec![file("a.rs", 2)])));
        r.marks = Some(super::marks::tests::marks());
        r.diff_area = Rect::new(30, 1, 80, 20);
        let mut click = |r: &mut Review, column| {
            let ev = MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column,
                row: 1,
                modifiers: KeyModifiers::NONE,
            };
            r.mouse(ev, Some(&mut store));
        };
        let folded = |r: &Review| r.folded.contains("a.rs");
        click(&mut r, 32);
        assert_eq!((r.file_check(0), folded(&r)), (Some(false), true));
        click(&mut r, 40);
        assert_eq!((r.file_check(0), folded(&r)), (Some(false), false));
        click(&mut r, 31);
        assert_eq!((r.file_check(0), folded(&r)), (None, false));
    }

    #[test]
    fn space_b_hides_and_shows_the_file_list_and_space_o_lists_open_files() {
        let mut r = review();
        let space = |r: &mut Review, c| {
            press(r, KeyCode::Char(' '));
            r.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE), None)
        };
        assert!(matches!(space(&mut r, 'b'), Outcome::None));
        assert!(!r.settings().files);
        space(&mut r, 'b');
        assert!(r.settings().files);
        assert!(matches!(space(&mut r, 'o'), Outcome::Pick(Pick::OpenFiles)));
    }

    #[test]
    fn the_wheel_keeps_moving_the_cursor_once_the_view_stops_at_the_top_or_bottom() {
        let mut r = review();
        r.height = 3;
        let last = r.lines.len() - 1;
        r.scroll(3);
        assert_eq!((r.top, r.cursor), (3, 3));
        r.scroll(-1);
        assert_eq!((r.top, r.cursor), (2, 2));
        r.scroll(-3);
        assert_eq!((r.top, r.cursor), (0, 0));
        r.scroll(100);
        assert_eq!((r.top, r.cursor), (last + 1 - 3, last));
    }

    #[test]
    fn shifted_n_m_move_between_first_changed_lines_and_n_m_between_files() {
        let mut r = review();
        press(&mut r, KeyCode::Char('N'));
        assert_eq!(r.current(), on_change(0, 0));
        press(&mut r, KeyCode::Char('N'));
        assert_eq!(r.current(), on_change(0, 1));
        press(&mut r, KeyCode::Char('n'));
        assert_eq!(r.current(), Some(Line::File(1)));
        press(&mut r, KeyCode::Char('M'));
        assert_eq!(r.current(), on_change(0, 1));
        press(&mut r, KeyCode::Char('m'));
        assert_eq!(r.current(), Some(Line::File(0)));
    }

    #[test]
    fn enter_on_a_file_header_folds_the_file_and_unfolds_it_again() {
        let mut r = review();
        press(&mut r, KeyCode::Char('N'));
        // Enter on code does nothing.
        press(&mut r, KeyCode::Enter);
        assert_eq!(r.current(), on_change(0, 0));
        press(&mut r, KeyCode::Char('m'));
        press(&mut r, KeyCode::Enter);
        assert_eq!(r.current(), Some(Line::File(0)));
        press(&mut r, KeyCode::Down);
        assert_eq!(r.current(), Some(Line::File(1)));
        press(&mut r, KeyCode::Char('m'));
        press(&mut r, KeyCode::Char('m'));
        press(&mut r, KeyCode::Enter);
        press(&mut r, KeyCode::Down);
        assert_eq!(r.current(), Some(Line::Hunk(0, 0)));
    }

    #[test]
    fn hunk_keys_stop_on_a_folded_files_header() {
        let mut r = review();
        r.set_folded(0, true);
        press(&mut r, KeyCode::Char('N'));
        assert_eq!(r.current(), on_change(1, 0));
        press(&mut r, KeyCode::Char('M'));
        assert_eq!(r.current(), Some(Line::File(0)));
    }

    #[test]
    fn jumps_put_the_hunk_or_file_at_the_top_under_the_pinned_header() {
        let mut r = review();
        press(&mut r, KeyCode::Char('N'));
        press(&mut r, KeyCode::Char('N'));
        // a.rs: header, divider, 2 rows, divider, 2 rows. The second divider shows first.
        assert_eq!(r.snap, Some(3));
        press(&mut r, KeyCode::Char('n'));
        assert_eq!(r.snap, Some(r.cursor));
        press(&mut r, KeyCode::Char('j'));
        assert_eq!(r.snap, None);
        // Picking a file in the list puts it at the top too.
        r.list_step(-1);
        assert_eq!(r.current(), Some(Line::File(0)));
        assert_eq!(r.snap, Some(r.cursor));
    }

    #[test]
    fn the_diff_shows_files_in_the_file_lists_order() {
        let mut r = Review::new(1, Target::Revset("@".into()));
        r.loaded(Ok((vec![], vec![file("z.rs", 1), file("a.rs", 1)])));
        let headers: Vec<&str> = r
            .lines
            .iter()
            .filter_map(|l| match l {
                Line::File(f) => Some(r.files[*f].path.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(headers, ["a.rs", "z.rs"]);
    }

    #[test]
    fn h_l_scroll_sideways_no_further_than_the_widest_row_needs() {
        let mut r = review();
        // Ten columns wider than the 67 an 80-column unified view shows.
        r.width = 80;
        r.files[0].hunks[0].rows[0].text = "x".repeat(77);
        press(&mut r, KeyCode::Char('l'));
        press(&mut r, KeyCode::Char('l'));
        assert_eq!(r.left, 8);
        press(&mut r, KeyCode::Char('l'));
        assert_eq!(r.left, 10);
        press(&mut r, KeyCode::Char('h'));
        assert_eq!(r.left, 6);
    }

    #[test]
    fn the_file_list_moves_a_file_at_a_time() {
        let mut r = review();
        press(&mut r, KeyCode::Char('N'));
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
