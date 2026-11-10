//! Drawing a frame: backgrounds, selection, text, cursor.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    App, Bounds, Font, FontStyle, FontWeight, Hsla, Pixels, Point, ShapedLine, Size,
    StrikethroughStyle, TextAlign, TextRun, UnderlineStyle, Window, fill, outline, point, px, rgb,
    size,
};

use super::frame::{CursorShape, Frame, Row, Run};
use crate::theme;

pub struct Grid {
    pub origin: Point<Pixels>,
    pub cell: Size<Pixels>,
    pub font: Font,
    pub font_size: Pixels,
    pub focused: bool,
    pub lines: LineCache,
}

/// Each row's runs as shaped for the last frame, kept while the row and font size stay the
/// same.
#[derive(Clone, Default)]
pub struct LineCache(Rc<RefCell<Vec<Shaped>>>);

#[derive(Default)]
struct Shaped {
    version: u64,
    font_size: Pixels,
    /// One per run, none for a run with nothing to draw.
    runs: Vec<Option<ShapedLine>>,
}

/// The cell size for `font`: the width of `M` by the line height.
pub fn cell_size(font: &Font, font_size: Pixels, window: &Window) -> Size<Pixels> {
    let ts = window.text_system();
    let id = ts.resolve_font(font);
    let width = ts
        .advance(id, font_size, 'M')
        .map(|s| s.width)
        .unwrap_or(font_size * 0.6);
    let height = (ts.ascent(id, font_size) + ts.descent(id, font_size).abs()) * 1.2;
    size(width, height.ceil())
}

fn color(c: u32) -> Hsla {
    rgb(c).into()
}

impl Grid {
    fn rect(&self, col: u16, row: usize, cells: u16) -> Bounds<Pixels> {
        Bounds {
            origin: point(
                self.origin.x + self.cell.width * col as f32,
                self.origin.y + self.cell.height * row as f32,
            ),
            size: size(self.cell.width * cells as f32, self.cell.height),
        }
    }

    fn text_run(&self, run: &Run, fg: u32, len: usize) -> TextRun {
        let mut c = color(fg);
        if run.faint {
            c.a = 0.6;
        }
        TextRun {
            len,
            font: Font {
                weight: if run.bold {
                    FontWeight::BOLD
                } else {
                    FontWeight::NORMAL
                },
                style: if run.italic {
                    FontStyle::Italic
                } else {
                    FontStyle::Normal
                },
                ..self.font.clone()
            },
            color: c,
            background_color: None,
            underline: run.underline.then(|| UnderlineStyle {
                thickness: px(1.),
                color: Some(c),
                wavy: false,
            }),
            strikethrough: run.strike.then(|| StrikethroughStyle {
                thickness: px(1.),
                color: Some(c),
            }),
        }
    }

    fn shape(&self, run: &Run, text: &str, fg: u32, w: &Window) -> ShapedLine {
        let force = (!run.single).then_some(self.cell.width);
        w.text_system().shape_line(
            text.to_owned().into(),
            self.font_size,
            &[self.text_run(run, fg, text.len())],
            force,
        )
    }

    fn text(
        &self,
        run: &Run,
        text: &str,
        fg: u32,
        at: Bounds<Pixels>,
        w: &mut Window,
        cx: &mut App,
    ) {
        let line = self.shape(run, text, fg, w);
        let _ = line.paint(at.origin, self.cell.height, TextAlign::Left, None, w, cx);
    }

    /// A row's runs shaped, each with nothing for a run that draws nothing.
    fn shape_row(&self, row: &Row, w: &Window) -> Vec<Option<ShapedLine>> {
        row.runs
            .iter()
            .map(|run| {
                let trimmed = run.text.trim_end_matches(' ');
                let lines = run.underline || run.strike;
                if trimmed.is_empty() && !lines {
                    return None;
                }
                let text = if lines { run.text.as_str() } else { trimmed };
                Some(self.shape(run, text, run.fg, w))
            })
            .collect()
    }

    pub fn paint(&self, frame: &Frame, window: &mut Window, cx: &mut App) {
        let mut cache = self.lines.0.borrow_mut();
        cache.resize_with(frame.rows.len(), Shaped::default);
        for (y, row) in frame.rows.iter().enumerate() {
            for run in &row.runs {
                if let Some(bg) = run.bg.filter(|bg| *bg != frame.bg) {
                    window.paint_quad(fill(self.rect(run.col, y, run.cells), color(bg)));
                }
            }
            if let Some((start, end)) = row.selection {
                let r = self.rect(start, y, end + 1 - start);
                window.paint_quad(fill(r, color(theme::SELECTION)));
            }
            let shaped = &mut cache[y];
            if shaped.version != row.version
                || shaped.font_size != self.font_size
                || shaped.runs.len() != row.runs.len()
            {
                *shaped = Shaped {
                    version: row.version,
                    font_size: self.font_size,
                    runs: self.shape_row(row, window),
                };
            }
            for (run, line) in row.runs.iter().zip(&shaped.runs) {
                if let Some(line) = line {
                    let at = self.rect(run.col, y, run.cells).origin;
                    let _ = line.paint(at, self.cell.height, TextAlign::Left, None, window, cx);
                }
            }
        }
        drop(cache);
        self.paint_cursor(frame, window, cx);
    }

    fn paint_cursor(&self, frame: &Frame, window: &mut Window, cx: &mut App) {
        let Some(c) = &frame.cursor else { return };
        let row = frame.rows.get(c.y as usize);
        let run = row.and_then(|r| {
            r.runs
                .iter()
                .find(|r| r.col <= c.x && c.x < r.col + r.cells)
        });
        let cells = run.filter(|r| r.single).map_or(1, |r| r.cells);
        let cell = self.rect(c.x, c.y as usize, cells);
        let color = color(c.color);
        if !self.focused {
            window.paint_quad(outline(cell, color, gpui::BorderStyle::Solid));
            return;
        }
        match c.shape {
            CursorShape::Block => {
                window.paint_quad(fill(cell, color));
                if let (Some(row), Some(run)) = (row, run)
                    && let Some(text) = row.text_at(c.x).filter(|t| *t != " ")
                {
                    let at = self.rect(if run.single { run.col } else { c.x }, c.y as usize, cells);
                    let single = Run {
                        single: true,
                        ..run.clone()
                    };
                    self.text(&single, text, frame.bg, at, window, cx);
                }
            }
            CursorShape::Bar => {
                window.paint_quad(fill(
                    Bounds {
                        size: size(px(2.), cell.size.height),
                        ..cell
                    },
                    color,
                ));
            }
            CursorShape::Underline => {
                let origin = point(cell.origin.x, cell.origin.y + cell.size.height - px(2.));
                window.paint_quad(fill(
                    Bounds {
                        origin,
                        size: size(cell.size.width, px(2.)),
                    },
                    color,
                ));
            }
        }
    }
}
