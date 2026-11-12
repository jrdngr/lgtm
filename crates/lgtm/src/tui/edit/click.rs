//! The mouse over the text: a click puts the cursor there and a drag selects, in any mode.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};

use super::Editor;
use super::draw::TAB;
use crate::edit::buffer::{Buffer, Sel};
use crate::edit::keys::Mode;

/// A screen row of text as last drawn: its line, and the column shown at `x`.
pub(super) struct TextRow {
    pub y: u16,
    pub x: u16,
    pub line: usize,
    pub col: usize,
}

/// The char at column `col` of `line`, tabs expanded, or the line's end past it.
fn char_at(b: &Buffer, line: usize, col: usize) -> usize {
    let start = b.line_start(line);
    let text = b.slice(start, b.line_end(line));
    let text = text.trim_end_matches('\r');
    let mut at = 0;
    for (k, ch) in text.chars().enumerate() {
        at += if ch == '\t' { TAB } else { 1 };
        if at > col {
            return start + k;
        }
    }
    start + text.chars().count()
}

impl Editor {
    /// The char drawn at a screen cell: the nearest row's, left of the text is its start.
    fn char_at_cell(&self, x: u16, y: u16) -> Option<usize> {
        let row = self
            .rows
            .iter()
            .rev()
            .find(|r| r.y <= y)
            .or(self.rows.first())?;
        let b = &self.open.get(self.cur)?.buf;
        let col = row.col + x.saturating_sub(row.x) as usize;
        Some(char_at(b, row.line, col))
    }

    /// Clicks and drags over the text.
    pub(super) fn text_mouse(&mut self, kind: MouseEventKind, x: u16, y: u16) {
        let Some(at) = self.char_at_cell(x, y) else {
            return;
        };
        match kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.in_sidebar = false;
                self.drag_from = Some(at);
                let b = &mut self.open[self.cur].buf;
                b.sel = Sel::point(at);
                b.column = None;
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                let Some(from) = self.drag_from else {
                    return;
                };
                if from == at {
                    return;
                }
                if self.keys.mode == Mode::Insert {
                    self.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
                }
                let b = &mut self.open[self.cur].buf;
                b.sel = Sel::new(from, at);
                b.column = None;
                b.clamp();
            }
            MouseEventKind::Up(MouseButton::Left) => self.drag_from = None,
            _ => {}
        }
    }
}
