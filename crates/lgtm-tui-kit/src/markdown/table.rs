//! Markdown tables, drawn with box lines.

use ratatui::style::Modifier;

use super::render::Run;
use crate::theme::{self, BG, HAIR};

#[derive(Clone, Copy)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// A table being read: rows of cells of runs. The first row is the header.
pub struct Table {
    aligns: Vec<Align>,
    rows: Vec<Vec<Vec<Run>>>,
    src: usize,
}

/// The narrowest a column gets.
const MIN: usize = 3;

impl Table {
    pub fn new(aligns: Vec<Align>, src: usize) -> Self {
        Self {
            aligns,
            rows: vec![],
            src,
        }
    }

    pub fn row(&mut self) {
        self.rows.push(vec![]);
    }

    pub fn cell(&mut self) {
        if let Some(r) = self.rows.last_mut() {
            r.push(vec![]);
        }
    }

    pub fn push(&mut self, run: Run) {
        if let Some(c) = self.rows.last_mut().and_then(|r| r.last_mut()) {
            c.push(run);
        }
    }

    /// The table's lines within `width` columns, each with its source line. Columns too
    /// wide to fit are narrowed, widest first, and their cells cut with `…`.
    pub fn lines(&self, width: usize) -> Vec<(Vec<Run>, usize)> {
        let cols = self.rows.iter().map(Vec::len).max().unwrap_or(0);
        if cols == 0 {
            return vec![];
        }
        let cell_w = |c: &Vec<Run>| c.iter().map(Run::width).sum::<usize>();
        let mut widths: Vec<usize> = (0..cols)
            .map(|i| {
                self.rows
                    .iter()
                    .filter_map(|r| r.get(i))
                    .map(cell_w)
                    .max()
                    .unwrap_or(0)
                    .max(MIN)
            })
            .collect();
        // Each column has a space either side, and a bar between columns.
        let room = width.saturating_sub(3 * cols - 1);
        while widths.iter().sum::<usize>() > room {
            let (i, &w) = widths.iter().enumerate().max_by_key(|(_, w)| **w).unwrap();
            if w <= MIN {
                break;
            }
            widths[i] -= 1;
        }
        let bar = || Run::new("│", theme::fg(HAIR).bg(BG));
        let mut out = vec![];
        for (r, row) in self.rows.iter().enumerate() {
            let mut line = vec![];
            for (i, &w) in widths.iter().enumerate() {
                if i > 0 {
                    line.push(bar());
                }
                let empty = vec![];
                let cell = row.get(i).unwrap_or(&empty);
                let align = self.aligns.get(i).copied().unwrap_or(Align::Left);
                line.extend(fit(cell, w, align, r == 0));
            }
            out.push((line, self.src + r + usize::from(r > 0)));
            if r == 0 {
                let rule: Vec<String> = widths.iter().map(|w| "─".repeat(w + 2)).collect();
                out.push((
                    vec![Run::new(rule.join("┼"), theme::fg(HAIR).bg(BG))],
                    self.src + 1,
                ));
            }
        }
        out
    }
}

/// A cell's runs padded or cut to `w` columns, with a space either side.
fn fit(cell: &[Run], w: usize, align: Align, header: bool) -> Vec<Run> {
    let mut runs: Vec<Run> = vec![];
    let total: usize = cell.iter().map(Run::width).sum();
    let mut left = w;
    for run in cell {
        if left == 0 {
            break;
        }
        let mut run = run.clone();
        if header {
            run.style = run.style.add_modifier(Modifier::BOLD);
        }
        if total > w && run.width() >= left {
            run.text = run.text.chars().take(left - 1).collect::<String>() + "…";
        }
        left -= run.width().min(left);
        runs.push(run);
    }
    let (before, after) = match align {
        Align::Left => (0, left),
        Align::Right => (left, 0),
        Align::Center => (left / 2, left - left / 2),
    };
    let pad = |n: usize| Run::new(" ".repeat(n + 1), theme::fg(BG).bg(BG));
    let mut out = vec![pad(before)];
    out.extend(runs);
    out.push(pad(after));
    out
}
