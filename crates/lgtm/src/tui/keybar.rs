//! The key hints along the bottom, and the menus over them.

use ratatui::layout::Rect;
use ratatui::style::Color;

use super::paint::Canvas;
use super::theme::{self, ACCENT, BG, FG, GREEN, HAIR, MUTED, PANEL};

/// A hint: its keys, space-separated when a pair goes both ways, its label, and whether what
/// it toggles is on.
pub type Hint<'a> = (&'a str, &'a str, bool);

/// The key bar after a badge naming the mode, like `NOR`, in its color.
pub fn draw_badged(c: &mut Canvas, y: u16, badge: Option<(&str, Color)>, hints: &[Hint]) {
    c.fill(c.area.x, y, c.area.width, PANEL);
    let mut x = c.area.x + 1;
    if let Some((label, color)) = badge {
        x = c.put(x, y, &format!(" {label} "), theme::bold(BG).bg(color)) + 2;
    }
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

/// A line being typed, with what Tab offers after it.
pub fn prompt(c: &mut Canvas, y: u16, text: &str, offers: &[String]) {
    c.fill(c.area.x, y, c.area.width, PANEL);
    let x = c.put(c.area.x + 1, y, text, theme::fg(FG).bg(PANEL));
    let mut x = c.put(x, y, " ", theme::fg(FG).bg(ACCENT)) + 2;
    for offer in offers {
        x = c.put(x, y, offer, theme::fg(MUTED).bg(PANEL)) + 2;
    }
}

/// A box at the bottom left of `area` listing `items`' keys beside what they do.
fn boxed(c: &mut Canvas, area: Rect, items: &[(&str, &str)]) {
    let kw = items
        .iter()
        .map(|(k, _)| k.chars().count())
        .max()
        .unwrap_or(0) as u16;
    let lw = items
        .iter()
        .map(|(_, l)| l.chars().count())
        .max()
        .unwrap_or(0) as u16;
    let w = (kw + lw + 7).min(area.width);
    let h = (items.len() as u16 + 2).min(area.height);
    let (x0, y0) = (area.x, area.bottom() - h);
    let border = theme::fg(HAIR).bg(PANEL);
    for y in y0..y0 + h {
        c.fill(x0, y, w, PANEL);
        c.put(x0, y, "│", border);
        c.put(x0 + w - 1, y, "│", border);
    }
    let rule = "─".repeat(w.saturating_sub(2) as usize);
    c.put(x0, y0, &format!("╭{rule}╮"), border);
    c.put(x0, y0 + h - 1, &format!("╰{rule}╯"), border);
    for (i, (k, l)) in items.iter().enumerate().take(h.saturating_sub(2) as usize) {
        let y = y0 + 1 + i as u16;
        c.put(x0 + 2, y, k, theme::bold(ACCENT).bg(PANEL));
        c.put(x0 + 4 + kw, y, l, theme::fg(FG).bg(PANEL));
    }
}

/// The commands that match what's typed, above the command line.
pub fn menu(c: &mut Canvas, area: Rect, items: &[(String, &str)]) {
    if items.is_empty() {
        return;
    }
    let items: Vec<(&str, &str)> = items.iter().map(|(k, l)| (k.as_str(), *l)).collect();
    boxed(c, area, &items);
}
