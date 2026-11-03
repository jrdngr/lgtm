//! Drawing the editor: the top bar, line numbers, the text in its colors with the selection
//! over it, and the key bar.

use lgtm_core::syntax::{self, HIGHLIGHT_NAMES, Segment};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use super::{Editor, Open};
use crate::edit::buffer::Buffer;
use crate::edit::keymap;
use crate::edit::keys::Mode;
use crate::tui::keybar::Hint;
use crate::tui::paint::Canvas;
use crate::tui::theme::{self, ACCENT, AMBER, BG, FAINT, FG, GREEN, MUTED, PANEL, RED, SEL};

/// Columns a tab takes.
const TAB: usize = 4;

impl Editor {
    /// Writes the file's name into the top bar from `x`, and on its right the cursor's line
    /// and column, or a notice.
    pub fn title(&self, c: &mut Canvas, x: u16, y: u16) {
        let Some(b) = self.current() else {
            return;
        };
        let x = c.put(x, y, &b.name, theme::bold(FG).bg(PANEL));
        if b.dirty() {
            c.put(x + 2, y, "●", theme::fg(FG).bg(PANEL));
        }
        match &self.notice {
            Some(n) => c.put_right(y, 1, n, theme::fg(RED).bg(PANEL)),
            None => {
                let line = b.line_of(b.sel.head);
                let col = b.sel.head - b.line_start(line);
                let at = format!("{}:{}", line + 1, col + 1);
                c.put_right(y, 1, &at, theme::fg(MUTED).bg(PANEL));
            }
        }
    }

    /// The mode's badge for the key bar.
    pub fn badge(&self) -> (&'static str, Color) {
        match self.keys.mode {
            Mode::Normal => ("NOR", ACCENT),
            Mode::Insert => ("INS", GREEN),
            Mode::Select => ("SEL", AMBER),
        }
    }

    pub fn hints(&self) -> Vec<Hint<'static>> {
        match self.keys.mode {
            Mode::Insert => vec![("Esc", "normal", false)],
            Mode::Normal | Mode::Select => vec![
                ("space", "menu", false),
                ("i", "insert", false),
                ("v", "select", self.keys.mode == Mode::Select),
                ("/", "search", false),
                (":", "command", false),
                ("?", "keys", false),
            ],
        }
    }

    /// What the keys typed so far could go on to, for the menu over the key bar.
    pub fn pending_menu(&self) -> Vec<(String, &'static str)> {
        let pending = self.keys.pending();
        if pending.is_empty() {
            return vec![];
        }
        keymap::after(&pending, self.keys.mode == Mode::Select)
            .into_iter()
            .map(|(k, l)| (k.to_string(), l))
            .collect()
    }

    pub fn draw(&mut self, c: &mut Canvas, body: Rect) {
        self.height = body.height as usize;
        let mode = self.keys.mode;
        let Some(open) = self.open.get_mut(self.cur) else {
            return;
        };
        open.recolor();
        let last = open.buf.last_line();
        let num_w = (last + 1).to_string().len().max(3) as u16;
        // A column for the diff gutter, the numbers, then a space before the text.
        let code_x = body.x + 1 + num_w + 2;
        let width = body.right().saturating_sub(code_x) as usize;
        open.scroll(body.height as usize, width);
        let (top, left) = (open.top, open.left);
        let b = &open.buf;
        let colors = open
            .colors
            .as_ref()
            .map(|(_, c)| c.as_slice())
            .unwrap_or(&[]);
        let cursor_line = b.line_of(b.sel.head);
        for row in 0..body.height {
            let line = top + row as usize;
            if line > last {
                break;
            }
            let y = body.y + row;
            let num = if line == cursor_line { FG } else { FAINT };
            let n = format!("{:>w$}", line + 1, w = num_w as usize);
            c.put(body.x + 1, y, &n, theme::fg(num).bg(BG));
            let segs = colors.get(line).map(Vec::as_slice).unwrap_or(&[]);
            draw_line(c, b, mode, line, segs, code_x, y, left, width);
        }
    }
}

