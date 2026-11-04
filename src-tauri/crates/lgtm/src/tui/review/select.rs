//! `v` selects text in the diff with the editor's motions, and `y` copies it. The diff's
//! lines become a read-only buffer, a line per screen line, so the buffer's lines are the
//! review's: code rows hold their text, or a wrapped row each part of it, and every other line
//! is empty.

use std::path::Path;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Position;

use super::Review;
use super::draw::{CODE_X, NUM};
use super::rows::Line;
use super::wrap::{code_parts, cols};
use crate::edit::buffer::{Buffer, Sel};
use crate::edit::keymap::{self, Cmd};
use crate::edit::keys::{Action, Keys, Mode, View};
use crate::edit::motion;
use crate::tui::edit::clipboard;

pub struct Selecting {
    pub buf: Buffer,
    pub(super) keys: Keys,
}

/// The keys while selecting: the review's own, then the editor's that move or select.
pub fn select_keys() -> Vec<(&'static str, &'static str)> {
    let own = [
        ("Home End", "line start, end"),
        ("click", "move the cursor"),
        ("y", "copy the selection"),
        ("v Esc", "stop selecting"),
    ];
    let editor = keymap::BINDS.iter().filter(|b| {
        !b.cmd.edits()
            && !matches!(b.keys, "left" | "right" | "up" | "down")
            && !matches!(
                b.cmd,
                Cmd::Pick(_)
                    | Cmd::Review
                    | Cmd::Sidebar
                    | Cmd::ToggleSidebar
                    | Cmd::Split { .. }
                    | Cmd::Command
                    | Cmd::Quit
                    | Cmd::Fold(_)
                    | Cmd::ToggleSelect
                    | Cmd::Escape
                    | Cmd::Yank
            )
    });
    own.into_iter()
        .chain(editor.map(|b| (b.keys, b.label)))
        .collect()
}

impl Review {
    /// The keys `?` lists: the selecting ones while selecting.
    pub fn help_keys(&self) -> Vec<(&'static str, &'static str)> {
        if self.selecting.is_some() {
            select_keys()
        } else {
            super::KEYS.to_vec()
        }
    }

    /// The text of each screen line: a code row's, the new side's in split view, or the part
    /// of it the line shows when rows wrap.
    fn line_texts(&self) -> String {
        let width = self.code_width();
        let texts: Vec<String> = self
            .lines
            .iter()
            .map(|&line| match line {
                Line::Code {
                    file,
                    hunk,
                    old,
                    new,
                    part,
                } => {
                    let rows = &self.files[file].hunks[hunk].rows;
                    let at = if self.split { new.or(old) } else { old };
                    let text = at.map_or("", |i| rows[i].text.as_str());
                    if !self.wrap {
                        return text.to_string();
                    }
                    let span = code_parts(text, width).get(part).cloned().unwrap_or(0..0);
                    text.chars().skip(span.start).take(span.len()).collect()
                }
                _ => String::new(),
            })
            .collect();
        texts.join("\n")
    }

    /// The selected text. Lines a wrapped row continues on join without a newline.
    pub(super) fn selected_text(&self, b: &Buffer) -> String {
        let (from, to) = (b.sel.from(), b.sel.to() + 1);
        let mut out = String::new();
        for (i, ch) in b.text.chars().enumerate().skip(from).take(to - from) {
            let next = b.line_of(i) + 1;
            let wraps = matches!(self.lines.get(next), Some(Line::Code { part, .. }) if *part > 0);
            if !(ch == '\n' && wraps) {
                out.push(ch);
            }
        }
        out
    }

    /// Starts selecting at the cursor line's first non-blank char.
    pub(super) fn start_select(&mut self) {
        let name = self
            .current()
            .map_or("", |l| self.files[l.file()].path.as_str());
        let mut buf = Buffer::with_text(Path::new(name), name, &self.line_texts());
        let at = motion::first_non_blank(&buf, self.cursor.min(buf.last_line()));
        buf.sel = Sel::point(at);
        self.selecting = Some(Selecting {
            buf,
            keys: Keys::read_only_select(),
        });
        self.follow_head();
    }

    /// Keeps the selection while the lines read the same, else ends it.
    pub(super) fn keep_selection(&mut self) {
        if self
            .selecting
            .as_ref()
            .is_some_and(|s| s.buf.text != self.line_texts().as_str())
        {
            self.selecting = None;
        }
    }

