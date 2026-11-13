//! Pane widths that `C-w <` and `C-w >` move: the divider goes a step left or right.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Columns a press moves a divider.
pub const STEP: i32 = 4;

/// Reads `C-w <` and `C-w >`. `pending` holds a `C-w` waiting for its second key, which is
/// taken whatever it is. None for a key that isn't part of one; else the step, when the key
/// ends one.
pub fn read(pending: &mut bool, key: &KeyEvent) -> Option<Option<i32>> {
    if std::mem::take(pending) {
        return Some(match key.code {
            KeyCode::Char('<') => Some(-STEP),
            KeyCode::Char('>') => Some(STEP),
            _ => None,
        });
    }
    if key.code == KeyCode::Char('w') && key.modifiers.contains(KeyModifiers::CONTROL) {
        *pending = true;
        return Some(None);
    }
    None
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
    fn a_divider_moves_on_ctrl_w_then_an_angle_bracket_and_a_bare_one_is_left_alone() {
        let key = |c, m| KeyEvent::new(KeyCode::Char(c), m);
        let (none, ctrl) = (KeyModifiers::NONE, KeyModifiers::CONTROL);
        let mut pending = false;
        assert_eq!(read(&mut pending, &key('>', none)), None);
        assert_eq!(read(&mut pending, &key('w', ctrl)), Some(None));
        assert_eq!(read(&mut pending, &key('>', none)), Some(Some(STEP)));
        assert_eq!(read(&mut pending, &key('<', none)), None);
        read(&mut pending, &key('w', ctrl));
        assert_eq!(read(&mut pending, &key('j', none)), Some(None));
        assert!(!pending);
    }

    #[test]
    fn a_step_starts_from_the_drawn_width_so_presses_past_the_edge_come_straight_back() {
        let max = 40;
        let past = moved(fit(Some(38), 30, 16, max), STEP, 16);
        assert_eq!(fit(Some(past), 30, 16, max), max);
        assert_eq!(moved(fit(Some(past), 30, 16, max), -STEP, 16), max - 4);
        assert_eq!(moved(18, -STEP, 16), 16);
    }
}
