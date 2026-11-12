//! A line being typed: its text and where the cursor is.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Clone, Debug, Default)]
pub struct Input {
    pub text: String,
    /// The cursor, in chars.
    pub at: usize,
}

impl Input {
    fn byte(&self, at: usize) -> usize {
        self.text
            .char_indices()
            .nth(at)
            .map_or(self.text.len(), |(i, _)| i)
    }

    fn len(&self) -> usize {
        self.text.chars().count()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn insert(&mut self, s: &str) {
        let i = self.byte(self.at);
        let s: String = s.chars().map(|c| if c == '\n' { ' ' } else { c }).collect();
        self.text.insert_str(i, &s);
        self.at += s.chars().count();
    }

    /// The start of the word before the cursor.
    fn word_start(&self) -> usize {
        let chars: Vec<char> = self.text.chars().collect();
        let mut i = self.at;
        while i > 0 && chars[i - 1] == ' ' {
            i -= 1;
        }
        while i > 0 && chars[i - 1] != ' ' {
            i -= 1;
        }
        i
    }

    fn delete(&mut self, from: usize, to: usize) {
        let (a, b) = (self.byte(from), self.byte(to));
        self.text.replace_range(a..b, "");
        self.at = from;
    }

    /// Edits for `key`. Returns whether it was an editing key.
    pub fn key(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            KeyCode::Char('u') if ctrl => self.delete(0, self.at),
            KeyCode::Char('w') if ctrl => self.delete(self.word_start(), self.at),
            KeyCode::Backspace if alt => self.delete(self.word_start(), self.at),
            KeyCode::Char('a') if ctrl => self.at = 0,
            KeyCode::Char('e') if ctrl => self.at = self.len(),
            KeyCode::Char(c) if !ctrl && !alt => self.insert(&c.to_string()),
            KeyCode::Backspace if self.at > 0 => self.delete(self.at - 1, self.at),
            KeyCode::Delete if self.at < self.len() => self.delete(self.at, self.at + 1),
            KeyCode::Left => self.at = self.at.saturating_sub(1),
            KeyCode::Right => self.at = (self.at + 1).min(self.len()),
            KeyCode::Home => self.at = 0,
            KeyCode::End => self.at = self.len(),
            KeyCode::Backspace | KeyCode::Delete => {}
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(input: &mut Input, code: KeyCode, mods: KeyModifiers) {
        input.key(KeyEvent::new(code, mods));
    }

    #[test]
    fn typing_goes_in_at_the_cursor_and_words_delete_back() {
        let mut input = Input::default();
        input.insert("keep it short");
        press(&mut input, KeyCode::Left, KeyModifiers::NONE);
        press(&mut input, KeyCode::Left, KeyModifiers::NONE);
        press(&mut input, KeyCode::Char('é'), KeyModifiers::NONE);
        assert_eq!(input.text, "keep it shoért");
        press(&mut input, KeyCode::Char('w'), KeyModifiers::CONTROL);
        assert_eq!((input.text.as_str(), input.at), ("keep it rt", 8));
        press(&mut input, KeyCode::Char('u'), KeyModifiers::CONTROL);
        assert_eq!((input.text.as_str(), input.at), ("rt", 0));
    }

    #[test]
    fn pasted_lines_join_into_one() {
        let mut input = Input::default();
        input.insert("a\nb");
        assert_eq!(input.text, "a b");
    }
}