    /// Moves the cursor to the selection's head, scrolling sideways to show it.
    pub(super) fn follow_head(&mut self) {
        let Some(b) = self.selecting.as_ref().map(|s| &s.buf) else {
            return;
        };
        let line = b.line_of(b.sel.head);
        let start = b.line_start(line);
        let col: usize = b
            .text
            .chars_at(start)
            .take(b.sel.head - start)
            .map(cols)
            .sum();
        self.cursor = line;
        if self.wrap {
            return;
        }
        let width = self.code_width();
        if col < self.left {
            self.left = col;
        } else if col >= self.left + width {
            self.left = col + 1 - width;
        }
    }

    /// Puts the selection's cursor, selecting nothing, on the char under a click at `at` while
    /// selecting. Returns whether the click was on code then.
    pub(super) fn click_select(&mut self, at: Position) -> bool {
        let i = self.top + (at.y - self.diff_area.y) as usize;
        let Some(&Line::Code { new, .. }) = self.lines.get(i) else {
            return false;
        };
        let x0 = self.diff_area.x;
        let start = if !self.split {
            x0 + CODE_X
        } else {
            let half = self.diff_area.width.saturating_sub(3) / 2;
            let side = u16::from(new.is_some());
            x0 + 2 + side * (half + 1) + NUM + 1
        };
        let left = if self.wrap { 0 } else { self.left };
        let want = left + at.x.saturating_sub(start) as usize;
        let Some(s) = self.selecting.as_mut() else {
            return false;
        };
        let b = &mut s.buf;
        let (from, end) = (b.line_start(i), b.line_end(i));
        let mut col = 0;
        let mut pos = from;
        for ch in b.text.chars_at(from).take(end - from) {
            col += cols(ch);
            if col > want {
                break;
            }
            pos += 1;
        }
        b.sel = Sel::point(pos.min(end.saturating_sub(1).max(from)));
        self.follow_head();
        true
    }

    /// Moves the selection's cursor to its line's start or end, selecting nothing.
    fn line_edge(&mut self, end: bool) {
        let Some(s) = self.selecting.as_mut() else {
            return;
        };
        let b = &mut s.buf;
        let line = b.line_of(b.sel.head);
        let (from, to) = (b.line_start(line), b.line_end(line));
        b.sel = Sel::point(if end {
            to.saturating_sub(1).max(from)
        } else {
            from
        });
        self.follow_head();
    }

    /// Handles a key while selecting: motions move the selection, Home and End go to the
    /// line's ends, `y` copies it, `/` searches from it, and `v` or Esc stop.
    pub(super) fn select_key(&mut self, key: KeyEvent) {
        let plain = !key.modifiers.contains(KeyModifiers::CONTROL);
        let waiting = self.selecting.as_ref().is_some_and(|s| s.keys.waiting());
        if !waiting && matches!(key.code, KeyCode::Home | KeyCode::End) {
            self.line_edge(key.code == KeyCode::End);
            return;
        }
        if plain
            && !waiting
            && let KeyCode::Char(c @ ('n' | 'N')) = key.code
        {
            self.next_match(c == 'n');
            return;
        }
        let Some(s) = self.selecting.as_mut() else {
            return;
        };
        let view = View {
            height: self.height,
            hidden: &[],
        };
        let action = if key.code == KeyCode::Char('y') && plain && !s.keys.waiting() {
            Action::Copy(String::new())
        } else {
            s.keys.press(&mut s.buf, key, view)
        };
        let selecting = s.keys.mode == Mode::Select;
        self.follow_head();
        match action {
            Action::Copy(_) => {
                let b = &self.selecting.as_ref().expect("checked above").buf;
                if let Err(e) = clipboard::copy(&self.selected_text(b)) {
                    self.notice = Some(format!("copy: {e}"));
                }
                self.selecting = None;
            }
            Action::Notice(n) => self.notice = Some(n),
            Action::Search => self.start_search(&self.selection_needle()),
            _ if !selecting => self.selecting = None,
            _ => {}
        }
    }

