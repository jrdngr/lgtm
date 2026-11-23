//! Markdown tables, drawn with box lines.

use ratatui::style::Modifier;

use super::render::Run;
use super::wrap::wrap;
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
    /// wide to fit are narrowed, widest first, and their cells wrapped.
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
            let cells: Vec<Vec<Vec<Run>>> = widths
                .iter()
                .enumerate()
                .map(|(i, &w)| {
                    let cell = row.get(i).map(Vec::as_slice).unwrap_or(&[]);
                    let runs = cell.iter().cloned().map(|mut run| {
                        if r == 0 {
                            run.style = run.style.add_modifier(Modifier::BOLD);
                        }
                        run
                    });
                    wrap(runs, w)
                })
                .collect();
            let height = cells.iter().map(Vec::len).max().unwrap_or(0).max(1);
            let src = self.src + r + usize::from(r > 0);
            for k in 0..height {
                let mut line = vec![];
                for (i, &w) in widths.iter().enumerate() {
                    if i > 0 {
                        line.push(bar());
                    }
                    let part = cells[i].get(k).map(Vec::as_slice).unwrap_or(&[]);
                    let align = self.aligns.get(i).copied().unwrap_or(Align::Left);
                    line.extend(pad(part, w, align));
                }
                out.push((line, src));
            }
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

/// One line of a cell padded to `w` columns, with a space either side.
fn pad(part: &[Run], w: usize, align: Align) -> Vec<Run> {
    let left = w.saturating_sub(part.iter().map(Run::width).sum());
    let (before, after) = match align {
        Align::Left => (0, left),
        Align::Right => (left, 0),
        Align::Center => (left / 2, left - left / 2),
    };
    let space = |n: usize| Run::new(" ".repeat(n + 1), theme::fg(BG).bg(BG));
    let mut out = vec![space(before)];
    out.extend(part.iter().cloned());
    out.push(space(after));
    out
}
