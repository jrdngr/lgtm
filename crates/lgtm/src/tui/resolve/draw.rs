//! Drawing the resolver: the file bar, the text, each conflict's head and sides, and the
//! top bar's count.

use lgtm_core::resolve::{Fate, Label, PartView, SideView};
use lgtm_core::syntax::{self, Segment};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::Resolver;
use super::layout::{self, Row, Slot};
use crate::edit::keys::Mode;
use crate::tui::edit::draw::{char_colors, draw_line};
use crate::tui::keybar::Hint;
use crate::tui::paint::Canvas;
use crate::tui::theme::{
    self, ACCENT, AMBER, BG, FAINT, FG, GREEN, HAIR, MUTED, PANEL, SEL, UNIQUE, rgb,
};

/// A kept side's label row.
const RAISE: Color = rgb(0x2a2a2b);
/// A side key's badge when the side isn't kept.
const KEY_BG: Color = rgb(0x3a3a3a);
/// Words a side changed.
const TINT: Color = rgb(0x24452c);
/// The bar beside a resolved conflict.
const DONE_BAR: Color = rgb(0x3d6b47);
/// An edit's toggle when on.
const EDITED_BG: Color = rgb(0x4a3c14);

impl Resolver {
    /// The count, or Resolved, on the right of the top bar.
    pub fn status(&self) -> (String, Color) {
        if self.view.done {
            ("Resolved".into(), GREEN)
        } else {
            (
                format!("{} of {} resolved", self.view.resolved, self.view.total),
                AMBER,
            )
        }
    }

    pub fn badge(&self) -> (&'static str, Color) {
        match self.mode() {
            Some(Mode::Insert) => ("INS", GREEN),
            Some(_) => ("NOR", ACCENT),
            None => ("RES", AMBER),
        }
    }

