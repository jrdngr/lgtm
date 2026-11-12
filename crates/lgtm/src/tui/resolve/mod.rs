//! Resolving a file's conflicts in the editor: numbers pick sides in the order pressed, `J`
//! and `K` move a kept side, `i` edits one, and `:w` writes what's resolved.

mod draw;
mod layout;

use crate::tui::help::Group;
use std::path::Path;

use lgtm_core::resolve::{Fate, FileChoice, ResolveView, Session};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::edit::buffer::{Buffer, Sel};
use crate::edit::keys::{Action, Keys, Mode, View};
use layout::{Row, Slot};

/// The keys, for `?`.
pub fn help_groups() -> Vec<Group> {
    vec![
        Group::new(
            "Pick",
            &[
                ("1 2 3", "keep a side, in order; again drops it"),
                ("0", "keep the base"),
                ("J K", "move the side down, up"),
                ("i", "edit the side; Esc twice to finish"),
                ("o", "edited, original"),
                ("u U", "undo, redo"),
            ],
        ),
        Group::new(
            "Move",
            &[("j k", "line"), ("n N", "next, previous conflict")],
        ),
        Group::new(
            "Files",
            &[
                ("F D", "keep, delete a deleted file"),
                ("c b x", "moved file: carry, keep both, drop"),
            ],
        ),
        Group::new(
            "Commands",
            &[
                (":w", "save what's resolved"),
                (":q!", "leave, dropping it"),
                (":raw", "edit the markers as text"),
            ],
        ),
    ]
}

/// A side being edited: its conflict, its place, and its text.
struct Editing {
    conflict: usize,
    pick: usize,
    buf: Buffer,
}

pub enum Outcome {
    None,
    /// Open the `:` line.
    Command,
    /// Something to show.
    Notice(String),
    Copy(String),
}

pub struct Resolver {
    pub session: Session,
    view: ResolveView,
    rows: Vec<Row>,
    cursor: usize,
    top: usize,
    /// Rows the text had when last drawn.
    height: usize,
    keys: Keys,
    editing: Option<Editing>,
    /// Highlight segments per line of each text drawn.
    colors: std::collections::HashMap<String, Vec<Vec<lgtm_core::syntax::Segment>>>,
}

impl Resolver {
    pub fn new(path: &str, text: &str) -> Self {
        let session = Session::new(path, text);
        let view = session.view();
        let rows = layout::rows(&view);
        let mut r = Self {
            session,
            view,
            rows,
            cursor: 0,
            top: 0,
            height: 20,
            keys: Keys::default(),
            editing: None,
            colors: Default::default(),
        };
        r.cursor = r.heads().first().copied().unwrap_or(0);
        r
    }

    /// Takes the labels and fate jj found for the same text.
    pub fn adopt(&mut self, filled: &Session) {
        self.session.adopt(filled);
        self.refresh();
    }

    /// Whether keys are going into a side's text.
    pub fn typing(&self) -> bool {
        self.editing.is_some() && (self.keys.mode == Mode::Insert || self.keys.waiting())
    }

    pub fn mode(&self) -> Option<Mode> {
        self.editing.as_ref().map(|_| self.keys.mode)
    }

    /// Lays the rows out again, the cursor on the same side's line, else its conflict.
    fn refresh(&mut self) {
        let at = self.rows.get(self.cursor).cloned();
        self.view = self.session.view();
        self.rows = layout::rows(&self.view);
        let Some(at) = at else {
            self.cursor = 0;
            return;
        };
        let same = |r: &Row| match (r, &at) {
            (
                Row::Code {
                    key, n, conflict, ..
                },
                Row::Code {
                    key: k,
                    n: m,
                    conflict: c,
                    ..
                },
            ) => key == k && n == m && conflict == c,
            (
                Row::Side { key, conflict, .. },
                Row::Side {
                    key: k,
                    conflict: c,
                    ..
                },
            ) => key == k && conflict == c,
            (Row::Text { line, .. }, Row::Text { line: l, .. }) => line == l,
            _ => *r == at,
        };
        let head = |r: &Row| {
            at.conflict().is_some()
                && r == &Row::Head {
                    conflict: at.conflict().unwrap_or(0),
                }
        };
        self.cursor = self
            .rows
            .iter()
            .position(same)
            .or_else(|| self.rows.iter().position(head))
            .unwrap_or_else(|| self.cursor.min(self.rows.len().saturating_sub(1)));
    }

