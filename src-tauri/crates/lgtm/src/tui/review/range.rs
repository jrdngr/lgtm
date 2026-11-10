//! Selecting a range of lines with Shift-J and Shift-K, for a comment on all of them.

use std::ops::RangeInclusive;

use super::Review;
use super::rows::Line;

/// A row by hunk and index in the hunk.
type HunkRow = (usize, usize);

impl Review {
    /// Moves the cursor `n` lines, selecting from where it was when the selection started.
    pub(super) fn extend(&mut self, n: isize) {
        if self.anchor.is_none() {
            self.anchor = self.current();
        }
        self.by(n);
    }

    /// Grows the selection to the cursor's hunk, then its file, then the whole review.
    pub(super) fn grow(&mut self) {
        let Some(Line::Code { file, hunk, .. }) = self.current() else {
            return;
        };
        let now = self.selection().unwrap_or(self.cursor..=self.cursor);
        let levels: [&dyn Fn(&Line) -> bool; 3] = [
            &|l| matches!(*l, Line::Code { file: f, hunk: h, .. } if f == file && h == hunk),
            &|l| matches!(*l, Line::Code { file: f, .. } if f == file),
            &|l| matches!(l, Line::Code { .. }),
        ];
        for level in levels {
            let (Some(first), Some(last)) = (
                self.lines.iter().position(level),
                self.lines.iter().rposition(level),
            ) else {
                continue;
            };
            if first <= *now.start() && last >= *now.end() && (first..=last) != now {
                self.grown.push((self.anchor, self.cursor));
                self.anchor = Some(self.lines[first]);
                self.cursor = last;
                return;
            }
        }
    }

    /// Goes back to the selection before the last `grow`.
    pub(super) fn shrink(&mut self) {
        if let Some((anchor, cursor)) = self.grown.pop() {
            self.anchor = anchor;
            self.cursor = cursor;
        }
    }

    /// The selected lines, first to last, when there's a selection.
    pub(super) fn selection(&self) -> Option<RangeInclusive<usize>> {
        let at = self.lines.iter().position(|l| Some(*l) == self.anchor)?;
        Some(at.min(self.cursor)..=at.max(self.cursor))
    }

    /// The lines shown as picked: the selection, else the rows of the comment being written
    /// or the cursor's comment.
    pub(super) fn picked_lines(&self) -> Option<RangeInclusive<usize>> {
        if let Some(s) = self.selection() {
            return Some(s);
        }
        let (file, start, end) = self.commented_rows()?;
        let at = |(h, r): HunkRow| {
            move |l: &Line| {
                matches!(*l, Line::Code { file: f, hunk, old, new, .. }
                    if f == file && hunk == h && (old == Some(r) || new == Some(r)))
            }
        };
        let first = self.lines.iter().position(at(start))?;
        let last = self.lines.iter().rposition(at(end))?;
        Some(first..=last)
    }

    /// The first and last code rows of the selection, as (hunk, row), in its last row's file,
    /// with that file. Without a selection, the cursor's row.
    pub(super) fn selected_rows(&self) -> Option<(usize, HunkRow, HunkRow)> {
        let range = self.selection().unwrap_or(self.cursor..=self.cursor);
        let rows: Vec<(usize, usize, usize)> = self.lines[range]
            .iter()
            .filter_map(|l| match *l {
                Line::Code {
                    file,
                    hunk,
                    old,
                    new,
                    ..
                } => Some((file, hunk, new.or(old)?)),
                _ => None,
            })
            .collect();
        let &(file, hunk, row) = rows.last()?;
        let &(_, first_hunk, first_row) = rows.iter().find(|r| r.0 == file)?;
        Some((file, (first_hunk, first_row), (hunk, row)))
    }
}

impl Review {
    /// The screen lines the text selection covers.
    pub(super) fn selecting_lines(&self) -> Option<RangeInclusive<usize>> {
        let b = &self.selecting.as_ref()?.buf;
        Some(b.line_of(b.sel.from())..=b.line_of(b.sel.to()))
    }

    /// Where the code rows on screen lines `range` are, one `path:lines` per hunk.
    pub(super) fn mention(&self, range: RangeInclusive<usize>) -> Option<String> {
        let mut hunks: Vec<((usize, usize), Vec<usize>)> = Vec::new();
        for line in self.lines.get(range)? {
            let Line::Code {
                file,
                hunk,
                old,
                new,
                ..
            } = *line
            else {
                continue;
            };
            let at = match hunks.last_mut() {
                Some((key, rows)) if *key == (file, hunk) => rows,
                _ => {
                    hunks.push(((file, hunk), Vec::new()));
                    &mut hunks.last_mut().expect("just pushed").1
                }
            };
            for row in [old, new].into_iter().flatten() {
                if !at.contains(&row) {
                    at.push(row);
                }
            }
        }
        let refs: Vec<String> = hunks
            .into_iter()
            .map(|((file, hunk), mut rows)| {
                let f = &self.files[file];
                rows.sort_unstable();
                let rows: Vec<_> = rows.iter().map(|&r| &f.hunks[hunk].rows[r]).collect();
                crate::tui::mention::rows(&f.path, &rows)
            })
            .collect();
        (!refs.is_empty()).then(|| refs.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::KeyCode;

    use super::super::Outcome;
    use super::super::tests::{press, review};
    use super::*;

    fn picked(r: &Review) -> Vec<Line> {
        r.selection()
            .map_or_else(Vec::new, |s| r.lines[s].to_vec())
            .into_iter()
            .filter(|l| matches!(l, Line::Code { .. }))
            .collect()
    }

    fn code(r: &Review, keep: impl Fn(usize, usize) -> bool) -> Vec<Line> {
        r.lines
            .iter()
            .copied()
            .filter(|l| matches!(*l, Line::Code { file, hunk, .. } if keep(file, hunk)))
            .collect()
    }

    #[test]
    fn a_sends_where_the_picked_rows_are_per_hunk() {
        use ratatui::crossterm::event::{KeyEvent, KeyModifiers};
        let mut r = review();
        r.cursor = r
            .lines
            .iter()
            .position(|l| matches!(l, Line::Code { .. }))
            .unwrap();
        press(&mut r, KeyCode::Char('e'));
        press(&mut r, KeyCode::Char('e'));
        let out = r.key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE), None);
        let hunks = r.files[0].hunks.len();
        let one = "a.rs:1, and removed old line 1";
        let Outcome::Mention(text) = out else {
            panic!("no mention")
        };
        assert_eq!(text, vec![one; hunks].join("\n"));
        assert_eq!(r.selection(), None);
    }

    #[test]
    fn e_grows_to_the_hunk_file_and_review_and_shrink_goes_back() {
        let mut r = review();
        r.cursor = r
            .lines
            .iter()
            .position(|l| matches!(l, Line::Code { .. }))
            .unwrap();
        press(&mut r, KeyCode::Char('e'));
        assert_eq!(picked(&r), code(&r, |f, h| f == 0 && h == 0));
        press(&mut r, KeyCode::Char('e'));
        assert_eq!(picked(&r), code(&r, |f, _| f == 0));
        press(&mut r, KeyCode::Char('e'));
        assert_eq!(picked(&r), code(&r, |_, _| true));
        press(&mut r, KeyCode::Char('E'));
        assert_eq!(picked(&r), code(&r, |f, _| f == 0));
        press(&mut r, KeyCode::Char('j'));
        press(&mut r, KeyCode::Char('E'));
        assert_eq!(r.selection(), None);
    }
}
