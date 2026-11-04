//! Drawing the review: the top bar, the file list, and the diff's lines.

use lgtm_core::diff::{Row, RowKind};
use lgtm_core::jj::FileDiff;
use std::ops::Range;

use ratatui::layout::Rect;
use ratatui::style::Color;

use super::marks::State;
use super::rows::{Line, NoteKey};
use super::wrap::{self, code_parts};
use super::{Review, tree};
use crate::tui::highlight::{self, Sides};
use crate::tui::load::Target;
use crate::tui::paint::Canvas;
use crate::tui::theme::{
    self, ACCENT, ADD, AMBER, BG, BLANK, CYAN, DEL, FAINT, FG, FUCHSIA, GREEN, HAIR, MUTED, NOTE,
    PANEL, RED, SEL, THREAD, UNIQUE,
};

/// Width of a line number column.
pub const NUM: u16 = 4;
/// Where code starts in unified view, past the gutter and line numbers.
pub const CODE_X: u16 = 13;

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
        State::Partial | State::None => (" ", FAINT),
    }
}

impl Review {
    /// Writes the review's ids and description into the top bar from `x`, and on its right
    /// how much is reviewed, or a failure.
    pub fn title(&self, c: &mut Canvas, x: u16, y: u16) {
        if let Some(notice) = &self.notice {
            c.put_right(y, 1, notice, theme::fg(RED).bg(PANEL));
        } else {
            let pct = self.percent().map(|p| format!("{p}%")).unwrap_or_default();
            c.put_right(y, 1, &pct, theme::fg(GREEN).bg(PANEL));
            if let Some((_, n)) = &self.pr {
                let right = match pct.len() {
                    0 => 1,
                    n => n as u16 + 4,
                };
                c.put_right(y, right, &format!("#{n}"), theme::fg(CYAN).bg(PANEL));
            }
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
                match &self.target {
                    Target::Between { span, .. } => {
                        let x = c.put(x + 2, y, "◇ ", theme::fg(AMBER).bg(PANEL));
                        c.put(x, y, span, theme::fg(FG).bg(PANEL));
                    }
                    Target::Revset(_) => {
                        let summary = one.description.lines().next().unwrap_or("");
                        c.put(x + 2, y, summary, theme::fg(FG).bg(PANEL));
                    }
                }
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
        let list = if self.show_list && body.width >= 100 {
            (body.width / 5).clamp(24, 40)
        } else {
            0
        };
        self.list_area = Rect::default();
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
        self.list_area = area;
        let entries = self.entries();
        let at = self.list_at(&entries);
        let height = area.height.saturating_sub(1) as usize;
        if at < self.list_top {
            self.list_top = at;
        } else if height > 0 && at >= self.list_top + height {
            self.list_top = at + 1 - height;
        }
        let mut c = Canvas::new(&mut *c.buf, area);
        c.put(
            area.x + 1,
            area.y,
            "Files",
            theme::title(self.in_list).bg(BG),
        );
        for (n, entry) in entries.iter().enumerate().skip(self.list_top).take(height) {
            let y = area.y + 1 + (n - self.list_top) as u16;
            let picked = n == at;
            let bg = match (picked, self.in_list) {
                (true, true) => SEL,
                (true, false) => HAIR,
                _ => BG,
            };
            c.fill(area.x, y, area.width, bg);
            let (state, name, depth, color) = match entry {
                tree::Entry::Dir { name, path, depth } => {
                    let fold = if self.closed_dirs.contains(path) {
                        "▸"
                    } else {
                        "▾"
                    };
                    (
                        self.dir_state(path),
                        format!("{fold} {name}/"),
                        *depth,
                        MUTED,
                    )
                }
                tree::Entry::File { file, name, depth } => {
                    (self.file_state(*file), name.clone(), *depth, FG)
                }
                tree::Entry::Reviewed { count, open } => {
                    let fold = if *open { "▾" } else { "▸" };
                    (State::None, format!("{fold} Reviewed {count}"), 0, MUTED)
                }
            };
            let (mark, mark_color) = icon(state);
            c.put(area.x + 1, y, mark, theme::fg(mark_color).bg(bg));
            let style = if self.in_list && picked {
                theme::bold(FG)
            } else {
                theme::fg(color)
            };
            let x = area.x + 3 + 2 * depth as u16;
            let x = c.put(x, y, &name, style.bg(bg));
            if let tree::Entry::File { file, .. } = entry {
                if self.unsaved.contains(&self.files[*file].path) {
                    c.put(x + 1, y, "●", theme::fg(FG).bg(bg));
                }
                // The counts on the right, open comments left of them and resolved ones left
                // of those, over the name's end if need be.
                let (added, removed) = counts(&self.files[*file]);
                let open = self.open_comments(*file);
                let resolved = self.resolved_comments(*file);
                let mut parts = vec![(format!("+{added}"), GREEN), (format!("−{removed}"), RED)];
                if open > 0 {
                    parts.insert(0, (format!("{open}"), ACCENT));
                }
                if resolved > 0 {
                    parts.insert(0, (format!("✓{resolved}"), FG));
                }
                let width: usize = parts.iter().map(|(t, _)| t.chars().count() + 1).sum();
                let mut x = area.right().saturating_sub(width as u16).max(area.x);
                c.fill(x.saturating_sub(1), y, width as u16 + 1, bg);
                for (text, color) in &parts {
                    x = c.put(x, y, text, theme::fg(*color).bg(bg)) + 1;
                }
            }
        }
    }

    /// The list entry on the list's screen row `y`, if any.
    pub(super) fn list_entry_at(&self, y: u16) -> Option<tree::Entry> {
        let n = y.checked_sub(self.list_area.y + 1)? as usize;
        self.entries().get(self.list_top + n).cloned()
    }

    fn draw_diff(&mut self, c: &mut Canvas) {
        let area = c.area;
        self.diff_area = area;
        let height = area.height as usize;
        self.height = height;
        if self.width != area.width as usize {
            self.width = area.width as usize;
            self.relayout();
        }
        // A rule above a header isn't a place for the cursor: it goes to the header.
        if matches!(self.lines.get(self.cursor), Some(Line::Gap(_))) {
            self.cursor += 1;
        }
        // At least one line, for the file header pinned at the top.
        let margin = if self.pinned && (self.top + 1..self.top + height).contains(&self.cursor) {
            1
        } else {
            (height / 4).min(3)
        };
        // The view keeps a margin only on the side the cursor is moving toward, so a line put
        // at the top by a jump stays there as the cursor moves down from it.
        let up = self.cursor < self.drawn_cursor;
        let down = self.cursor > self.drawn_cursor;
        if let Some(top) = self.snap {
            let last = self.lines.len().saturating_sub(height);
            self.top = top.min(last);
        } else if self.cursor < self.top || (up && self.cursor < self.top + margin) {
            self.top = self.cursor.saturating_sub(margin);
        } else if self.cursor >= self.top + height
            || (down && self.cursor + margin >= self.top + height)
        {
            self.top = (self.cursor + margin + 1).saturating_sub(height);
        }
        self.drawn_cursor = self.cursor;
        let selection = self.selection();
        let selecting = self.selecting.is_some();
        for (n, &line) in self.lines.iter().enumerate().skip(self.top).take(height) {
            let y = area.y + (n - self.top) as u16;
            let cursor = !selecting
                && !self.in_list
                && (n == self.cursor || selection.as_ref().is_some_and(|s| s.contains(&n)));
            let chars = self.selected_chars(n);
            let gutter = line
                .hunk()
                .and_then(|h| bar(self.hunk_state(line.file(), h)));
            match line {
                Line::File(f) => {
                    let folded = self.folded.contains(&self.files[f].path);
                    let unsaved = self.unsaved.contains(&self.files[f].path);
                    let check = self.file_check(f);
                    file_header(c, y, &self.files[f], check, folded, unsaved, cursor)
                }
                Line::Gap(_) => {
                    c.put(area.x, y, &"─".repeat(area.width.into()), theme::fg(FAINT));
                }
                Line::Hunk(..) => hunk_divider(c, y, cursor),
                Line::Code {
                    file,
                    hunk,
                    old,
                    new,
                    part,
                } => {
                    let rows = &self.files[file].hunks[hunk].rows;
                    let sides = self.sides[file].as_ref();
                    let row = |i: Option<usize>| i.map(|i| &rows[i]);
                    let show = Show {
                        part,
                        wrap: self.wrap.then(|| self.code_width()),
                        left: self.left,
                        chars,
                    };
                    if self.split {
                        split_row(c, y, sides, (row(old), row(new)), cursor, &show);
                    } else if let Some(r) = row(old) {
                        unified_row(c, y, sides, r, cursor, &show);
                    }
                }
                Line::Note { key, part, .. } => self.draw_note(c, y, key, part, cursor),
            }
            if let Some(color) = gutter {
                c.put(area.x, y, "▌", theme::fg(color));
            }
        }
        // The file the top line is in keeps its header at the top.
        self.sticky = match self.lines.get(self.top) {
            Some(Line::Gap(_) | Line::File(_)) | None => None,
            Some(line) => Some(line.file()),
        };
        if let Some(f) = self.sticky {
            let folded = self.folded.contains(&self.files[f].path);
            let unsaved = self.unsaved.contains(&self.files[f].path);
            let check = self.file_check(f);
            file_header(c, area.y, &self.files[f], check, folded, unsaved, false);
        }
    }
}

impl Review {
    fn draw_note(&self, c: &mut Canvas, y: u16, key: NoteKey, part: usize, cursor: bool) {
        let x0 = c.area.x + 2;
        let w = c.area.width.saturating_sub(3);
        match key {
            NoteKey::Comment(n) => {
                let Some(comment) = self.comment(n) else {
                    return;
                };
                let bg = if cursor { SEL } else { NOTE };
                let (accent, fg) = if comment.done {
                    (MUTED, MUTED)
                } else {
                    (ACCENT, FG)
                };
                c.fill(x0, y, w, bg);
                c.put(x0, y, "▎", theme::fg(accent).bg(bg));
                let id = format!("C{n} ");
                let lines = self.comment_lines(comment);
                let text = lines.get(part).map_or("", String::as_str);
                let x = if part == 0 {
                    c.put(x0 + 2, y, &id, theme::bold(accent).bg(bg))
                } else {
                    x0 + 2 + id.len() as u16
                };
                c.put(x, y, text, theme::fg(fg).bg(bg));
            }
            NoteKey::Thread(id) => {
                let Some(t) = self.thread(id) else { return };
                let bg = if cursor { SEL } else { THREAD };
                c.fill(x0, y, w, bg);
                c.put(x0, y, "▎", theme::fg(CYAN).bg(bg));
                let tag = format!("{} ", self.thread_label(id));
                let lines = t.lines();
                let Some(line) = lines.get(part) else { return };
                let mut x = if part == 0 {
                    c.put(x0 + 2, y, &tag, theme::bold(CYAN).bg(bg))
                } else {
                    x0 + 2 + tag.len() as u16
                };
                if part == 0 && t.outdated() {
                    x = c.put(x, y, "outdated  ", theme::fg(AMBER).bg(bg));
                }
                if let Some(who) = line.who {
                    x = c.put(x, y, &format!("{who}  "), theme::fg(FG).bg(bg));
                }
                c.put(x, y, line.text, theme::fg(FG).bg(bg));
            }
            NoteKey::Input => {
                let lines = self.input_lines();
                let (at_line, at_col) = self.input_cursor();
                let border = theme::fg(ACCENT).bg(BG);
                c.fill(x0, y, w, BG);
                let inner = w.saturating_sub(2) as usize;
                match part {
                    0 => {
                        let head = format!("╭─ C{} ", self.input_number());
                        let rule = inner.saturating_sub(head.chars().count() - 1);
                        c.put(x0, y, &format!("{head}{}╮", "─".repeat(rule)), border);
                    }
                    _ if part <= lines.len() => {
                        c.put(x0, y, "│", border);
                        let line = lines[part - 1].as_str();
                        c.put(x0 + 2, y, line, theme::fg(FG).bg(BG));
                        if part - 1 == at_line {
                            let under = line.chars().nth(at_col).unwrap_or(' ');
                            let x = x0 + 2 + at_col as u16;
                            c.put(x, y, &under.to_string(), theme::fg(BG).bg(FG));
                        }
                        c.put(x0 + w - 1, y, "│", border);
                    }
                    _ => {
                        c.put(x0, y, &format!("╰{}╯", "─".repeat(inner)), border);
                    }
                }
            }
        }
    }
}

/// Where a file header's checkbox is, from the diff's left edge.
pub const CHECK_X: std::ops::Range<u16> = 1..4;

/// A file's header: its checkbox (`check` is `Some(true)` when an agent's), then `▸` when it's
/// folded.
fn file_header(
    c: &mut Canvas,
    y: u16,
    f: &FileDiff,
    check: Option<bool>,
    folded: bool,
    unsaved: bool,
    cursor: bool,
) {
    let bg = if cursor { SEL } else { PANEL };
    let x0 = c.area.x;
    c.fill(x0, y, c.area.width, bg);
    let mut x = c.put(x0 + CHECK_X.start, y, "[", theme::fg(MUTED).bg(bg));
    x = match check {
        Some(agent) => c.put(
            x,
            y,
            "✓",
            theme::fg(if agent { CYAN } else { GREEN }).bg(bg),
        ),
        None => c.put(x, y, " ", theme::fg(MUTED).bg(bg)),
    };
    x = c.put(x, y, "] ", theme::fg(MUTED).bg(bg));
    if folded {
        x = c.put(x, y, "▸ ", theme::fg(MUTED).bg(bg));
    }
    if let Some(old) = &f.old_path {
        x = c.put(x, y, old, theme::fg(MUTED).bg(bg));
        x = c.put(x, y, " → ", theme::fg(MUTED).bg(bg));
    }
    x = c.put(x, y, &f.path, theme::bold(FG).bg(bg));
    if unsaved {
        x = c.put(x + 1, y, "●", theme::fg(FG).bg(bg));
    }
    let (added, removed) = counts(f);
    x = c.put(x + 2, y, &format!("+{added}"), theme::fg(GREEN).bg(bg));
    c.put(x + 1, y, &format!("−{removed}"), theme::fg(RED).bg(bg));
}

/// The file's added and removed rows.
fn counts(f: &FileDiff) -> (usize, usize) {
    let count = |k| {
        f.hunks
            .iter()
            .flat_map(|h| &h.rows)
            .filter(|r| r.kind == k)
            .count()
    };
    (count(RowKind::Added), count(RowKind::Removed))
}

/// The divider above a hunk, in a file with more than one.
fn hunk_divider(c: &mut Canvas, y: u16, cursor: bool) {
    let bg = if cursor { SEL } else { BG };
    let x0 = c.area.x;
    c.fill(x0, y, c.area.width, bg);
    let width = c.area.width.saturating_sub(3) as usize;
    c.put(x0 + 2, y, &"╌".repeat(width), theme::fg(HAIR).bg(bg));
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

/// The chars of a row selected with `v`, and the cursor's char when it's on the row.
type Chars = Option<(Range<usize>, Option<usize>)>;

/// Which part of a row's code a screen line shows.
#[derive(Clone)]
struct Show {
    /// The screen line of the row, and the columns rows wrap to, when they do.
    part: usize,
    wrap: Option<usize>,
    /// Columns scrolled off to the left.
    left: usize,
    /// Selected chars, counted from the part's first.
    chars: Chars,
}

/// The row's text in syntax colors, from the side it belongs to: the part `show` picks, with
/// its selected chars.
fn code(c: &mut Canvas, x: u16, y: u16, sides: Option<&Sides>, r: &Row, bg: Color, show: &Show) {
    let segs = sides.and_then(|s| match r.kind {
        RowKind::Removed => s.old.get(r.old_line? as usize - 1),
        _ => s.new.get(r.new_line? as usize - 1),
    });
    let colors: Vec<Color> = highlight::runs(&r.text, segs.map_or(&[], Vec::as_slice))
        .into_iter()
        .flat_map(|(text, color)| text.chars().map(move |_| color).collect::<Vec<_>>())
        .collect();
    let chars: Vec<char> = r.text.chars().collect();
    let span = match show.wrap {
        Some(w) => code_parts(&r.text, w)
            .get(show.part)
            .cloned()
            .unwrap_or(chars.len()..chars.len()),
        None => 0..chars.len(),
    };
    // Columns before the span, and the first one on screen.
    let start: usize = chars[..span.start].iter().map(|&c| wrap::cols(c)).sum();
    let first = start + if show.wrap.is_some() { 0 } else { show.left };
    let (range, head) = show.chars.clone().unwrap_or_default();
    let mut col = start;
    for k in 0..=span.len() {
        let i = span.start + k;
        // Past the span's last char, only the cursor shows.
        let ch = match chars.get(i) {
            Some(&ch) if i < span.end => ch,
            _ if head == Some(k) => ' ',
            _ => break,
        };
        let fg = colors.get(col).copied().unwrap_or(FG);
        let style = if head == Some(k) {
            theme::fg(BG).bg(FG)
        } else if range.contains(&k) {
            theme::fg(fg).bg(SEL)
        } else {
            theme::fg(fg).bg(bg)
        };
        let glyph = if ch == '\t' {
            " ".to_string()
        } else {
            ch.to_string()
        };
        for n in col..col + wrap::cols(ch) {
            if let Some(dx) = n.checked_sub(first).and_then(|d| u16::try_from(d).ok()) {
                c.put(x.saturating_add(dx), y, &glyph, style);
            }
        }
        col += wrap::cols(ch);
    }
}

fn unified_row(c: &mut Canvas, y: u16, sides: Option<&Sides>, r: &Row, cursor: bool, show: &Show) {
    let bg = if cursor { SEL } else { row_bg(r) };
    let x0 = c.area.x;
    c.fill(x0 + 1, y, c.area.width.saturating_sub(1), bg);
    if show.part == 0 {
        let x = c.put(x0 + 2, y, &num(r.old_line), theme::fg(FAINT).bg(bg));
        c.put(x + 1, y, &num(r.new_line), theme::fg(FAINT).bg(bg));
    }
    code(c, x0 + CODE_X, y, sides, r, bg, show);
}

fn split_row(
    c: &mut Canvas,
    y: u16,
    sides: Option<&Sides>,
    (old, new): (Option<&Row>, Option<&Row>),
    cursor: bool,
    show: &Show,
) {
    let x0 = c.area.x;
    let half = c.area.width.saturating_sub(3) / 2;
    // The selection is on the new side, or the old where there's no new row.
    let selected = if new.is_some() { 1 } else { 0 };
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
            if show.part == 0 {
                let n = if i == 0 { r.old_line } else { r.new_line };
                side.put(sx, y, &num(n), theme::fg(FAINT).bg(bg));
            }
            let show = Show {
                chars: if i == selected {
                    show.chars.clone()
                } else {
                    None
                },
                ..show.clone()
            };
            code(&mut side, sx + NUM + 1, y, sides, r, bg, &show);
        }
        if i == 0 {
            c.put(sx + half, y, "│", theme::fg(HAIR).bg(BG));
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::buffer::Buffer;

    use super::super::tests::file;
    use super::*;

    #[test]
    fn the_list_shows_each_files_counts_on_the_right_over_a_long_name() {
        let mut r = Review::new(1, Target::Revset("@".into()));
        r.loaded(Ok((vec![], vec![file("a_long_file_name.rs", 2)])));
        let area = Rect::new(0, 0, 20, 2);
        let mut buf = Buffer::empty(area);
        r.draw_list(&mut Canvas::new(&mut buf, area), area);
        let row: String = (0..20).map(|x| buf[(x, 1)].symbol()).collect();
        assert_eq!(row, "   a_long_fil +2 −2 ");
    }
}
