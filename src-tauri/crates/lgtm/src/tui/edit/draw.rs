//! Drawing the editor: the top bar, line numbers, the text in its colors with the selection
//! over it, and the key bar.

use lgtm_core::syntax::{self, HIGHLIGHT_NAMES, Segment};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use super::{Editor, Open};
use crate::edit::buffer::Buffer;
use crate::edit::gutter::{self, Mark};
use crate::edit::keymap;
use crate::edit::keys::Mode;
use crate::tui::keybar::Hint;
use crate::tui::paint::Canvas;
use crate::tui::theme::{self, ACCENT, AMBER, BG, FAINT, FG, GREEN, HAIR, MUTED, PANEL, RED, SEL};

/// Columns a tab takes.
const TAB: usize = 4;
/// Columns the sidebar takes, with its border.
const SIDEBAR: u16 = 30;
/// The narrowest screen that shows the sidebar.
const MIN_SIDEBAR_WIDTH: u16 = 100;

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
                if self.open[self.cur].conflict {
                    let pad = at.chars().count() as u16 + 4;
                    c.put_right(y, pad, "changed on disk", theme::fg(AMBER).bg(PANEL));
                }
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
        if self.in_sidebar {
            return vec![
                ("↑ ↓", "move", false),
                ("← →", "close, open", false),
                ("Enter", "open", false),
                ("Esc", "back", false),
            ];
        }
        let conflict = self.open.get(self.cur).is_some_and(|o| o.conflict);
        let mut hints = match self.keys.mode {
            Mode::Insert => vec![("Esc", "normal", false)],
            Mode::Normal | Mode::Select => vec![
                ("space", "menu", false),
                ("z", "fold", false),
                ("i", "insert", false),
                ("v", "select", self.keys.mode == Mode::Select),
                ("/", "search", false),
                (":", "command", false),
                ("?", "keys", false),
            ],
        };
        if conflict {
            hints.splice(1..1, [(":reload", "reload", false), (":w", "save", false)]);
        }
        hints
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

    /// Draws the sidebar, when there's room, and returns what's left for the text.
    fn draw_sidebar(&mut self, c: &mut Canvas, body: Rect) -> Rect {
        let room = body.width >= MIN_SIDEBAR_WIDTH || self.in_sidebar && body.width > SIDEBAR * 2;
        if !room || self.sidebar.paths().is_empty() || body.height < 2 {
            return body;
        }
        let open = &self.open;
        let marked = |p: &str| open.iter().any(|o| o.buf.name == p && o.buf.dirty());
        let tree = Rect::new(body.x, body.y + 1, SIDEBAR - 1, body.height - 1);
        self.sidebar.draw(c, tree, true, &marked);
        for y in body.y..body.bottom() {
            c.put(body.x + SIDEBAR, y, "│", theme::fg(HAIR).bg(BG));
        }
        Rect::new(
            body.x + SIDEBAR + 1,
            body.y,
            body.width - SIDEBAR - 1,
            body.height,
        )
    }

    pub fn draw(&mut self, c: &mut Canvas, full: Rect) {
        let body = self.draw_sidebar(c, full);
        self.draw_text(c, body);
        if let Some(f) = self.finder.as_mut() {
            f.draw(c, full);
        }
    }

    fn draw_text(&mut self, c: &mut Canvas, body: Rect) {
        self.height = body.height as usize;
        let mode = self.keys.mode;
        let Some(open) = self.open.get_mut(self.cur) else {
            return;
        };
        open.recolor();
        open.remark();
        open.folds.update(&open.buf);
        let last = open.buf.last_line();
        let visible = open.folds.visible(last);
        let num_w = (last + 1).to_string().len().max(3) as u16;
        // A column for the diff gutter, the numbers, then a space before the text.
        let code_x = body.x + 1 + num_w + 2;
        let width = body.right().saturating_sub(code_x) as usize;
        open.scroll(&visible, body.height as usize, width);
        let first = visible.partition_point(|&l| l < open.top);
        let left = open.left;
        let b = &open.buf;
        let colors = open
            .colors
            .as_ref()
            .map(|(_, c)| c.as_slice())
            .unwrap_or(&[]);
        let hidden = open.folds.hidden();
        let marks = open
            .marks
            .as_ref()
            .map(|(_, m)| m.as_slice())
            .unwrap_or(&[]);
        let cursor_line = b.line_of(b.sel.head);
        for (row, &line) in visible
            .iter()
            .skip(first)
            .take(body.height as usize)
            .enumerate()
        {
            let y = body.y + row as u16;
            let num = if line == cursor_line { FG } else { FAINT };
            let n = format!("{:>w$}", line + 1, w = num_w as usize);
            c.put(body.x + 1, y, &n, theme::fg(num).bg(BG));
            if let Some(Some(mark)) = marks.get(line) {
                let (s, color) = match mark {
                    Mark::Added => ("▍", GREEN),
                    Mark::Modified => ("▍", AMBER),
                    Mark::Deleted => ("▔", RED),
                };
                c.put(body.x, y, s, theme::fg(color).bg(BG));
            }
            let segs = colors.get(line).map(Vec::as_slice).unwrap_or(&[]);
            let end = draw_line(c, b, mode, line, segs, code_x, y, left, width);
            if let Some(&(_, fold_end)) = hidden.iter().find(|&&(s, _)| s == line) {
                draw_fold(c, b, fold_end, code_x, y, end.saturating_sub(left));
            }
        }
    }
}

impl Open {
    /// Works out the gutter's marks again when the text or its base changed.
    fn remark(&mut self) {
        let Some(base) = &self.base else {
            return;
        };
        if self
            .marks
            .as_ref()
            .is_some_and(|(v, _)| *v == self.buf.version)
        {
            return;
        }
        let marks = gutter::marks(base, &self.buf.text.to_string());
        self.marks = Some((self.buf.version, marks));
    }

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

    /// Scrolls so the cursor is in view, `height` of the `visible` lines by `width` columns.
    fn scroll(&mut self, visible: &[usize], height: usize, width: usize) {
        let b = &self.buf;
        let line = b.line_of(b.sel.head);
        let at = visible.partition_point(|&l| l < line);
        let top = visible.partition_point(|&l| l < self.top);
        let top = if at < top {
            at
        } else if height > 0 && at >= top + height {
            at + 1 - height
        } else {
            top
        };
        self.top = visible.get(top).copied().unwrap_or(0);
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

/// After a closed fold's first line: a mark for the hidden lines, and the last line when
/// it's only a closing bracket or so.
fn draw_fold(c: &mut Canvas, b: &Buffer, end: usize, x0: u16, y: u16, col: usize) {
    let x = c.put(x0 + col as u16, y, " ⋯ ", theme::fg(FAINT).bg(BG));
    let last = b.slice(b.line_start(end), b.line_end(end));
    let last = last.trim();
    if last.chars().count() <= 3 {
        c.put(x, y, last, theme::fg(FG).bg(BG));
    }
}

/// Draws a line of text. Returns the column after it.
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
) -> usize {
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
    col
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
