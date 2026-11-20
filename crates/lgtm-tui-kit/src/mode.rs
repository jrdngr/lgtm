//! The screens `space m` switches between, each on its own key.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Editor,
    Review,
    Issues,
}

impl Mode {
    pub fn key(self) -> char {
        match self {
            Mode::Editor => 'e',
            Mode::Review => 'r',
            Mode::Issues => 'i',
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Mode::Editor => "editor",
            Mode::Review => "review",
            Mode::Issues => "issues",
        }
    }

    /// The mode in `to` on key `c`.
    pub fn pick(c: char, to: &[Mode]) -> Option<Mode> {
        to.iter().copied().find(|m| m.key() == c)
    }
}

/// The menu after `space m`, for switching to the modes in `to`.
pub fn menu(to: &[Mode]) -> Vec<(String, &'static str)> {
    to.iter()
        .map(|m| (m.key().to_string(), m.label()))
        .collect()
}

/// `space`, then `m` and a mode's key, where a screen has no space menu of its own.
#[derive(Debug, Default)]
pub struct Leader {
    step: Option<Step>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Space,
    Mode,
}

/// What a key did to the leader.
#[derive(Debug, PartialEq, Eq)]
pub enum Press {
    /// Not the leader's: the screen takes it.
    Pass,
    /// The leader took it.
    Held,
    Switch(Mode),
    /// A key after `space` other than `m`, for the screen's own entries.
    Other(char),
}

impl Leader {
    pub fn active(&self) -> bool {
        self.step.is_some()
    }

    /// Takes `key` on a screen that isn't taking text, offering the modes in `to`.
    pub fn key(&mut self, key: &KeyEvent, to: &[Mode]) -> Press {
        let plain = !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);
        let c = match key.code {
            KeyCode::Char(c) if plain => Some(c),
            _ => None,
        };
        match (self.step.take(), c) {
            (None, Some(' ')) => {
                self.step = Some(Step::Space);
                Press::Held
            }
            (None, _) => Press::Pass,
            (Some(Step::Space), Some('m')) => {
                self.step = Some(Step::Mode);
                Press::Held
            }
            (Some(Step::Space), Some(c)) => Press::Other(c),
            (Some(Step::Mode), Some(c)) => Mode::pick(c, to).map_or(Press::Held, Press::Switch),
            (Some(_), None) => Press::Held,
        }
    }

    /// What the keys so far go on to: `extra`, the screen's own entries, and `m` after
    /// `space`, then the modes in `to`.
    pub fn menu(&self, to: &[Mode], extra: &[(char, &'static str)]) -> Vec<(String, &'static str)> {
        match self.step {
            None => vec![],
            Some(Step::Space) => extra
                .iter()
                .chain([&('m', "mode")])
                .map(|(k, l)| (k.to_string(), *l))
                .collect(),
            Some(Step::Mode) => menu(to),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(l: &mut Leader, c: char) -> Press {
        l.key(
            &KeyEvent::from(KeyCode::Char(c)),
            &[Mode::Editor, Mode::Issues],
        )
    }

    #[test]
    fn space_m_and_a_key_switch_to_an_offered_mode() {
        let mut l = Leader::default();
        assert_eq!(press(&mut l, ' '), Press::Held);
        assert_eq!(l.menu(&[Mode::Editor], &[('p', "text")])[1].0, "m");
        assert_eq!(press(&mut l, 'm'), Press::Held);
        assert_eq!(l.menu(&[Mode::Editor], &[]), menu(&[Mode::Editor]));
        assert_eq!(press(&mut l, 'i'), Press::Switch(Mode::Issues));
        assert!(!l.active());
    }

    #[test]
    fn other_keys_end_the_leader_or_pass_through() {
        let mut l = Leader::default();
        assert_eq!(press(&mut l, 'j'), Press::Pass);
        press(&mut l, ' ');
        assert_eq!(press(&mut l, 'p'), Press::Other('p'));
        press(&mut l, ' ');
        press(&mut l, 'm');
        assert_eq!(press(&mut l, 'r'), Press::Held);
        assert_eq!(press(&mut l, 'j'), Press::Pass);
    }

    #[test]
    fn only_offered_modes_are_picked() {
        assert_eq!(
            Mode::pick('r', &[Mode::Editor, Mode::Review]),
            Some(Mode::Review)
        );
        assert_eq!(Mode::pick('h', &[Mode::Editor, Mode::Review]), None);
    }
}
