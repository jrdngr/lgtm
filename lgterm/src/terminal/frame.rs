//! The visible screen as runs of same-styled cells, rebuilt only for rows that changed.

use std::rc::Rc;

use anyhow::Result;
use ghostty_vt::render::{CellIterator, CursorVisualStyle, Dirty, RowIterator};
use ghostty_vt::screen::CellWide;
use ghostty_vt::style::{RgbColor, Underline};
use ghostty_vt::{RenderState, Terminal};

use crate::ui::theme;

/// Cells in one row that share a style. Narrow runs hold one char per cell; a wide character
/// or a multi-codepoint cluster gets a run of its own.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Run {
    pub col: u16,
    pub cells: u16,
    pub text: String,
    pub fg: u32,
    pub bg: Option<u32>,
    pub bold: bool,
    pub italic: bool,
    pub faint: bool,
    pub underline: bool,
    pub strike: bool,
    /// Drawn at its natural width, not squeezed into cells.
    pub single: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Row {
    pub runs: Vec<Run>,
    /// Selected columns, inclusive.
    pub selection: Option<(u16, u16)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorShape {
    Block,
    Bar,
    Underline,
}

#[derive(Clone, Debug)]
pub struct Cursor {
    pub x: u16,
    pub y: u16,
    pub shape: CursorShape,
    pub color: u32,
}

#[derive(Clone)]
pub struct Frame {
    pub rows: Rc<Vec<Row>>,
    pub cursor: Option<Cursor>,
    pub bg: u32,
}

/// Reusable iterators over a terminal's render state.
pub struct Snapshotter {
    state: RenderState,
    rows: RowIterator,
    cells: CellIterator,
    lines: Rc<Vec<Row>>,
}

fn hex(c: RgbColor) -> u32 {
    (c.r as u32) << 16 | (c.g as u32) << 8 | c.b as u32
}

impl Snapshotter {
    pub fn new() -> Result<Self> {
        Ok(Snapshotter {
            state: RenderState::new()?,
            rows: RowIterator::new()?,
            cells: CellIterator::new()?,
            lines: Rc::default(),
        })
    }

    pub fn frame(&mut self, vt: &Terminal) -> Result<Frame> {
        let snap = self.state.update(vt)?;
        let colors = snap.colors()?;
        let (fg, bg) = (hex(colors.foreground), hex(colors.background));
        let height = snap.rows()? as usize;
        let lines = Rc::make_mut(&mut self.lines);
        let mut full = snap.dirty()? == Dirty::Full;
        if lines.len() != height {
            lines.resize_with(height, Row::default);
            full = true;
        }

        let mut rows = self.rows.update(&snap)?;
        let mut y = 0;
        while let Some(row) = rows.next() {
            let Some(line) = lines.get_mut(y) else { break };
            line.selection = row.selection()?.map(|s| (s.start_x, s.end_x));
            if full || row.dirty()? {
                line.runs.clear();
                let mut cells = self.cells.update(row)?;
                let mut x = 0u16;
                let mut text = String::new();
                while let Some(cell) = cells.next() {
                    let col = x;
                    x += 1;
                    let wide = cell.raw_cell()?.wide()?;
                    if matches!(wide, CellWide::SpacerTail | CellWide::SpacerHead) {
                        continue;
                    }
                    let style = cell.style()?;
                    text.clear();
                    cell.graphemes_utf8(&mut text)?;
                    if text.is_empty() || style.invisible {
                        text.clear();
                        text.push(' ');
                    }
                    let mut cell_fg = cell.fg_color()?.map_or(fg, hex);
                    let mut cell_bg = cell.bg_color()?.map(hex);
                    if style.inverse {
                        let back = cell_bg.unwrap_or(bg);
                        cell_bg = Some(cell_fg);
                        cell_fg = back;
                    }
                    let run = Run {
                        col,
                        cells: if wide == CellWide::Wide { 2 } else { 1 },
                        text: String::new(),
                        fg: cell_fg,
                        bg: cell_bg,
                        bold: style.bold,
                        italic: style.italic,
                        faint: style.faint,
                        underline: style.underline != Underline::None,
                        strike: style.strikethrough,
                        single: wide == CellWide::Wide || text.chars().nth(1).is_some(),
                    };
                    match line.runs.last_mut() {
                        Some(last)
                            if !run.single
                                && !last.single
                                && last.col + last.cells == col
                                && same_style(last, &run) =>
                        {
                            last.cells += 1;
                            last.text.push_str(&text);
                        }
                        _ => line.runs.push(Run {
                            text: text.clone(),
                            ..run
                        }),
                    }
                }
                row.set_dirty(false)?;
            }
            y += 1;
        }

        let c = snap.cursor()?;
        let cursor = c.viewport.filter(|_| c.visible).map(|v| Cursor {
            x: v.x,
            y: v.y,
            shape: match c.visual_style {
                CursorVisualStyle::Bar => CursorShape::Bar,
                CursorVisualStyle::Underline => CursorShape::Underline,
                _ => CursorShape::Block,
            },
            color: colors.cursor.map_or(theme::TERM_FG, hex),
        });
        snap.set_dirty(Dirty::Clean)?;
        Ok(Frame {
            rows: self.lines.clone(),
            cursor,
            bg,
        })
    }
}

fn same_style(a: &Run, b: &Run) -> bool {
    (a.fg, a.bg, a.bold, a.italic, a.faint, a.underline, a.strike)
        == (b.fg, b.bg, b.bold, b.italic, b.faint, b.underline, b.strike)
}

impl Row {
    /// The row as text, with gaps between runs filled with spaces.
    pub fn text(&self) -> String {
        let mut out = String::new();
        let mut col = 0;
        for run in &self.runs {
            while col < run.col {
                out.push(' ');
                col += 1;
            }
            out.push_str(&run.text);
            col = run.col + run.cells;
        }
        out.trim_end().to_owned()
    }

    /// The index in `text()` of the char drawn in column `col`.
    pub fn char_index(&self, col: u16) -> Option<usize> {
        let mut idx = 0;
        let mut at = 0;
        for run in &self.runs {
            if col < run.col {
                return Some(idx + (col - at) as usize);
            }
            idx += (run.col - at) as usize;
            if col < run.col + run.cells {
                let offset = if run.single { 0 } else { col - run.col };
                return Some(idx + offset as usize);
            }
            idx += run.text.chars().count();
            at = run.col + run.cells;
        }
        None
    }

    /// The text in column `col`, if a run starts or covers it.
    pub fn text_at(&self, col: u16) -> Option<&str> {
        let run = self
            .runs
            .iter()
            .find(|r| r.col <= col && col < r.col + r.cells)?;
        if run.single {
            return Some(&run.text);
        }
        let start = run.text.char_indices().nth((col - run.col) as usize)?.0;
        let len = run.text[start..].chars().next()?.len_utf8();
        Some(&run.text[start..start + len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ghostty_vt::TerminalOptions;

    fn frame(input: &[u8]) -> Frame {
        let mut vt = Terminal::new(TerminalOptions {
            cols: 10,
            rows: 3,
            max_scrollback: 0,
        })
        .unwrap();
        vt.vt_write(input);
        Snapshotter::new().unwrap().frame(&vt).unwrap()
    }

    #[test]
    fn rows_read_back_as_text() {
        let f = frame(b"ab \x1b[1mcd\x1b[0m  e");
        assert_eq!(f.rows[0].text(), "ab cd  e");
    }

    #[test]
    fn cells_with_the_same_style_merge_into_one_run() {
        let f = frame(b"ab\x1b[31mcd\x1b[0me");
        let runs = &f.rows[0].runs;
        assert_eq!(runs[0].text, "ab");
        assert_eq!((runs[1].col, runs[1].text.as_str()), (2, "cd"));
        assert_ne!(runs[1].fg, runs[0].fg);
        assert_eq!(runs[2].text, "e     ");
        assert_eq!(f.rows[0].text_at(3), Some("d"));
    }

    #[test]
    fn wide_characters_take_two_cells_in_a_run_of_their_own() {
        let f = frame("a中b".as_bytes());
        let runs = &f.rows[0].runs;
        assert_eq!((runs[1].col, runs[1].cells, runs[1].single), (1, 2, true));
        assert_eq!((runs[2].col, runs[2].text.chars().next()), (3, Some('b')));
        assert_eq!(f.rows[0].text_at(2), Some("中"));
    }

    #[test]
    fn columns_map_to_chars_past_wide_cells_and_clusters() {
        let char_at = |input: &str, col| {
            let row = &frame(input.as_bytes()).rows[0];
            row.text().chars().nth(row.char_index(col)?)
        };
        assert_eq!(char_at("中中 ab", 1), Some('中'));
        assert_eq!(char_at("中中 ab", 5), Some('a'));
        assert_eq!(char_at("e\u{301} ab", 2), Some('a'));
        assert_eq!(char_at("e\u{301} ab", 3), Some('b'));
    }

    #[test]
    fn inverse_swaps_colors_and_the_cursor_follows_output() {
        let f = frame(b"\x1b[7mx\x1b[0m\r\nhi");
        let run = &f.rows[0].runs[0];
        assert_eq!(run.fg, f.bg);
        assert!(run.bg.is_some_and(|bg| bg != f.bg));
        let c = f.cursor.unwrap();
        assert_eq!((c.x, c.y), (2, 1));
    }
}
