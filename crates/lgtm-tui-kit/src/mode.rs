//! The screens `space m` switches between, each on its own key.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_offered_modes_are_picked() {
        assert_eq!(
            Mode::pick('r', &[Mode::Editor, Mode::Review]),
            Some(Mode::Review)
        );
        assert_eq!(Mode::pick('h', &[Mode::Editor, Mode::Review]), None);
    }
}
