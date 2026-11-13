//! Selecting rendered text with the mouse: a drag selects from where it started. A click
//! without a drag clears it.

use ratatui::crossterm::event::{MouseButton, MouseEventKind};

use super::Preview;

/// A rendered line and a column in it.
pub type Pos = (usize, usize);

impl Preview {
    /// Scrolls with the wheel and selects with a drag.
    pub fn mouse(&mut self, kind: MouseEventKind, x: u16, y: u16) {
        match kind {
            MouseEventKind::ScrollDown => self.scroll(3),
            MouseEventKind::ScrollUp => self.scroll(-3),
            MouseEventKind::Down(MouseButton::Left) => {
                self.sel = self.pos(x, y).map(|p| (p, p));
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let (Some((from, _)), Some(to)) = (self.sel, self.pos(x, y)) {
                    self.sel = Some((from, to));
                }
            }
            MouseEventKind::Up(MouseButton::Left) if self.selection().is_empty() => {
                self.sel = None;
            }
            _ => {}
        }
    }

    /// The rendered line and column at a screen cell, clamped to the text.
    fn pos(&self, x: u16, y: u16) -> Option<Pos> {
        let (area, x0) = self.drawn;
        if !area.contains((x, y).into()) || self.lines.is_empty() {
            return None;
        }
        let line = (self.top + (y - area.y) as usize).min(self.lines.len() - 1);
        Some((line, x.saturating_sub(x0) as usize))
    }

    /// The selection's start and end in reading order.
    fn ordered(&self) -> Option<(Pos, Pos)> {
        let (a, b) = self.sel?;
        Some(if a <= b { (a, b) } else { (b, a) })
    }

    /// The columns selected on rendered line `line`, as a start and an end.
    pub(super) fn selected(&self, line: usize) -> Option<(usize, usize)> {
        let ((l0, c0), (l1, c1)) = self.ordered()?;
        if line < l0 || line > l1 || (l0, c0) == (l1, c1) {
            return None;
        }
        let len = self.line_text(line).chars().count();
        let from = if line == l0 { c0.min(len) } else { 0 };
        let to = if line == l1 { (c1 + 1).min(len) } else { len };
        (from < to).then_some((from, to))
    }

    fn line_text(&self, line: usize) -> String {
        self.lines[line]
            .runs
            .iter()
            .map(|r| r.text.as_str())
            .collect()
    }

    /// The selected text, a line per rendered line, without trailing spaces.
    pub fn selection(&self) -> String {
        let Some(((l0, _), (l1, _))) = self.ordered() else {
            return String::new();
        };
        (l0..=l1)
            .map(|l| {
                let (from, to) = self.selected(l).unwrap_or((0, 0));
                let text: String = self
                    .line_text(l)
                    .chars()
                    .skip(from)
                    .take(to - from)
                    .collect();
                text.trim_end().to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
