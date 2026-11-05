//! Draws DASH mode: the running turn in the top bar, then the files, the live diff, and the
//! todo list over the churn.

use lgtm_core::diff::RowKind;
use lgtm_core::store::{AgentActivity, TodoStatus};
use ratatui::layout::Rect;

use super::diff::{self, Line};
use super::{Dash, Focus, churn};
use crate::tui::files::FileTree;
use crate::tui::paint::Canvas;
use crate::tui::theme::{
    self, ACCENT, ADD, BG, DEL, FAINT, FG, FRESH, GREEN, HAIR, MUTED, PANEL, RED, SEL, TEAL,
};

/// How many rows the churn chart takes, with its title and totals.
const CHURN: u16 = 12;

impl Dash {
    /// The running turn after the title at `x` of the top bar: its prompt, then its line
    /// counts and how long it has run on the right.
    pub fn title(&self, c: &mut Canvas, x: u16, y: u16) {
        let Some(a) = &self.activity else {
            c.put(x, y, "no agent turn", theme::fg(MUTED).bg(PANEL));
            return;
        };
        let mut x = x;
        if a.running {
            x = c.put(x, y, "● ", theme::fg(ACCENT).bg(PANEL));
        }
        x = c.put(x, y, "›_ ", theme::fg(TEAL).bg(PANEL));
        let prompt = a
            .prompt
            .as_deref()
            .and_then(|p| p.lines().next())
            .unwrap_or("");
        c.put(x, y, prompt, theme::fg(FG).bg(PANEL));
        let (added, removed) = a
            .edits
            .iter()
            .fold((0, 0), |(x, y), e| (x + e.added, y + e.removed));
        let time = elapsed(a);
        let counts = format!("+{added} −{removed}");
        let w = (counts.chars().count() + time.chars().count() + 2) as u16;
        let right = c.area.right().saturating_sub(w + 1);
        c.fill(right.saturating_sub(1), y, w + 1, PANEL);
        let x = c.put(right, y, &format!("+{added}"), theme::fg(GREEN).bg(PANEL));
        let x = c.put(x + 1, y, &format!("−{removed}"), theme::fg(RED).bg(PANEL));
        c.put(x + 2, y, &time, theme::fg(MUTED).bg(PANEL));
    }

    pub fn draw(&mut self, c: &mut Canvas, body: Rect, tree: &mut FileTree) {
        let files_w = (body.width * 22 / 100).clamp(16, 40);
        let side_w = (body.width * 26 / 100).clamp(20, 48);
        if body.width < files_w + side_w + 20 || body.height < 3 {
            self.draw_diff(c, body);
            return;
        }
        let mid_x = body.x + files_w + 1;
        let mid_w = body.width - files_w - side_w - 2;
        let side_x = mid_x + mid_w + 1;
        for y in body.y..body.bottom() {
            c.put(body.x + files_w, y, "│", theme::fg(HAIR).bg(BG));
            c.put(side_x - 1, y, "│", theme::fg(HAIR).bg(BG));
        }
        let files = Rect::new(body.x, body.y, files_w, body.height);
        let focused = self.focus == Focus::Files;
        let end = c.put(files.x + 1, files.y, "Files", theme::title(focused).bg(BG));
        crate::tui::heat::mark(c, end, files.y, tree.heat_on);
        let rows = Rect::new(files.x, files.y + 1, files.width, files.height - 1);
        tree.draw(c, rows, focused, &|_| false);
        self.draw_diff(c, Rect::new(mid_x, body.y, mid_w, body.height));
        self.draw_side(c, Rect::new(side_x, body.y, side_w, body.height));
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
        if self.jump {
            self.jump_to_fresh();
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
                    let fresh = r.kind == RowKind::Added && self.fresh.contains(&(file, hunk, row));
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
                if y + CHURN + 1 >= area.bottom() {
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

/// How long the turn has run, or ran: `1m 42s`, or `42s` under a minute.
fn elapsed(a: &AgentActivity) -> String {
    let end = a.ended_at.unwrap_or_else(lgtm_core::turns::now_ms);
    let s = (end - a.started_at).max(0) / 1000;
    if s < 60 {
        format!("{s}s")
    } else {
        format!("{}m {:02}s", s / 60, s % 60)
    }
}
