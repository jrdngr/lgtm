//! Pane widths that `C-w <` and `C-w >` move: the divider goes a step left or right, and
//! more presses of `<` or `>` keep moving it until another key.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Columns a press moves a divider.
pub const STEP: i32 = 4;

/// Where a divider move stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Divider {
    #[default]
    Off,
    /// `C-w` was pressed: the next key is taken, whatever it is.
    Prefix,
    /// The divider moved: `<` and `>` move it again.
    Moving,
}

/// The step `<` or `>` moves a divider.
pub fn angle(key: &KeyEvent) -> Option<i32> {
    match key.code {
        KeyCode::Char('<') => Some(-STEP),
        KeyCode::Char('>') => Some(STEP),
        _ => None,
    }
}

/// Reads `C-w <` and `C-w >` and the `<` and `>` presses after them. None for a key that
/// isn't part of a move; else the step, when the key makes one.
pub fn read(state: &mut Divider, key: &KeyEvent) -> Option<Option<i32>> {
    match std::mem::take(state) {
        Divider::Prefix => {
            let step = angle(key);
            if step.is_some() {
                *state = Divider::Moving;
            }
            return Some(step);
        }
        Divider::Moving if angle(key).is_some() => {
            *state = Divider::Moving;
            return Some(angle(key));
        }
        _ => {}
    }
    if key.code == KeyCode::Char('w') && key.modifiers.contains(KeyModifiers::CONTROL) {
        *state = Divider::Prefix;
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
    fn ctrl_w_then_angle_brackets_move_a_divider_until_another_key() {
        let key = |c, m| KeyEvent::new(KeyCode::Char(c), m);
        let (none, ctrl) = (KeyModifiers::NONE, KeyModifiers::CONTROL);
        let mut state = Divider::Off;
        assert_eq!(read(&mut state, &key('>', none)), None);
        assert_eq!(read(&mut state, &key('w', ctrl)), Some(None));
        assert_eq!(read(&mut state, &key('>', none)), Some(Some(STEP)));
        assert_eq!(read(&mut state, &key('>', none)), Some(Some(STEP)));
        assert_eq!(read(&mut state, &key('<', none)), Some(Some(-STEP)));
        assert_eq!(read(&mut state, &key('j', none)), None);
        assert_eq!(read(&mut state, &key('<', none)), None);
        read(&mut state, &key('w', ctrl));
        assert_eq!(read(&mut state, &key('j', none)), Some(None));
        assert_eq!(state, Divider::Off);
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
