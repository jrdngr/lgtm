//! The screen's frame: the top bar with `lgtm` and a title, and room for the key bar.

use lgtm_tui_kit::flow::{self, Piece};
use ratatui::layout::Rect;

use super::paint::Canvas;
use super::theme::{self, ACCENT, BG, PANEL};
use super::{edit, help, keybar, mention};

/// Paints the background and the top bar with `lgtm`, then `left`, on it, and `right` on its
/// right, wrapping onto more rows when they don't fit. Returns the body between it and a key
/// bar `foot` rows tall, when there's room.
pub(super) fn frame(c: &mut Canvas, left: Vec<Piece>, right: &[Piece], foot: u16) -> Option<Rect> {
    let area = c.area;
    let mut pieces = vec![Piece::new(0, "lgtm", theme::bold(ACCENT).bg(PANEL))];
    pieces.extend(left);
    let width = area.width.saturating_sub(2);
    let head = flow::rows(width, &pieces, right);
    if area.height < head + foot + 1 {
        return None;
    }
    for y in area.top()..area.bottom() {
        c.fill(area.x, y, area.width, BG);
    }
    for y in area.y..area.y + head {
        c.fill(area.x, y, area.width, PANEL);
    }
    flow::draw(c, (area.x + 1, area.y), width, &pieces, right);
    Some(Rect::new(
        area.x,
        area.y + head,
        area.width,
        area.height - head - foot,
    ))
}

/// Draws the editor screen, with the keys over it when `help` is up.
pub(super) fn draw_editor(
    c: &mut Canvas,
    e: &mut edit::Editor,
    help: Option<&mut help::Help>,
    pick: Option<&mention::Pick>,
) {
    let bottom = c.area.bottom().saturating_sub(1);
    let prompt = e.prompt();
    let badge = Some(e.badge());
    let hints = e.hints();
    let foot = match prompt {
        Some(_) => 1,
        None => keybar::rows(c.area.width, badge, &hints),
    };
    let (left, right) = e.title();
    let Some(body) = frame(c, left, &right, foot) else {
        return;
    };
    e.draw(c, body);
    if let Some(text) = prompt {
        keybar::prompt(c, bottom, &text, &[]);
        keybar::menu(c, body, &e.command_menu());
        return;
    }
    keybar::draw_badged(c, bottom, badge, &hints);
    let pending = e.pending_menu();
    if !pending.is_empty() {
        keybar::menu(c, body, &pending);
    }
    if let Some(pick) = pick {
        keybar::menu(c, body, &pick.menu());
    }
    if let Some(h) = help {
        h.draw(c, body, &e.help_groups());
    }
}
