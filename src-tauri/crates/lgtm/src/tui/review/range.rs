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

    /// The selected lines, first to last, when there's a selection.
    pub(super) fn selection(&self) -> Option<RangeInclusive<usize>> {
        let at = self.lines.iter().position(|l| Some(*l) == self.anchor)?;
        Some(at.min(self.cursor)..=at.max(self.cursor))
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
                } => Some((file, hunk, new.or(old)?)),
                _ => None,
            })
            .collect();
        let &(file, hunk, row) = rows.last()?;
        let &(_, first_hunk, first_row) = rows.iter().find(|r| r.0 == file)?;
        Some((file, (first_hunk, first_row), (hunk, row)))
    }
}