    /// The selected chars on screen line `n`, as a range of its chars, and the cursor's char
    /// when it's on the line.
    pub(super) fn selected_chars(
        &self,
        n: usize,
    ) -> Option<(std::ops::Range<usize>, Option<usize>)> {
        let b = &self.selecting.as_ref()?.buf;
        if n > b.last_line() {
            return None;
        }
        let start = b.line_start(n);
        let len = b.line_end(n) - start;
        let (from, to) = (b.sel.from(), b.sel.to() + 1);
        let range = from.clamp(start, start + len) - start..to.clamp(start, start + len) - start;
        let head = (b.line_of(b.sel.head) == n).then(|| b.sel.head - start);
        (!range.is_empty() || head.is_some()).then_some((range, head))
    }
}

#[cfg(test)]
mod tests {
    use lgtm_core::diff::{Hunk, Row, RowKind};

    use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

    use super::super::tests::{file, press};
    use super::*;
    use crate::tui::load::Target;

    fn review() -> Review {
        let row = |kind, text: &str| Row {
            kind,
            old_line: Some(1),
            new_line: Some(1),
            text: text.into(),
        };
        let mut f = file("a.rs", 0);
        f.hunks.push(Hunk {
            old_start: 1,
            old_lines: 1,
            new_start: 1,
            new_lines: 1,
            section: String::new(),
            rows: vec![
                row(RowKind::Removed, "  old a0"),
                row(RowKind::Added, "  new a0"),
            ],
        });
        let mut r = Review::new(1, Target::Revset("@".into()));
        r.loaded(Ok((vec![], vec![f])));
        r
    }

    #[test]
    fn v_selects_with_the_editors_motions_and_esc_stops() {
        let mut r = review();
        r.cursor = 1;
        press(&mut r, KeyCode::Char('v'));
        press(&mut r, KeyCode::Char('j'));
        press(&mut r, KeyCode::Char('g'));
        press(&mut r, KeyCode::Char('l'));
        let b = &r.selecting.as_ref().unwrap().buf;
        assert_eq!(b.slice(b.sel.from(), b.sel.to() + 1), "old a0\n  new a0");
        assert_eq!(r.cursor, 2);
        assert_eq!(r.selected_chars(1), Some((2..8, None)));
        assert_eq!(r.selected_chars(2), Some((0..8, Some(7))));
        // Keys that would change the text do nothing.
        press(&mut r, KeyCode::Char('d'));
        assert_eq!(r.lines.len(), 3);
        assert!(r.selecting.is_some());
        press(&mut r, KeyCode::Esc);
        assert!(r.selecting.is_none());
    }

    #[test]
    fn home_end_and_clicks_move_the_cursor_without_selecting_and_semicolon_collapses() {
        let mut r = review();
        r.diff_area = ratatui::layout::Rect::new(0, 0, 80, 10);
        r.cursor = 1;
        press(&mut r, KeyCode::Char('v'));
        press(&mut r, KeyCode::End);
        let point = |r: &Review| {
            let b = &r.selecting.as_ref().unwrap().buf;
            (b.sel.anchor == b.sel.head).then_some(b.sel.head - b.line_start(b.line_of(b.sel.head)))
        };
        assert_eq!(point(&r), Some(7));
        press(&mut r, KeyCode::Home);
        assert_eq!(point(&r), Some(0));
        press(&mut r, KeyCode::Char('w'));
        assert_eq!(point(&r), None);
        press(&mut r, KeyCode::Char(';'));
        assert!(point(&r).is_some());
        // A click on the second row's "new" lands there, still selecting.
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: CODE_X + 3,
            row: 2,
            modifiers: KeyModifiers::NONE,
        };
        r.mouse(click, None);
        assert_eq!(r.cursor, 2);
        assert_eq!(point(&r), Some(3));
        assert_eq!(r.help_keys()[0].0, "Home End");
    }

    #[test]
    fn space_w_wraps_rows_and_a_copy_joins_a_wrapped_row_back_up() {
        let mut r = review();
        r.width = CODE_X as usize + 5;
        press(&mut r, KeyCode::Char(' '));
        press(&mut r, KeyCode::Char('w'));
        // "  old a0" wraps after its spaces: "  ", "old ", "a0".
        assert_eq!(r.lines.len(), 1 + 3 + 3);
        r.cursor = 1;
        press(&mut r, KeyCode::Char('v'));
        press(&mut r, KeyCode::Char('j'));
        press(&mut r, KeyCode::Char('j'));
        press(&mut r, KeyCode::Char('g'));
        press(&mut r, KeyCode::Char('l'));
        let b = &r.selecting.as_ref().unwrap().buf;
        assert_eq!(r.selected_text(b), "old a0");
    }
}
