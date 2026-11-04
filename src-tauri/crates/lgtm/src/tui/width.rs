//! Pane widths that `<` and `>` move: the divider goes a step left or right.

use ratatui::crossterm::event::{KeyCode, KeyEvent};

/// Columns a press moves a divider.
const STEP: i32 = 4;

/// How far `key` moves a divider: left for `<`, right for `>`.
pub fn step(key: &KeyEvent) -> Option<i32> {
    match key.code {
        KeyCode::Char('<') => Some(-STEP),
        KeyCode::Char('>') => Some(STEP),
        _ => None,
    }
}

/// The width a pane drawn `drawn` wide takes after a step, never under `min`.
pub fn moved(drawn: u16, step: i32, min: u16) -> u16 {
    (i32::from(drawn) + step).clamp(i32::from(min), i32::from(u16::MAX)) as u16
}

/// The width to draw: the chosen one, else `default`, kept to `min..=max` (`min` wins when
/// they cross).
pub fn fit(chosen: Option<u16>, default: u16, min: u16, max: u16) -> u16 {
    chosen.unwrap_or(default).min(max).max(min)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_starts_from_the_drawn_width_so_presses_past_the_edge_come_straight_back() {
        let max = 40;
        let past = moved(fit(Some(38), 30, 16, max), STEP, 16);
        assert_eq!(fit(Some(past), 30, 16, max), max);
        assert_eq!(moved(fit(Some(past), 30, 16, max), -STEP, 16), max - 4);
        assert_eq!(moved(18, -STEP, 16), 16);
    }
}
