//! Drawing the review: the top bar, the file list, and the diff's lines.

use lgtm_core::diff::{Row, RowKind};
use lgtm_core::jj::FileDiff;
use ratatui::layout::Rect;
use ratatui::style::Color;

use super::Review;
use super::marks::State;
use super::rows::Line;
use crate::tui::highlight::{self, Sides};
use crate::tui::paint::Canvas;
use crate::tui::theme::{
    self, ACCENT, ADD, BG, BLANK, CYAN, DEL, FAINT, FG, FUCHSIA, GREEN, HAIR, MUTED, PANEL, RED,
    SEL, UNIQUE,
};

/// Width of a line number column.
const NUM: u16 = 4;

/// The gutter color of a hunk in `state`.
fn bar(state: State) -> Option<Color> {
    match state {
        State::Reviewed => Some(GREEN),
        State::Agent => Some(CYAN),
        State::Flagged => Some(RED),
        State::AgentFlagged => Some(FUCHSIA),
        State::None | State::Partial => None,
    }
}

/// A file's icon in the file list.
fn icon(state: State) -> (&'static str, Color) {
    match state {
        State::Reviewed => ("✓", GREEN),
        State::Agent => ("✓", CYAN),
        State::Flagged => ("!", RED),
        State::AgentFlagged => ("!", FUCHSIA),
        State::Partial => ("◐", GREEN),
        State::None => (" ", FAINT),
    }
}

impl Review {
    /// Writes the review's ids and description into the top bar from `x`, and on its right
    /// how much is reviewed, or a failure.
    pub fn title(&self, c: &mut Canvas, x: u16, y: u16) {
        if let Some(notice) = &self.notice {
            c.put_right(y, 1, notice, theme::fg(RED).bg(PANEL));
        } else if let Some(pct) = self.percent() {
            c.put_right(y, 1, &format!("{pct}%"), theme::fg(GREEN).bg(PANEL));
        }
        let id = |c: &mut Canvas, x: u16, ch: &lgtm_core::jj::Change| {
            let id: String = ch.change_id.chars().take(8).collect();
            let unique = (ch.unique as usize).min(id.len());
            let x = c.put(x, y, &id[..unique], theme::fg(UNIQUE).bg(PANEL));
            c.put(x, y, &id[unique..], theme::fg(MUTED).bg(PANEL))
        };
        match self.changes.as_slice() {
            [] => {
                c.put(x, y, &self.revset, theme::fg(FG).bg(PANEL));
            }
            [one] => {
                let x = id(c, x, one);
                let summary = one.description.lines().next().unwrap_or("");
                c.put(x + 2, y, summary, theme::fg(FG).bg(PANEL));
            }
            [oldest, .., newest] => {
                let x = id(c, x, oldest);
                let x = c.put(x, y, "..", theme::fg(MUTED).bg(PANEL));
                id(c, x, newest);
            }
        }
    }

    pub fn draw(&mut self, c: &mut Canvas, body: Rect) {
        if let Some(e) = &self.error {
            c.put_lines(body.x + 1, body.y + 1, e, theme::fg(RED));
            return;
        }
        let list = if body.width >= 100 {
            (body.width / 5).clamp(24, 40)
        } else {
            0
        };
        if list > 0 {
            self.draw_list(c, Rect::new(body.x, body.y, list, body.height));
            for y in body.top()..body.bottom() {
                c.put(body.x + list, y, "│", theme::fg(HAIR).bg(BG));
            }
        }
        let x0 = body.x + list + u16::from(list > 0);
        let diff = Rect::new(x0, body.y, body.right() - x0, body.height);
        self.draw_diff(&mut Canvas::new(&mut *c.buf, diff));
    }

    fn draw_list(&mut self, c: &mut Canvas, area: Rect) {
        let selected = self.current().map_or(0, Line::file);
        let height = area.height.saturating_sub(1) as usize;
        if selected < self.list_top {
            self.list_top = selected;
        } else if height > 0 && selected >= self.list_top + height {
            self.list_top = selected + 1 - height;
        }
        let mut c = Canvas::new(&mut *c.buf, area);
        for (n, (i, f)) in self
            .files
            .iter()
            .enumerate()
            .skip(self.list_top)
            .take(height)
            .enumerate()
        {
            let y = area.y + 1 + n as u16;
            let bg = if i == selected { SEL } else { BG };
            c.fill(area.x, y, area.width, bg);
            let (mark, color) = icon(self.file_state(i));
            c.put(area.x + 1, y, mark, theme::fg(color).bg(bg));
            let name = f.path.rsplit('/').next().unwrap_or(&f.path);
            let dir = &f.path[..f.path.len() - name.len()];
            let x = c.put(area.x + 3, y, dir, theme::fg(MUTED).bg(bg));
            let style = if self.in_list && i == selected {
                theme::bold(FG)
            } else {
                theme::fg(FG)
            };
            c.put(x, y, name, style.bg(bg));
        }
    }

