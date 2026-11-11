//! `/` searches the diff's text. The match it finds is selected, so `n` and `N` go on to the
//! next and previous match while the selection lasts. In select mode `/` starts from the
//! selected text, and `n` and `N` look for it.

use super::Review;
use super::command::CmdLine;
use crate::edit::buffer::Sel;
use crate::edit::motion;

impl Review {
    /// Opens the search line holding `text`.
    pub(super) fn start_search(&mut self, text: &str) {
        self.cmd = Some(CmdLine {
            text: text.to_string(),
            search: true,
            ..CmdLine::default()
        });
    }

    /// The selection's text up to its first line break, for the search line.
    pub(super) fn selection_needle(&self) -> String {
        let Some(s) = &self.selecting else {
            return String::new();
        };
        let text = self.selected_text(&s.buf);
        text.lines().next().unwrap_or("").to_string()
    }

    /// Selects the first match of `needle` after the cursor.
    pub(super) fn search(&mut self, needle: &str) {
        if needle.is_empty() {
            return;
        }
        if self.selecting.is_none() {
            self.start_select();
        }
        if let Some(s) = self.selecting.as_mut() {
            s.keys.search = Some(needle.to_string());
        }
        if !self.next_match(true) {
            self.selecting = None;
        }
    }

    /// Selects the next or previous match of what's searched, or of the selection when
    /// nothing is. Returns whether there was one.
    pub(super) fn next_match(&mut self, forward: bool) -> bool {
        let needle = self.selection_needle();
        let Some(s) = self.selecting.as_mut() else {
            return false;
        };
        let Some(needle) = s
            .keys
            .search
            .clone()
            .or(Some(needle).filter(|n| !n.is_empty()))
        else {
            return false;
        };
        s.keys.search = Some(needle.clone());
        let b = &mut s.buf;
        let at = if forward { b.sel.to() } else { b.sel.from() };
        let Some((from, to)) = motion::search(b, at, &needle, forward) else {
            self.notice = Some(format!("no match for {needle}"));
            return false;
        };
        b.sel = Sel::new(from, to - 1);
        self.follow_head();
        true
    }
}

#[cfg(test)]
mod tests {
    use lgtm_core::diff::{Hunk, Row, RowKind};
    use ratatui::crossterm::event::KeyCode;

    use super::super::tests::{file, press};
    use super::*;
    use crate::tui::load::Target;

    fn review() -> Review {
        let row = |kind, text: &str| Row {
            kind,
            old_line: Some(1),
            new_line: Some(1),
            text: text.into(),
        };
        let mut f = file("a.rs", 0);
        f.hunks.push(Hunk {
            old_start: 1,
            old_lines: 3,
            new_start: 1,
            new_lines: 3,
            section: String::new(),
            rows: vec![
                row(RowKind::Context, "let foo = 1;"),
                row(RowKind::Context, "bar(foo);"),
                row(RowKind::Context, "foo + foo"),
            ],
        });
        let mut r = Review::new(1, Target::Revset("@".into()));
        r.loaded(Ok((vec![], vec![f, file("b.rs", 1)])));
        r
    }

    fn typed(r: &mut Review, text: &str) {
        for c in text.chars() {
            press(r, KeyCode::Char(c));
        }
    }

    fn selected(r: &Review) -> (usize, String) {
        let s = r.selecting.as_ref().expect("a selection");
        (r.cursor, r.selected_text(&s.buf))
    }

    #[test]
    fn slash_selects_the_next_match_and_n_and_shift_n_step_through_the_rest() {
        let mut r = review();
        press(&mut r, KeyCode::Char('/'));
        typed(&mut r, "foo");
        press(&mut r, KeyCode::Enter);
        assert_eq!(selected(&r), (1, "foo".into()));
        press(&mut r, KeyCode::Char('n'));
        assert_eq!(selected(&r), (2, "foo".into()));
        press(&mut r, KeyCode::Char('n'));
        assert_eq!(selected(&r).0, 3);
        press(&mut r, KeyCode::Char('N'));
        assert_eq!(selected(&r).0, 2);
        // Without a selection, n goes to the next file again.
        press(&mut r, KeyCode::Esc);
        press(&mut r, KeyCode::Char('n'));
        assert!(r.selecting.is_none());
        assert_eq!(r.current().map(|l| l.file()), Some(1));
    }

    #[test]
    fn a_selection_fills_the_search_line_and_n_and_shift_n_look_for_it() {
        let mut r = review();
        typed(&mut r, "jjjve");
        assert_eq!(selected(&r), (3, "foo".into()));
        press(&mut r, KeyCode::Char('/'));
        assert_eq!(r.prompt().map(|p| p.0), Some("/foo".into()));
        press(&mut r, KeyCode::Esc);
        press(&mut r, KeyCode::Char('N'));
        assert_eq!(selected(&r), (2, "foo".into()));
    }

    #[test]
    fn a_search_with_no_match_says_so_and_drops_the_selection() {
        let mut r = review();
        press(&mut r, KeyCode::Char('/'));
        typed(&mut r, "nope");
        press(&mut r, KeyCode::Enter);
        assert!(r.selecting.is_none());
        assert_eq!(r.notice.as_deref(), Some("no match for nope"));
    }
}
