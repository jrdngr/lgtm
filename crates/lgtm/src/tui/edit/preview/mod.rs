//! A markdown file rendered, read-only, in place of its text: `space m` goes in and out.

mod render;
mod table;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;

use crate::tui::paint::Canvas;
use crate::tui::theme::BG;
use render::Line;

/// The widest the text gets, for lines that read well.
const MAX_WIDTH: usize = 100;

pub struct Preview {
    lines: Vec<Line>,
    /// The buffer version and width the lines are for.
    made: Option<(u64, usize)>,
    top: usize,
    /// The source line to open at, until the first draw finds its row.
    open_at: Option<usize>,
    height: usize,
    /// The first key of a two-key binding: `g` or `space`.
    pending: Option<char>,
    /// Where each link drawn last is: row, columns, and URL.
    links: Vec<(u16, std::ops::Range<u16>, String)>,
}

/// What a key asks of the editor.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    None,
    /// Back to the text.
    Close,
    Command,
}

impl Preview {
    /// A preview that opens where source line `line` is.
    pub fn new(line: usize) -> Self {
        Self {
            lines: vec![],
            made: None,
            top: 0,
            open_at: Some(line),
            height: 0,
            pending: None,
            links: vec![],
        }
    }

    /// The source line at the top of the view.
    pub fn src_line(&self) -> usize {
        self.lines.get(self.top).map_or(0, |l| l.src)
    }

    pub fn scroll(&mut self, by: isize) {
        let most = self.lines.len().saturating_sub(self.height);
        self.top = self.top.saturating_add_signed(by).min(most);
    }

    pub fn key(&mut self, key: KeyEvent) -> Outcome {
        let half = (self.height / 2).max(1) as isize;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match (self.pending.take(), key.code) {
            (Some('g'), KeyCode::Char('g')) => self.top = 0,
            (Some('g'), KeyCode::Char('e')) => self.scroll(isize::MAX),
            (Some(' '), KeyCode::Char('m')) => return Outcome::Close,
            (Some(_), _) => {}
            (None, KeyCode::Char(c @ ('g' | ' '))) => self.pending = Some(c),
            (None, KeyCode::Char('j') | KeyCode::Down) => self.scroll(1),
            (None, KeyCode::Char('k') | KeyCode::Up) => self.scroll(-1),
            (None, KeyCode::Char('d')) if ctrl => self.scroll(half),
            (None, KeyCode::Char('u')) if ctrl => self.scroll(-half),
            (None, KeyCode::PageDown) => self.scroll(half * 2),
            (None, KeyCode::PageUp) => self.scroll(-half * 2),
            (None, KeyCode::Esc) => return Outcome::Close,
            (None, KeyCode::Char(':')) => return Outcome::Command,
            _ => {}
        }
        Outcome::None
    }

    /// The URL of the link at a screen cell.
    pub fn link_at(&self, x: u16, y: u16) -> Option<&str> {
        self.links
            .iter()
            .find(|(row, cols, _)| *row == y && cols.contains(&x))
            .map(|(_, _, url)| url.as_str())
    }

    /// Draws `text`, version `version`, over `area`.
    pub fn draw(&mut self, c: &mut Canvas, area: Rect, text: &str, version: u64) {
        let width = (area.width.saturating_sub(4) as usize).min(MAX_WIDTH);
        if self.made != Some((version, width)) {
            let src = self.made.map(|_| self.src_line());
            self.lines = render::render(text, width);
            self.made = Some((version, width));
            if let Some(line) = self.open_at.take().or(src) {
                self.top = self.lines.partition_point(|l| l.src < line);
            }
        }
        self.height = area.height as usize;
        self.scroll(0);
        self.links.clear();
        let x0 = area.x + 2;
        for (i, line) in self.lines.iter().skip(self.top).enumerate() {
            let y = area.y + i as u16;
            if y >= area.bottom() {
                break;
            }
            c.fill(area.x, y, area.width, BG);
            if let Some(fill) = line.fill {
                c.fill(
                    x0,
                    y,
                    (width as u16).min(area.width.saturating_sub(2)),
                    fill,
                );
            }
            let mut x = x0;
            for run in &line.runs {
                let end = c.put(x, y, &run.text, run.style);
                if let Some(url) = &run.link {
                    self.links.push((y, x..end, url.clone()));
                }
                x = end;
            }
        }
        let drawn = self.lines.len().saturating_sub(self.top) as u16;
        for y in area.y + drawn.min(area.height)..area.bottom() {
            c.fill(area.x, y, area.width, BG);
        }
    }
}

#[cfg(test)]
mod tests;
