//! The review as a mode beside the editor: `space` and its menu, and landing on the line the
//! editor was on.

use super::Review;
use super::reload::Spot;

impl Review {
    /// What `space` goes on to, while it waits for the next key.
    pub fn space_menu(&self) -> Vec<(String, &'static str)> {
        if !self.space {
            return vec![];
        }
        [
            ("f", "files"),
            ("b", "buffers"),
            ("/", "search"),
            ("r", "editor"),
        ]
        .into_iter()
        .map(|(k, l)| (k.to_string(), l))
        .collect()
    }

    /// Puts the cursor on `line` of `path`, a line from 1 holding `text`, or on the row
    /// nearest it: now when the diff has the file, and again once the next load is in.
    pub fn go_to_line(&mut self, path: &str, line: u32, text: &str) {
        let spot = Spot::line(path, line, text);
        if self.files.iter().any(|f| f.path == path) {
            self.cursor = spot.find(&self.files, &self.lines);
            self.pinned = false;
        }
        self.goto = Some(spot);
    }

    pub(super) fn go_to(&mut self, spot: Spot) {
        if !self.files.iter().any(|f| f.path == spot.path()) {
            self.notice = Some(format!("no changes in {}", spot.path()));
            return;
        }
        self.cursor = spot.find(&self.files, &self.lines);
        self.pinned = false;
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::super::tests::{file, review};
    use super::super::{Outcome, Review};
    use crate::edit::keymap::Pick;
    use crate::tui::load::Target;

    fn key(r: &mut Review, c: char) -> Outcome {
        r.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE), None)
    }

    #[test]
    fn space_r_goes_to_the_editor_on_the_cursors_line_and_space_f_picks_a_file() {
        let mut r = review();
        key(&mut r, ']');
        assert!(matches!(key(&mut r, ' '), Outcome::None));
        assert_eq!(r.space_menu().len(), 4);
        let out = key(&mut r, 'r');
        assert!(matches!(out, Outcome::Editor(Some((p, 1))) if p == "a.rs"));
        assert!(r.space_menu().is_empty());
        key(&mut r, ' ');
        assert!(matches!(key(&mut r, 'f'), Outcome::Pick(Pick::Files)));
    }

    #[test]
    fn going_to_a_line_lands_in_its_file_once_the_diff_loads() {
        let mut r = Review::new(1, Target::Revset("@".into()));
        r.go_to_line("b.rs", 1, "");
        r.loaded(Ok((vec![], vec![file("a.rs", 2), file("b.rs", 1)])));
        assert_eq!(r.current().map(|l| l.file()), Some(1));
        r.go_to_line("c.rs", 1, "");
        r.loaded(Ok((vec![], vec![file("a.rs", 2), file("b.rs", 1)])));
        assert_eq!(r.current().map(|l| l.file()), Some(1));
        assert_eq!(r.notice.as_deref(), Some("no changes in c.rs"));
    }
}