    pub fn hints(&self) -> Vec<Hint<'static>> {
        if self.editing.is_some() {
            return match self.keys.mode {
                Mode::Insert => vec![("Esc", "normal", false)],
                _ => vec![
                    ("i", "insert", false),
                    ("Esc", "done", false),
                    ("u U", "undo", false),
                ],
            };
        }
        let mut hints = match self.view.fate {
            Fate::Here => vec![],
            Fate::Deleted { .. } => vec![("F", "keep file", false), ("D", "delete file", false)],
            Fate::Moved { .. } => vec![
                ("c", "carry", false),
                ("b", "keep both", false),
                ("x", "drop", false),
            ],
        };
        hints.extend([
            ("1 2 3", "pick", false),
            ("0", "base", false),
            ("n N", "conflict", false),
            ("J K", "reorder", false),
            ("i", "edit", false),
            ("o", "original", false),
            ("u U", "undo", false),
            (":w", "save", false),
            (":q!", "discard", false),
        ]);
        hints
    }

    /// The text a row's line comes from, and which of its lines it is.
    fn source(&self, row: &Row) -> Option<(String, usize)> {
        match row {
            Row::Text { part, n, .. } => match self.view.parts.get(*part)? {
                PartView::Text { text, .. } => Some((text.clone(), *n)),
                _ => None,
            },
            Row::Code {
                conflict,
                slot,
                key,
                n,
                ..
            } => {
                let c = self.view.parts.iter().find_map(|p| match p {
                    PartView::Conflict(c) if c.index as usize == *conflict => Some(c),
                    _ => None,
                })?;
                let side: &SideView = match slot {
                    Slot::Pick(p) => c.picks.get(*p)?,
                    Slot::Rest => c.rest.iter().find(|s| s.key == *key)?,
                };
                Some((side.text.clone(), *n))
            }
            _ => None,
        }
    }

    /// The highlight segments of a row's line.
    fn colors(&mut self, row: &Row) -> Vec<Segment> {
        let Some((text, n)) = self.source(row) else {
            return vec![];
        };
        if !self.colors.contains_key(&text) {
            if self.colors.len() > 200 {
                self.colors.clear();
            }
            let lines = syntax::highlight_file(&self.session.path, &text);
            self.colors.insert(text.clone(), lines);
        }
        self.colors[&text].get(n).cloned().unwrap_or_default()
    }

    pub fn draw(&mut self, c: &mut Canvas, body: Rect) {
        self.height = body.height as usize;
        if let Some(e) = &self.editing {
            let text = e.buf.text.to_string();
            let (conflict, pick) = (e.conflict, e.pick);
            let line = e.buf.line_of(e.buf.sel.head);
            self.rows = layout::rows(&self.view);
            layout::editing(&mut self.rows, conflict, pick, &text);
            let mine = |r: &Row| matches!(r, Row::Code { conflict: c, slot: Slot::Pick(p), n, .. } if *c == conflict && *p == pick && *n == line);
            if let Some(i) = self.rows.iter().position(mine) {
                self.cursor = i;
            }
        }
        let height = body.height as usize;
        if self.cursor < self.top {
            self.top = self.cursor;
        } else if height > 0 && self.cursor >= self.top + height {
            self.top = self.cursor + 1 - height;
        }
        self.top = self.top.min(self.rows.len().saturating_sub(1));
        let last_line = self.rows.iter().filter_map(|r| match r {
            Row::Text { line, .. }
            | Row::Code {
                line: Some(line), ..
            } => Some(*line),
            _ => None,
        });
        let num_w = last_line.max().unwrap_or(1).to_string().len().max(3) as u16;
        let rows: Vec<Row> = self
            .rows
            .iter()
            .skip(self.top)
            .take(height)
            .cloned()
            .collect();
        for (i, row) in rows.iter().enumerate() {
            let y = body.y + i as u16;
            let at = self.top + i == self.cursor;
            self.draw_row(c, body, y, row, at, num_w);
        }
    }

    fn bar(&self, conflict: usize) -> Color {
        let resolved = self.view.parts.iter().any(
            |p| matches!(p, PartView::Conflict(c) if c.index as usize == conflict && c.resolved),
        );
        if resolved { DONE_BAR } else { AMBER }
    }

    fn draw_row(&mut self, c: &mut Canvas, body: Rect, y: u16, row: &Row, at: bool, num_w: u16) {
        let code_x = body.x + 2 + num_w + 2;
        match row {
            Row::Blank => {}
            Row::FileBar => self.draw_file_bar(c, body, y, at),
            Row::Text { line, text, .. } => {
                let n = format!("{line:>w$}", w = num_w as usize);
                c.put(
                    body.x + 2,
                    y,
                    &n,
                    theme::fg(if at { FG } else { FAINT }).bg(BG),
                );
                let segs = self.colors(row);
                draw_code(
                    c,
                    (code_x, y, body.right()),
                    text,
                    &segs,
                    &[],
                    at,
                    Style::default(),
                );
            }
            Row::Head { conflict } => {
                c.fill(body.x, y, body.width, PANEL);
                c.put(body.x + 1, y, "▎", theme::fg(self.bar(*conflict)).bg(PANEL));
                let title = format!("Conflict {} of {}", conflict + 1, self.view.total);
                let style = if at {
                    theme::fg(BG).bg(FG)
                } else {
                    theme::fg(MUTED).bg(PANEL)
                };
                let x = c.put(body.x + 3, y, &title[..1], style);
                let x = c.put(x, y, &title[1..], theme::fg(MUTED).bg(PANEL)) + 2;
                let (pill, color) = match self.bar(*conflict) == AMBER {
                    true => ("open", AMBER),
                    false => ("resolved", GREEN),
                };
                c.put(x, y, pill, theme::bold(color).bg(PANEL));
            }
            Row::Side {
                conflict,
                key,
                slot,
                open,
            } => {
                let picked = matches!(slot, Slot::Pick(_));
                let bg = if picked { RAISE } else { BG };
                c.fill(body.x, y, body.width, bg);
                c.put(body.x + 1, y, "▎", theme::fg(self.bar(*conflict)).bg(bg));
                let badge = format!(" {key} ");
                let style = match (picked, open) {
                    _ if at => theme::bold(BG).bg(FG),
                    (true, _) => theme::bold(BG).bg(ACCENT),
                    (false, true) => theme::bold(FG).bg(KEY_BG),
                    (false, false) => theme::fg(MUTED).bg(KEY_BG),
                };
                if picked {
                    c.put(body.x + 3, y, "⠿", theme::fg(FAINT).bg(bg));
                } else if !open {
                    c.put(body.x + 3, y, "▸", theme::fg(MUTED).bg(bg));
                }
                c.put(body.x + 5, y, &badge, style);
                let side = self.side_view(*conflict, *slot, *key);
                if let Some(s) = &side {
                    label(c, body.x + 9, y, &s.label, bg, !picked && !open);
                }
                if let Some(s) = side.filter(|s| s.edited) {
                    let x = body.right().saturating_sub(20);
                    let (on, off) = (theme::bold(AMBER).bg(EDITED_BG), theme::fg(MUTED).bg(bg));
                    let (edited, original) = if s.original { (off, on) } else { (on, off) };
                    c.put(x, y, " Edited ", edited);
                    c.put(x + 8, y, " Original ", original);
                }
            }
            Row::Code {
                conflict,
                line,
                text,
                tints,
                slot,
                ..
            } => {
                c.put(body.x + 1, y, "▎", theme::fg(self.bar(*conflict)).bg(BG));
                if let Some(line) = line {
                    let n = format!("{line:>w$}", w = num_w as usize);
                    c.put(
                        body.x + 2,
                        y,
                        &n,
                        theme::fg(if at { FG } else { FAINT }).bg(BG),
                    );
                }
                let editing = self
                    .editing
                    .as_ref()
                    .filter(|e| e.conflict == *conflict && Slot::Pick(e.pick) == *slot);
                if let Some(e) = editing {
                    let n = match row {
                        Row::Code { n, .. } => *n,
                        _ => 0,
                    };
                    let width = body.right().saturating_sub(code_x) as usize;
                    let segs = vec![];
                    draw_line(c, &e.buf, self.keys.mode, n, &segs, code_x, y, 0, width);
                    return;
                }
                let gone = if self.view.removing {
                    theme::fg(FAINT).add_modifier(Modifier::CROSSED_OUT)
                } else {
                    Style::default()
                };
                let segs = self.colors(row);
                draw_code(c, (code_x, y, body.right()), text, &segs, tints, at, gone);
            }
        }
    }

    fn side_view(&self, conflict: usize, slot: Slot, key: u8) -> Option<SideView> {
        self.view.parts.iter().find_map(|p| match p {
            PartView::Conflict(c) if c.index as usize == conflict => match slot {
                Slot::Pick(i) => c.picks.get(i).cloned(),
                Slot::Rest => c.rest.iter().find(|s| s.key == key).cloned(),
            },
            _ => None,
        })
    }

    fn draw_file_bar(&self, c: &mut Canvas, body: Rect, y: u16, at: bool) {
        c.fill(body.x, y, body.width, PANEL);
        let chosen = self.view.choice;
        c.put(
            body.x + 1,
            y,
            "▎",
            theme::fg(if chosen.is_some() { HAIR } else { AMBER }).bg(PANEL),
        );
        let (words, by) = match &self.view.fate {
            Fate::Here => return,
            Fate::Deleted { by } => ("Deleted in".to_string(), by),
            Fate::Moved { to, by } => (format!("Moved to {to} in"), by),
        };
        let first = if at {
            theme::fg(BG).bg(FG)
        } else {
            theme::fg(FG).bg(PANEL)
        };
        let x = c.put(body.x + 3, y, &words[..1], first);
        let x = c.put(x, y, &words[1..], theme::fg(FG).bg(PANEL)) + 1;
        label(c, x, y, by, PANEL, false);
        let to = match &self.view.fate {
            Fate::Moved { to, .. } => format!(" to {to}"),
            _ => String::new(),
        };
        let items: Vec<(String, &str, bool)> = self
            .choices()
            .iter()
            .map(|(k, l, ch)| {
                let l = if *ch == lgtm_core::resolve::FileChoice::Carry {
                    format!("{l}{to}")
                } else {
                    l.to_string()
                };
                (l, *k, chosen == Some(*ch))
            })
            .collect();
        let width: u16 = items
            .iter()
            .map(|(l, k, _)| (l.chars().count() + k.len() + 4) as u16)
            .sum();
        let mut x = body.right().saturating_sub(width + 1);
        for (l, k, on) in items {
            let bg = if on { SEL } else { PANEL };
            x = c.put(x, y, k, theme::bold(ACCENT).bg(bg));
            let style = if on {
                theme::bold(FG).bg(bg)
            } else {
                theme::fg(FG).bg(bg)
            };
            x = c.put(x, y, &format!(" {l}"), style);
            x += 3;
        }
    }
}

