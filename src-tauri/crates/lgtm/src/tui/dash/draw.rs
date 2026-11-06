//! Draws DASH mode: the running turn in the top bar, then the files, the live diff, and the
//! todo list over the churn.

use lgtm_core::diff::RowKind;
use lgtm_core::store::TodoStatus;
use ratatui::layout::Rect;

use super::diff::{self, Line};
use super::{Dash, Focus, churn};
use crate::tui::files::FileTree;
use crate::tui::paint::Canvas;
use crate::tui::picker::Picker;
use crate::tui::theme::{
    self, ACCENT, ADD, BG, DEL, FAINT, FG, FRESH, GREEN, HAIR, MUTED, PANEL, RED, SEL,
};

/// How many rows the churn chart takes at most, and at least, with its title and totals.
const CHURN: u16 = 12;
const CHURN_MIN: u16 = 7;

impl Dash {
    /// Draws the panes, the home screen's revisions under the todo list and churn.
    pub fn draw(&mut self, c: &mut Canvas, body: Rect, tree: &mut FileTree, log: &mut Picker) {
        let side_w = (body.width * 26 / 100).clamp(20, 48);
        let most = body.width.saturating_sub(side_w + 22);
        let default = (body.width * 22 / 100).clamp(super::FILES_MIN, 40);
        let files_w = crate::tui::width::fit(self.files_width, default, super::FILES_MIN, most);
        self.files_drawn = files_w;
        if body.width < files_w + side_w + 20 || body.height < 3 {
            self.draw_diff(c, body);
            return;
        }
        // The files on their side, the turn and the log on the other, the diff between.
        let mid_w = body.width - files_w - side_w - 2;
        let left = crate::tui::side::files_left();
        let (files_x, side_x) = if left {
            (body.x, body.right() - side_w)
        } else {
            (body.right() - files_w, body.x)
        };
        let mid_x = body.x + if left { files_w } else { side_w } + 1;
        for y in body.y..body.bottom() {
            c.put(mid_x - 1, y, "│", theme::fg(HAIR).bg(BG));
            c.put(mid_x + mid_w, y, "│", theme::fg(HAIR).bg(BG));
        }
        let files = Rect::new(files_x, body.y, files_w, body.height);
        let focused = self.focus == Focus::Files;
        let end = c.put(files.x + 1, files.y, "Files", theme::title(focused).bg(BG));
        crate::tui::heat::mark(c, end, files.y, tree.heat_on);
        let rows = Rect::new(files.x, files.y + 1, files.width, files.height - 1);
        tree.draw(c, rows, focused, &|_| false);
        self.draw_diff(c, Rect::new(mid_x, body.y, mid_w, body.height));
        let top = body.height / 2;
        self.draw_side(c, Rect::new(side_x, body.y, side_w, top));
        let rule = "─".repeat(usize::from(side_w));
        c.put(side_x, body.y + top, &rule, theme::fg(HAIR).bg(BG));
        let revisions = Rect::new(side_x, body.y + top + 1, side_w, body.height - top - 1);
        let mut sub = Canvas::new(&mut *c.buf, revisions);
        log.draw(&mut sub, revisions, false);
        sub.put(
            side_x + 1,
            revisions.y,
            "Revisions",
            theme::title(false).bg(BG),
        );
    }

