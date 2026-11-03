//! The key hints along the bottom, and the `?` list of every key.

use ratatui::layout::Rect;

use super::paint::Canvas;
use super::theme::{self, ACCENT, FG, GREEN, HAIR, PANEL};

/// A hint: its keys, space-separated when a pair goes both ways, its label, and whether what
/// it toggles is on.
pub type Hint<'a> = (&'a str, &'a str, bool);

pub fn draw(c: &mut Canvas, y: u16, hints: &[Hint]) {
    c.fill(c.area.x, y, c.area.width, PANEL);
    let mut x = c.area.x + 1;
    for (keys, label, on) in hints {
        for (i, key) in keys.split(' ').enumerate() {
            if i > 0 {
                x += 1;
            }
            x = c.put(x, y, key, theme::bold(ACCENT).bg(PANEL));
        }
        x = c.put(
            x + 1,
            y,
            label,
            theme::fg(if *on { GREEN } else { FG }).bg(PANEL),
        );
        x += 3;
    }
}

/// A box over the middle of `area` listing `keys` beside what they do.
pub fn help(c: &mut Canvas, area: Rect, keys: &[(&str, &str)]) {
    let kw = keys
        .iter()
        .map(|(k, _)| k.chars().count())
        .max()
        .unwrap_or(0) as u16;
    let lw = keys
        .iter()
        .map(|(_, l)| l.chars().count())
        .max()
        .unwrap_or(0) as u16;
    let w = (kw + lw + 7).min(area.width);
    let h = (keys.len() as u16 + 2).min(area.height);
    let x0 = area.x + (area.width - w) / 2;
    let y0 = area.y + (area.height - h) / 2;
    let border = theme::fg(HAIR).bg(PANEL);
    for y in y0..y0 + h {
        c.fill(x0, y, w, PANEL);
        c.put(x0, y, "│", border);
        c.put(x0 + w - 1, y, "│", border);
    }
    let rule = "─".repeat(w.saturating_sub(2) as usize);
    c.put(x0, y0, &format!("╭{rule}╮"), border);
    c.put(x0, y0 + h - 1, &format!("╰{rule}╯"), border);
    for (i, (k, l)) in keys.iter().enumerate().take(h.saturating_sub(2) as usize) {
        let y = y0 + 1 + i as u16;
        c.put(x0 + 2, y, k, theme::bold(ACCENT).bg(PANEL));
        c.put(x0 + 4 + kw, y, l, theme::fg(FG).bg(PANEL));
    }
}