    fn heads(&self) -> Vec<usize> {
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, r)| matches!(r, Row::Head { .. }))
            .map(|(i, _)| i)
            .collect()
    }

    /// The conflict number keys act on: the cursor's, else the next one down, else the last.
    fn target(&self) -> Option<usize> {
        self.rows[self.cursor..]
            .iter()
            .chain(self.rows[..self.cursor].iter().rev())
            .find_map(Row::conflict)
    }

    /// The kept side under the cursor: its conflict and place.
    fn pick_here(&self) -> Option<(usize, usize)> {
        match self.rows.get(self.cursor)?.side()? {
            (c, _, Slot::Pick(p)) => Some((c, p)),
            _ => None,
        }
    }

    fn step(&mut self, by: isize) {
        let last = self.rows.len().saturating_sub(1) as isize;
        self.cursor = (self.cursor as isize + by).clamp(0, last) as usize;
    }

    /// Moves to the next or previous conflict's head, around the ends.
    fn jump(&mut self, forward: bool) {
        let heads = self.heads();
        let next = if forward {
            heads.iter().find(|&&h| h > self.cursor).or(heads.first())
        } else {
            heads
                .iter()
                .rev()
                .find(|&&h| h < self.cursor)
                .or(heads.last())
        };
        if let Some(&h) = next {
            self.cursor = h;
        }
    }

    fn act(&mut self, changed: bool) {
        if changed {
            self.refresh();
        }
    }

    pub fn key(&mut self, key: KeyEvent) -> Outcome {
        if self.editing.is_some() {
            return self.edit_key(key);
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let page = (self.height / 2).max(1) as isize;
        let choose = |s: &mut Self, choice| {
            let changed = s.session.choose(choice);
            s.act(changed);
        };
        match key.code {
            KeyCode::Char(':') => return Outcome::Command,
            KeyCode::Char('j') | KeyCode::Down => self.step(1),
            KeyCode::Char('k') | KeyCode::Up => self.step(-1),
            KeyCode::Char('d') if ctrl => self.step(page),
            KeyCode::Char('u') if ctrl => self.step(-page),
            KeyCode::PageDown => self.step(page),
            KeyCode::PageUp => self.step(-page),
            KeyCode::Char('g') | KeyCode::Home => self.cursor = 0,
            KeyCode::Char('G') | KeyCode::End => self.step(isize::MAX / 2),
            KeyCode::Char('n') => self.jump(true),
            KeyCode::Char('N') => self.jump(false),
            KeyCode::Char(d @ '0'..='9') => {
                let Some(c) = self.target() else {
                    return Outcome::None;
                };
                let key = d as u8 - b'0';
                if !self.session.pickable(c, key) {
                    return Outcome::Notice(format!("no side {key}"));
                }
                let inside = self.rows[self.cursor].conflict() == Some(c);
                self.session.toggle(c, key);
                self.refresh();
                if !inside {
                    self.cursor = self
                        .rows
                        .iter()
                        .position(|r| *r == Row::Head { conflict: c })
                        .unwrap_or(0);
                }
            }
            KeyCode::Char(k @ ('J' | 'K')) => {
                if let Some((c, p)) = self.pick_here() {
                    let to = if k == 'J' { p + 1 } else { p.wrapping_sub(1) };
                    if to != usize::MAX && self.session.place(c, p, to) {
                        self.refresh_on_pick(c, to);
                    }
                }
            }
            KeyCode::Char('i') => self.start_edit(),
            KeyCode::Char('o') => {
                if let Some((c, p)) = self.pick_here() {
                    let changed = self.session.flip_original(c, p);
                    self.act(changed);
                }
            }
            KeyCode::Char('u') => {
                let changed = self.session.undo();
                self.act(changed);
            }
            KeyCode::Char('U') => {
                let changed = self.session.redo();
                self.act(changed);
            }
            KeyCode::Char('F') => choose(self, FileChoice::Keep),
            KeyCode::Char('D') => choose(self, FileChoice::Delete),
            KeyCode::Char('c') => choose(self, FileChoice::Carry),
            KeyCode::Char('b') => choose(self, FileChoice::Both),
            KeyCode::Char('x') => choose(self, FileChoice::Drop),
            _ => {}
        }
        Outcome::None
    }

    /// Lays out again with the cursor on the label of pick `p` of conflict `c`.
    fn refresh_on_pick(&mut self, c: usize, p: usize) {
        self.refresh();
        let label = |r: &Row| matches!(r, Row::Side { conflict, slot: Slot::Pick(q), .. } if *conflict == c && *q == p);
        if let Some(i) = self.rows.iter().position(label) {
            self.cursor = i;
        }
    }

    /// Edits the kept side under the cursor, from the line the cursor is on.
    fn start_edit(&mut self) {
        let Some((c, p)) = self.pick_here() else {
            return;
        };
        let n = match self.rows[self.cursor] {
            Row::Code { n, .. } => n,
            _ => 0,
        };
        let pick = &self.session.picks(c)[p];
        let text = self.session.pick_text(c, pick).to_string();
        let mut buf = Buffer::with_text(Path::new(&self.session.path), &self.session.path, &text);
        let line = n.min(buf.last_line());
        buf.sel = Sel::point(buf.line_start(line));
        self.keys = Keys::default();
        self.keys.mode = Mode::Insert;
        self.editing = Some(Editing {
            conflict: c,
            pick: p,
            buf,
        });
    }

    /// Ends the edit, keeping its text as one undo step.
    fn finish_edit(&mut self) {
        let Some(e) = self.editing.take() else {
            return;
        };
        self.keys.mode = Mode::Normal;
        let text = e.buf.text.to_string();
        let changed = self.session.edit(e.conflict, e.pick, &text);
        self.act(changed);
    }

    fn edit_key(&mut self, key: KeyEvent) -> Outcome {
        if key.code == KeyCode::Esc && self.keys.mode == Mode::Normal && !self.keys.waiting() {
            self.finish_edit();
            return Outcome::None;
        }
        let Some(e) = self.editing.as_mut() else {
            return Outcome::None;
        };
        let view = View {
            height: self.height,
            hidden: &[],
        };
        match self.keys.press(&mut e.buf, key, view) {
            Action::Command => {
                self.finish_edit();
                Outcome::Command
            }
            Action::Copy(text) => Outcome::Copy(text),
            Action::Paste => match crate::tui::edit::clipboard::paste() {
                Ok(text) => {
                    self.keys.paste_text(&mut e.buf, &text);
                    Outcome::None
                }
                Err(err) => Outcome::Notice(format!("couldn't paste: {err}")),
            },
            Action::Notice(n) => Outcome::Notice(n),
            _ => Outcome::None,
        }
    }

    /// Takes text the terminal pasted into the side being edited.
    pub fn paste(&mut self, text: &str) {
        if let Some(e) = self.editing.as_mut() {
            self.keys.put_text(&mut e.buf, &text.replace("\r\n", "\n"));
        }
    }

    /// Ends any edit, then writes the file into `root`.
    pub fn save(&mut self, root: &Path) -> std::io::Result<lgtm_core::resolve::Saved> {
        self.finish_edit();
        let saved = self.session.save(root)?;
        self.refresh();
        Ok(saved)
    }

    /// Whether there's something saving would write.
    pub fn dirty(&self) -> bool {
        self.session.dirty() || self.editing.is_some()
    }

    /// The file bar's choices for the file's fate: key, label, choice.
    fn choices(&self) -> &'static [(&'static str, &'static str, FileChoice)] {
        match self.view.fate {
            Fate::Here => &[],
            Fate::Deleted { .. } => &[
                ("F", "Keep file", FileChoice::Keep),
                ("D", "Delete file", FileChoice::Delete),
            ],
            Fate::Moved { .. } => &[
                ("c", "Carry edits", FileChoice::Carry),
                ("b", "Keep both", FileChoice::Both),
                ("x", "Drop edits", FileChoice::Drop),
            ],
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