    fn draw_diff(&mut self, c: &mut Canvas, area: Rect) {
        let focused = self.focus == Focus::Diff;
        c.put(
            area.x + 1,
            area.y,
            "Live diff",
            theme::title(focused).bg(BG),
        );
        if self.following {
            let mut head = Canvas::new(&mut *c.buf, Rect::new(area.x, area.y, area.width, 1));
            head.put_right(area.y, 1, "following", theme::fg(ACCENT).bg(BG));
        }
        self.height = area.height.saturating_sub(1) as usize;
        if self.following {
            self.top = self.bottom();
        }
        let mut sub = Canvas::new(&mut *c.buf, area);
        for (i, line) in self
            .lines
            .iter()
            .skip(self.top)
            .take(self.height)
            .enumerate()
        {
            let y = area.y + 1 + i as u16;
            match *line {
                Line::File(file) => {
                    let d = &self.diffs[file];
                    let (a, r) = diff::counts(d);
                    sub.fill(area.x + 1, y, area.width - 1, PANEL);
                    let x = sub.put(area.x + 2, y, &d.path, theme::bold(FG).bg(PANEL));
                    let x = sub.put(x + 2, y, &format!("+{a}"), theme::fg(GREEN).bg(PANEL));
                    sub.put(x + 1, y, &format!("−{r}"), theme::fg(RED).bg(PANEL));
                }
                Line::Row { file, hunk, row } => {
                    let r = &self.diffs[file].hunks[hunk].rows[row];
                    let fresh = r.kind == RowKind::Added && file >= self.fresh;
                    let bg = match r.kind {
                        RowKind::Added if fresh => FRESH,
                        RowKind::Added => ADD,
                        RowKind::Removed => DEL,
                        RowKind::Context => BG,
                    };
                    sub.fill(area.x + 1, y, area.width - 1, bg);
                    if fresh {
                        sub.put(area.x, y, "▌", theme::fg(GREEN).bg(BG));
                    }
                    let num = |n: Option<u32>| n.map_or(String::new(), |n| n.to_string());
                    let lines = format!("{:>4} {:>4}  ", num(r.old_line), num(r.new_line));
                    let x = sub.put(area.x + 1, y, &lines, theme::fg(MUTED).bg(bg));
                    sub.put(x, y, &r.text.replace('\t', "    "), theme::fg(FG).bg(bg));
                }
                Line::Gap => {
                    sub.put(area.x + 2, y, "⋯", theme::fg(FAINT).bg(BG));
                }
                Line::Sep => {
                    let rule = "─".repeat(usize::from(area.width.saturating_sub(1)));
                    sub.put(area.x + 1, y, &rule, theme::fg(HAIR).bg(BG));
                }
            }
        }
    }

    fn draw_side(&self, c: &mut Canvas, area: Rect) {
        let mut sub = Canvas::new(&mut *c.buf, area);
        let x = area.x + 1;
        let Some(a) = &self.activity else {
            return;
        };
        let mut y = area.y;
        if !a.todos.is_empty() {
            let done = a
                .todos
                .iter()
                .filter(|t| t.status == TodoStatus::Completed)
                .count();
            sub.put(x, y, "Todo", theme::title(false).bg(BG));
            sub.put_right(
                y,
                1,
                &format!("{done}/{}", a.todos.len()),
                theme::fg(MUTED).bg(BG),
            );
            y += 1;
            let room = usize::from(area.width.saturating_sub(4));
            for t in &a.todos {
                if y + CHURN_MIN + 1 >= area.bottom() {
                    break;
                }
                let (mark, color, text, bg) = match t.status {
                    TodoStatus::Completed => ("✓", GREEN, MUTED, BG),
                    TodoStatus::InProgress => ("●", ACCENT, FG, SEL),
                    TodoStatus::Pending => ("○", FAINT, FG, BG),
                };
                sub.fill(area.x, y, area.width, bg);
                let x = sub.put(x, y, mark, theme::fg(color).bg(bg));
                sub.put(x + 1, y, &clip(&t.content, room), theme::fg(text).bg(bg));
                y += 1;
            }
            y += 1;
            let rule = "─".repeat(usize::from(area.width));
            sub.put(area.x, y, &rule, theme::fg(HAIR).bg(BG));
            y += 1;
        }
        if y + 4 > area.bottom() {
            return;
        }
        sub.put(x, y, "Churn", theme::title(false).bg(BG));
        let n = a.edits.len();
        let edits = format!("{n} edit{}", if n == 1 { "" } else { "s" });
        sub.put_right(y, 1, &edits, theme::fg(MUTED).bg(BG));
        let h = CHURN.min(area.bottom() - y - 1);
        let chart = Rect::new(x, y + 2, area.width.saturating_sub(2), h.saturating_sub(1));
        churn::draw(&mut sub, chart, &a.edits);
    }
}

/// `text` cut to `room` chars, with `…` where it was cut.
fn clip(text: &str, room: usize) -> String {
    if text.chars().count() <= room {
        return text.to_string();
    }
    let mut cut: String = text.chars().take(room.saturating_sub(1)).collect();
    cut.push('…');
    cut
}