/// A label: change id with its unique prefix, commit id, bookmarks, description.
fn label(c: &mut Canvas, x: u16, y: u16, l: &Label, bg: Color, dim: bool) -> u16 {
    let mut x = x;
    if !l.change_id.is_empty() {
        let unique = (l.unique as usize).min(l.change_id.len());
        x = c.put(x, y, &l.change_id[..unique], theme::bold(UNIQUE).bg(bg));
        x = c.put(x, y, &l.change_id[unique..], theme::fg(FAINT).bg(bg)) + 1;
        x = c.put(x, y, &l.commit_id, theme::fg(FAINT).bg(bg)) + 1;
    }
    for b in &l.bookmarks {
        x = c.put(x, y, b, theme::fg(if dim { FAINT } else { UNIQUE }).bg(bg)) + 1;
    }
    c.put(
        x,
        y,
        &l.description,
        theme::fg(if dim { MUTED } else { FG }).bg(bg),
    )
}

/// A line of code from `x`, in its colors with tinted runs, and the cursor on its first char.
fn draw_code(
    c: &mut Canvas,
    (x, y, right): (u16, u16, u16),
    text: &str,
    segs: &[Segment],
    tints: &[(u32, u32)],
    cursor: bool,
    over: Style,
) {
    let colors = char_colors(text, segs);
    let mut col = x;
    let width = right.saturating_sub(x) as usize;
    for (i, ch) in text.chars().enumerate() {
        if (col - x) as usize >= width {
            break;
        }
        let tinted = tints
            .iter()
            .any(|&(a, b)| (a as usize) <= i && i < b as usize);
        let fg = colors.get(i).copied().unwrap_or(FG);
        let mut style = theme::fg(fg).bg(if tinted { TINT } else { BG }).patch(over);
        if cursor && i == 0 {
            style = theme::fg(BG).bg(FG);
        }
        let s = if ch == '\t' {
            "    ".to_string()
        } else {
            ch.to_string()
        };
        col = c.put(col, y, &s, style);
    }
    if cursor && text.is_empty() {
        c.put(x, y, " ", theme::fg(BG).bg(FG));
    }
}
