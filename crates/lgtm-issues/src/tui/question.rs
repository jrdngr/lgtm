//! A question with the whole screen, and the answer line under it.

use lgtm_tui_kit::paint::Canvas;
use lgtm_tui_kit::theme::{self, ACCENT, BG, FAINT, FG, FUCHSIA, SEL};
use ratatui::layout::Rect;

use super::input::Input;
use super::qq;
use super::view::header;
use crate::issue::{Entry, Issue};

/// An answer being written: the option picked, if any, and the text.
#[derive(Clone, Debug, Default)]
pub struct Draft {
    pub pick: Option<usize>,
    pub input: Input,
}

const INDENT: u16 = 3;

pub fn draw(c: &mut Canvas, area: Rect, issue: &Issue, q: &Entry, draft: &Draft, top: &mut usize) {
    header(c, area.x, area.y, issue);
    let width = area.width.saturating_sub(INDENT * 2) as usize;
    let lines = qq::render(&q.text, width);
    let body = Rect {
        y: area.y + 2,
        height: area.height.saturating_sub(4),
        ..area
    };
    let most = lines.len().saturating_sub(body.height as usize);
    *top = (*top).min(most);
    for (i, line) in lines
        .iter()
        .skip(*top)
        .take(body.height as usize)
        .enumerate()
    {
        let y = body.y + i as u16;
        let picked = line.option.is_some() && line.option == draft.pick;
        let bg = if picked { SEL } else { BG };
        c.fill(area.x, y, area.width, bg);
        let mut x = area.x + INDENT;
        for (text, style) in &line.runs {
            x = c.put(x, y, text, style.bg(bg));
        }
    }
    answer_line(
        c,
        area.x,
        area.bottom().saturating_sub(1),
        area.width,
        draft,
    );
}

fn answer_line(c: &mut Canvas, x0: u16, y: u16, width: u16, draft: &Draft) {
    c.fill(x0, y, width, BG);
    let mut x = c.put(x0 + 1, y, "› ", theme::bold(ACCENT));
    if let Some(n) = draft.pick {
        x = c.put(x, y, &format!(" {n} "), theme::bold(BG).bg(ACCENT));
        x = c.put(x + 2, y, "notes ", theme::fg(FAINT));
    } else if !draft.input.is_empty() {
        x = c.put(x, y, " reply ", theme::bold(BG).bg(FUCHSIA)) + 2;
    }
    let room = (x0 + width).saturating_sub(x + 1) as usize;
    let chars: Vec<char> = draft.input.text.chars().collect();
    let skip = draft.input.at.saturating_sub(room.saturating_sub(1));
    let shown: String = chars.iter().skip(skip).take(room).collect();
    c.put(x, y, &shown, theme::fg(FG));
    c.bar_at(x + (draft.input.at - skip) as u16, y);
}