impl Open {
    /// Highlights the text again when it changed since last time.
    fn recolor(&mut self) {
        if self
            .colors
            .as_ref()
            .is_some_and(|(v, _)| *v == self.buf.version)
        {
            return;
        }
        let text = self.buf.text.to_string();
        let colors = syntax::highlight_file(&self.buf.name, &text);
        self.colors = Some((self.buf.version, colors));
    }

    /// Scrolls so the cursor is in view, `height` lines by `width` columns.
    fn scroll(&mut self, height: usize, width: usize) {
        let b = &self.buf;
        let line = b.line_of(b.sel.head);
        if line < self.top {
            self.top = line;
        } else if height > 0 && line >= self.top + height {
            self.top = line + 1 - height;
        }
        let col = column(b, b.sel.head);
        if col < self.left {
            self.left = col;
        } else if width > 0 && col >= self.left + width {
            self.left = col + 1 - width;
        }
    }
}

/// The screen column of char `at` in its line, tabs expanded.
fn column(b: &Buffer, at: usize) -> usize {
    let start = b.line_start(b.line_of(at));
    b.text
        .slice(start..at.min(b.len()))
        .chars()
        .map(|c| if c == '\t' { TAB } else { 1 })
        .sum()
}

/// Each char's color in a line, from its highlight segments, which count UTF-16 units.
fn char_colors(text: &str, segs: &[Segment]) -> Vec<Color> {
    let mut out = Vec::with_capacity(text.len());
    let mut segs = segs.iter().copied();
    let mut current = segs.next();
    for ch in text.chars() {
        let color = current
            .and_then(|(class, _)| class)
            .and_then(|c| HIGHLIGHT_NAMES.get(c as usize))
            .map_or(FG, |name| theme::syntax(name));
        out.push(color);
        if let Some((_, len)) = current.as_mut() {
            let used = ch.len_utf16() as u32;
            if *len > used {
                *len -= used;
            } else {
                current = segs.next();
            }
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn draw_line(
    c: &mut Canvas,
    b: &Buffer,
    mode: Mode,
    line: usize,
    segs: &[Segment],
    x0: u16,
    y: u16,
    left: usize,
    width: usize,
) {
    let start = b.line_start(line);
    let end = b.line_end(line);
    let text: String = b.text.slice(start..end).chars().collect();
    let text = text.strip_suffix('\r').unwrap_or(&text);
    let colors = char_colors(text, segs);
    let selected = |i: usize| mode != Mode::Insert && i >= b.sel.from() && i <= b.sel.to();
    let cursor = |i: usize| i == b.sel.head;
    let mut col = 0usize;
    let cell = |c: &mut Canvas, col: usize, s: &str, style: Style| {
        if col >= left && col < left + width {
            c.put(x0 + (col - left) as u16, y, s, style);
        }
    };
    for (k, ch) in text.chars().enumerate() {
        let i = start + k;
        let w = if ch == '\t' { TAB } else { 1 };
        let fg = colors.get(k).copied().unwrap_or(FG);
        let style = if cursor(i) {
            theme::fg(BG).bg(FG)
        } else if selected(i) {
            theme::fg(fg).bg(SEL)
        } else {
            theme::fg(fg).bg(BG)
        };
        if ch == '\t' {
            for n in 0..w {
                cell(c, col + n, " ", style);
            }
        } else {
            cell(c, col, &ch.to_string(), style);
        }
        col += w;
    }
    // The newline, or the end of the text, when the cursor or selection is on it.
    let eol = start + text.chars().count();
    if cursor(eol) {
        cell(c, col, " ", theme::fg(BG).bg(FG));
    } else if selected(eol) && eol < b.len() {
        cell(c, col, " ", theme::fg(FG).bg(SEL));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn char_colors_follow_utf16_segments() {
        let keyword = HIGHLIGHT_NAMES
            .iter()
            .position(|n| *n == "keyword")
            .unwrap() as u8;
        // "😀" is two UTF-16 units: the keyword run covers "fn" and stops before it.
        let colors = char_colors(
            "fn😀x",
            &[(Some(keyword), 2), (None, 2), (Some(keyword), 1)],
        );
        let kw = theme::syntax("keyword");
        assert_eq!(colors, vec![kw, kw, FG, kw]);
    }
}
