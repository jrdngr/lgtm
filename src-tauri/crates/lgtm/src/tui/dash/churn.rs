//! The churn chart in block characters: a bar per edit, added lines up in green with the
//! ones that rewrote the turn's own lines in amber, removed lines down in red.

use lgtm_core::store::TurnEdit;
use ratatui::layout::Rect;

use crate::tui::paint::Canvas;
use crate::tui::theme::{self, AMBER, BG, GREEN, HAIR, RED};

/// Columns a bar takes, and the gap after it.
const BAR: u16 = 2;
const SLOT: u16 = BAR + 1;
/// Bars always leave room for at least this many lines, so a few don't fill the chart.
const MIN_SCALE: u32 = 10;

const UP: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

/// The cell `row` cells above the baseline of a bar `eighths` tall, if it reaches it.
fn up_cell(eighths: u32, row: u32) -> Option<&'static str> {
    let fill = eighths.saturating_sub(row * 8).min(8);
    (fill > 0).then(|| UP[fill as usize - 1])
}

/// The cell `row` cells below the baseline of a bar `eighths` deep. Only a few eighths have a
/// block that hangs from the top.
fn down_cell(eighths: u32, row: u32) -> Option<&'static str> {
    match eighths.saturating_sub(row * 8).min(8) {
        0 => None,
        1..=2 => Some("▔"),
        3..=5 => Some("▀"),
        _ => Some("█"),
    }
}

/// Draws the newest edits that fit in `area`, the totals on its last row.
pub fn draw(c: &mut Canvas, area: Rect, edits: &[TurnEdit]) {
    if area.height < 4 || area.width < SLOT {
        return;
    }
    let chart = area.height - 2;
    let up = (u32::from(chart) * 5 / 8).max(1);
    let down = u32::from(chart) - up - 1;
    let base = area.y + up as u16;
    let slots = (area.width / SLOT) as usize;
    let shown = &edits[edits.len().saturating_sub(slots)..];
    let scale = shown
        .iter()
        .flat_map(|e| [e.added, e.removed])
        .max()
        .unwrap_or(0)
        .max(MIN_SCALE);
    let eighths = |n: u32, rows: u32| (n * rows * 8).div_ceil(scale);
    for i in 0..slots {
        let x = area.x + i as u16 * SLOT;
        c.put(x, base, &"─".repeat(BAR.into()), theme::fg(HAIR).bg(BG));
        let Some(e) = shown.get(i) else { continue };
        let (added, redo) = (eighths(e.added, up), eighths(e.rewritten.min(e.added), up));
        for row in 0..up {
            if let Some(cell) = up_cell(added, row) {
                let color = if row * 8 < redo { AMBER } else { GREEN };
                c.put(
                    x,
                    base - 1 - row as u16,
                    &cell.repeat(BAR.into()),
                    theme::fg(color).bg(BG),
                );
            }
        }
        let removed = eighths(e.removed, down);
        for row in 0..down {
            if let Some(cell) = down_cell(removed, row) {
                c.put(
                    x,
                    base + 1 + row as u16,
                    &cell.repeat(BAR.into()),
                    theme::fg(RED).bg(BG),
                );
            }
        }
    }
    let (a, d, r) = edits.iter().fold((0, 0, 0), |(a, d, r), e| {
        (a + e.added, d + e.removed, r + e.rewritten)
    });
    let y = area.bottom() - 1;
    let x = c.put(area.x, y, &format!("+{a}"), theme::fg(GREEN).bg(BG));
    let x = c.put(x + 1, y, &format!("−{d}"), theme::fg(RED).bg(BG));
    c.put(x + 1, y, &format!("{r} rewritten"), theme::fg(AMBER).bg(BG));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bar_fills_whole_cells_from_the_baseline_then_tops_off_with_a_partial_one() {
        let cells: Vec<_> = (0..3).map(|row| up_cell(12, row)).collect();
        assert_eq!(cells, [Some("█"), Some("▄"), None]);
    }

    #[test]
    fn a_removed_bar_hangs_from_the_baseline() {
        let cells: Vec<_> = (0..3).map(|row| down_cell(12, row)).collect();
        assert_eq!(cells, [Some("█"), Some("▀"), None]);
    }
}
