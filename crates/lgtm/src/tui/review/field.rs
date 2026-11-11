//! The text of a comment being written, with a cursor: the arrows, Home and End move it, a
//! click puts it where it lands, and Shift+Enter starts a new line.

use std::ops::Range;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::wrap::code_parts;

#[derive(Default)]
pub struct Field {
    pub text: String,
    /// Where the cursor is, in chars.
    pub cursor: usize,
}

/// What a key did to a field.
#[derive(Debug, PartialEq, Eq)]
pub enum Typed {
    Changed,
    Moved,
    /// Not a key the field takes.
    No,
}

impl Field {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            cursor: text.chars().count(),
        }
    }

    /// The screen lines the text takes at `width` columns, as ranges of its chars. A line
    /// that wraps keeps its last space; a line break ends a line without being in it.
    pub fn lines(&self, width: usize) -> Vec<Range<usize>> {
        let mut out = Vec::new();
        let mut start = 0;
        for para in self.text.split('\n') {
            let len = para.chars().count();
            for part in code_parts(para, width) {
                out.push(start + part.start..start + part.end);
            }
            start += len + 1;
        }
        out
    }

    /// The cursor's screen line and column at `width` columns.
    pub fn spot(&self, width: usize) -> (usize, usize) {
        let lines = self.lines(width);
        let line = lines
            .iter()
            .rposition(|l| l.start <= self.cursor)
            .unwrap_or(0);
        (line, self.cursor - lines[line].start)
    }

    /// Puts the cursor on screen line `line` at column `col`, or at the line's end.
    pub fn place(&mut self, width: usize, line: usize, col: usize) {
        let lines = self.lines(width);
        let Some(l) = lines.get(line) else {
            self.cursor = self.text.chars().count();
            return;
        };
        let mut at = (l.start + col).min(l.end);
        // The end of a line that wraps is the start of the next one.
        if at == l.end && at > l.start && lines.get(line + 1).is_some_and(|n| n.start == l.end) {
            at -= 1;
        }
        self.cursor = at;
    }

    fn byte(&self, char_at: usize) -> usize {
        self.text
            .char_indices()
            .nth(char_at)
            .map_or(self.text.len(), |(b, _)| b)
    }

    fn insert(&mut self, c: char) {
        let at = self.byte(self.cursor);
        self.text.insert(at, c);
        self.cursor += 1;
    }

    /// Handles `key` with the text wrapped at `width` columns.
    pub fn key(&mut self, key: KeyEvent, width: usize) -> Typed {
        let len = self.text.chars().count();
        let (line, col) = self.spot(width);
        let newline = key
            .modifiers
            .intersects(KeyModifiers::SHIFT | KeyModifiers::ALT);
        match key.code {
            KeyCode::Enter if newline => self.insert('\n'),
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => self.insert(c),
            KeyCode::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                let at = self.byte(self.cursor);
                self.text.remove(at);
            }
            KeyCode::Delete if self.cursor < len => {
                let at = self.byte(self.cursor);
                self.text.remove(at);
            }
            KeyCode::Backspace | KeyCode::Delete => return Typed::Moved,
            KeyCode::Left => {
                self.cursor = self.cursor.saturating_sub(1);
                return Typed::Moved;
            }
            KeyCode::Right => {
                self.cursor = (self.cursor + 1).min(len);
                return Typed::Moved;
            }
            KeyCode::Up if line > 0 => {
                self.place(width, line - 1, col);
                return Typed::Moved;
            }
            KeyCode::Down => {
                if line + 1 < self.lines(width).len() {
                    self.place(width, line + 1, col);
                }
                return Typed::Moved;
            }
            KeyCode::Up => return Typed::Moved,
            KeyCode::Home => {
                self.place(width, line, 0);
                return Typed::Moved;
            }
            KeyCode::End => {
                self.place(width, line, usize::MAX / 2);
                return Typed::Moved;
            }
            _ => return Typed::No,
        }
        Typed::Changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(f: &mut Field, code: KeyCode, mods: KeyModifiers) {
        f.key(KeyEvent::new(code, mods), 10);
    }

    fn keys(f: &mut Field, codes: &[KeyCode]) {
        for &c in codes {
            press(f, c, KeyModifiers::NONE);
        }
    }

    #[test]
    fn typing_goes_in_at_the_cursor_and_shift_enter_breaks_the_line() {
        let mut f = Field::new("helo");
        keys(&mut f, &[KeyCode::Left, KeyCode::Left, KeyCode::Char('l')]);
        assert_eq!(f.text, "hello");
        keys(&mut f, &[KeyCode::End]);
        press(&mut f, KeyCode::Enter, KeyModifiers::SHIFT);
        keys(
            &mut f,
            &[KeyCode::Char('x'), KeyCode::Home, KeyCode::Backspace],
        );
        assert_eq!(f.text, "hellox");
        keys(&mut f, &[KeyCode::Home, KeyCode::Delete]);
        assert_eq!(f.text, "ellox");
    }

    #[test]
    fn up_and_down_keep_the_column_across_wrapped_and_broken_lines() {
        // "aaaa bbbb " wraps after its space at 10 columns, then "cc" is its own line.
        let mut f = Field::new("aaaa bbbb cccc\ncc");
        assert_eq!(f.lines(10), vec![0..10, 10..14, 15..17]);
        assert_eq!(f.spot(10), (2, 2));
        keys(&mut f, &[KeyCode::Up]);
        assert_eq!(f.spot(10), (1, 2));
        keys(&mut f, &[KeyCode::Up, KeyCode::End]);
        // The end of a wrapped line stays on it, before the next line's first char.
        assert_eq!(f.spot(10), (0, 9));
        f.place(10, 1, 99);
        assert_eq!(f.spot(10), (1, 4));
    }
}
