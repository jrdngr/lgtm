//! Drawing text and backgrounds straight into a frame's buffer, clipped to an area.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

pub struct Canvas<'a> {
    pub buf: &'a mut Buffer,
    pub area: Rect,
    /// Where the terminal's own cursor shows, as a bar: where typing goes in insert mode.
    pub bar: Option<(u16, u16)>,
}

impl<'a> Canvas<'a> {
    pub fn new(buf: &'a mut Buffer, area: Rect) -> Self {
        Self {
            buf,
            area,
            bar: None,
        }
    }

    /// Puts the terminal's cursor bar at (`x`, `y`), if that's in the area.
    pub fn bar_at(&mut self, x: u16, y: u16) {
        if self.inside(x, y) {
            self.bar = Some((x, y));
        }
    }

    fn right(&self) -> u16 {
        self.area.right()
    }

    fn inside(&self, x: u16, y: u16) -> bool {
        x >= self.area.x && x < self.right() && y >= self.area.y && y < self.area.bottom()
    }

    /// Writes `s` at (`x`, `y`) up to the area's right edge. Returns the column after it.
    pub fn put(&mut self, x: u16, y: u16, s: &str, style: Style) -> u16 {
        if !self.inside(x, y) {
            return x;
        }
        let room = (self.right() - x) as usize;
        self.buf.set_stringn(x, y, s, room, style).0
    }

    /// Paints `w` cells from (`x`, `y`) with `bg`, clearing their text.
    pub fn fill(&mut self, x: u16, y: u16, w: u16, bg: Color) {
        if !self.inside(x, y) {
            return;
        }
        let end = x.saturating_add(w).min(self.right());
        for i in x..end {
            self.buf[(i, y)].set_symbol(" ").set_bg(bg);
        }
    }

    /// Sets the background of `w` cells from (`x`, `y`), keeping their text.
    pub fn tint(&mut self, x: u16, y: u16, w: u16, bg: Color) {
        if !self.inside(x, y) {
            return;
        }
        let end = x.saturating_add(w).min(self.right());
        for i in x..end {
            self.buf[(i, y)].set_bg(bg);
        }
    }

    /// Writes `text` a line per row from (`x`, `y`).
    pub fn put_lines(&mut self, x: u16, y: u16, text: &str, style: Style) {
        for (i, line) in text.lines().enumerate() {
            self.put(x, y.saturating_add(i as u16), line, style);
        }
    }

    /// Writes `s` so it ends at the area's right edge, less `pad`.
    pub fn put_right(&mut self, y: u16, pad: u16, s: &str, style: Style) {
        let w = s.chars().count() as u16;
        let x = self.right().saturating_sub(w + pad).max(self.area.x);
        self.put(x, y, s, style);
    }
}