    fn draw_diff(&mut self, c: &mut Canvas) {
        let area = c.area;
        let height = area.height as usize;
        self.height = height;
        let margin = (height / 4).min(3);
        if self.cursor < self.top + margin {
            self.top = self.cursor.saturating_sub(margin);
        } else if self.cursor + margin >= self.top + height {
            self.top = (self.cursor + margin + 1).saturating_sub(height);
        }
        for (n, &line) in self.lines.iter().enumerate().skip(self.top).take(height) {
            let y = area.y + (n - self.top) as u16;
            let cursor = n == self.cursor;
            let gutter = line
                .hunk()
                .and_then(|h| bar(self.hunk_state(line.file(), h)));
            match line {
                Line::File(f) => file_header(c, y, &self.files[f], cursor),
                Line::Hunk(f, h) => {
                    let folded = self.folded.contains(&(f, h));
                    hunk_header(c, y, &self.files[f].hunks[h], folded, cursor)
                }
                Line::Code {
                    file,
                    hunk,
                    old,
                    new,
                } => {
                    let rows = &self.files[file].hunks[hunk].rows;
                    let sides = self.sides[file].as_ref();
                    let row = |i: Option<usize>| i.map(|i| &rows[i]);
                    if self.split {
                        split_row(c, y, sides, row(old), row(new), cursor);
                    } else if let Some(r) = row(old) {
                        unified_row(c, y, sides, r, cursor);
                    }
                }
            }
            if let Some(color) = gutter {
                c.put(area.x, y, "▌", theme::fg(color));
            }
        }
    }
}

fn file_header(c: &mut Canvas, y: u16, f: &FileDiff, cursor: bool) {
    let bg = if cursor { SEL } else { PANEL };
    let x0 = c.area.x;
    c.fill(x0, y, c.area.width, bg);
    let mut x = x0 + 1;
    if let Some(old) = &f.old_path {
        x = c.put(x, y, old, theme::fg(MUTED).bg(bg));
        x = c.put(x, y, " → ", theme::fg(MUTED).bg(bg));
    }
    x = c.put(x, y, &f.path, theme::bold(FG).bg(bg));
    let count = |k| {
        f.hunks
            .iter()
            .flat_map(|h| &h.rows)
            .filter(|r| r.kind == k)
            .count()
    };
    x = c.put(
        x + 2,
        y,
        &format!("+{}", count(RowKind::Added)),
        theme::fg(GREEN).bg(bg),
    );
    c.put(
        x + 1,
        y,
        &format!("−{}", count(RowKind::Removed)),
        theme::fg(RED).bg(bg),
    );
}

fn hunk_header(c: &mut Canvas, y: u16, h: &lgtm_core::diff::Hunk, folded: bool, cursor: bool) {
    let bg = if cursor { SEL } else { BG };
    let x0 = c.area.x;
    c.fill(x0, y, c.area.width, bg);
    let mut x = x0 + 2;
    if folded {
        x = c.put(x, y, "▸ ", theme::fg(MUTED).bg(bg));
    }
    let range = format!(
        "@@ -{},{} +{},{} @@",
        h.old_start, h.old_lines, h.new_start, h.new_lines
    );
    x = c.put(x, y, &range, theme::fg(ACCENT).bg(bg));
    if !h.section.is_empty() {
        c.put(x + 1, y, h.section.trim(), theme::fg(MUTED).bg(bg));
    }
    if folded {
        let changed = h.rows.iter().filter(|r| r.kind != RowKind::Context).count();
        c.put_right(y, 1, &format!("{changed} lines"), theme::fg(FAINT).bg(bg));
    }
}

fn row_bg(r: &Row) -> Color {
    match r.kind {
        RowKind::Added => ADD,
        RowKind::Removed => DEL,
        RowKind::Context => BG,
    }
}

fn num(n: Option<u32>) -> String {
    n.map_or_else(
        || " ".repeat(NUM as usize),
        |n| format!("{n:>w$}", w = NUM as usize),
    )
}

/// The row's text in syntax colors, from the side it belongs to.
fn code(c: &mut Canvas, x: u16, y: u16, sides: Option<&Sides>, r: &Row, bg: Color) {
    let segs = sides.and_then(|s| match r.kind {
        RowKind::Removed => s.old.get(r.old_line? as usize - 1),
        _ => s.new.get(r.new_line? as usize - 1),
    });
    let mut x = x;
    for (text, color) in highlight::runs(&r.text, segs.map_or(&[], Vec::as_slice)) {
        x = c.put(x, y, &text, theme::fg(color).bg(bg));
    }
}

fn unified_row(c: &mut Canvas, y: u16, sides: Option<&Sides>, r: &Row, cursor: bool) {
    let bg = if cursor { SEL } else { row_bg(r) };
    let x0 = c.area.x;
    c.fill(x0 + 1, y, c.area.width.saturating_sub(1), bg);
    let x = c.put(x0 + 2, y, &num(r.old_line), theme::fg(FAINT).bg(bg));
    let x = c.put(x + 1, y, &num(r.new_line), theme::fg(FAINT).bg(bg));
    let (sign, color) = match r.kind {
        RowKind::Added => ("+", GREEN),
        RowKind::Removed => ("-", RED),
        RowKind::Context => (" ", FAINT),
    };
    c.put(x + 1, y, sign, theme::fg(color).bg(bg));
    code(c, x + 3, y, sides, r, bg);
}

fn split_row(
    c: &mut Canvas,
    y: u16,
    sides: Option<&Sides>,
    old: Option<&Row>,
    new: Option<&Row>,
    cursor: bool,
) {
    let x0 = c.area.x;
    let half = c.area.width.saturating_sub(3) / 2;
    for (i, r) in [old, new].into_iter().enumerate() {
        let sx = x0 + 2 + i as u16 * (half + 1);
        let bg = match r {
            _ if cursor && i == 1 => SEL,
            Some(r) => row_bg(r),
            None => BLANK,
        };
        let mut side = Canvas::new(&mut *c.buf, Rect::new(sx, y, half, 1));
        side.fill(sx, y, half, bg);
        if let Some(r) = r {
            let n = if i == 0 { r.old_line } else { r.new_line };
            side.put(sx, y, &num(n), theme::fg(FAINT).bg(bg));
            code(&mut side, sx + NUM + 1, y, sides, r, bg);
        }
        if i == 0 {
            c.put(sx + half, y, "│", theme::fg(HAIR).bg(BG));
        }
    }
}
