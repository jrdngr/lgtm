//! The mouse over the text: a click puts the cursor there and a drag selects, in any mode,
//! scrolling at the text's top and bottom rows.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};

use super::Editor;
use crate::edit::buffer::Sel;
use crate::edit::keys::Mode;
use crate::edit::motion;
use crate::edit::wrap::char_at;

/// A screen row of text as last drawn: its line, and the column shown at `x`.
pub(super) struct TextRow {
    pub y: u16,
    pub x: u16,
    pub line: usize,
    pub col: usize,
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

    /// On the text's top or bottom row, the char a row past `at`, which the view scrolls to
    /// show; elsewhere `at`.
    fn past_edge(&mut self, at: usize, y: u16) -> usize {
        let by = match (self.rows.first(), self.rows.last()) {
            (Some(top), _) if y <= top.y => -1,
            (_, Some(bottom)) if y >= bottom.y => 1,
            _ => return at,
        };
        let open = &mut self.open[self.cur];
        open.folds.update(&open.buf);
        let hidden = open.folds.hidden();
        let b = &open.buf;
        let to = match self.wrap.then_some(self.text_width) {
            Some(width) => motion::vertical_wrapped(b, at, by, None, &hidden, width),
            None => motion::vertical_folded(b, at, by, None, &hidden),
        };
        to.0
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
                let at = self.past_edge(at, y);
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
