//! `m` and `M`: the cursor steps to the next or previous syntax item at its line's level, on
//! the side of the file its row shows. Items the diff doesn't show are passed over.

use lgtm_core::syntax::Items;

use super::Review;
use super::rows::Line;

impl Review {
    /// The side and line from 0 that screen line `i` shows: the new side's where it has one.
    fn side_line(&self, i: usize) -> Option<(usize, bool, u32)> {
        let Line::Code {
            file,
            hunk,
            old,
            new,
            part: 0,
        } = *self.lines.get(i)?
        else {
            return None;
        };
        let rows = &self.files[file].hunks[hunk].rows;
        let row = &rows[new.or(old)?];
        match (row.new_line, row.old_line) {
            (Some(n), _) => Some((file, true, n - 1)),
            (None, Some(o)) => Some((file, false, o - 1)),
            _ => None,
        }
    }

    /// Moves the cursor to the next or previous item after the one on its line.
    pub(super) fn jump_item(&mut self, forward: bool) {
        let Some((file, new, mut line)) = self.side_line(self.cursor) else {
            return;
        };
        let Some(sides) = self.sides.get(file).and_then(Option::as_ref) else {
            return;
        };
        let items: &Items = if new {
            &sides.new_items
        } else {
            &sides.old_items
        };
        let steps = if forward { &items.next } else { &items.prev };
        let shown: Vec<(u32, usize)> = (0..self.lines.len())
            .filter_map(|i| match self.side_line(i) {
                Some((f, n, l)) if f == file && n == new => Some((l, i)),
                _ => None,
            })
            .collect();
        for _ in 0..steps.len() {
            let Some(to) = steps.get(line as usize).copied().flatten() else {
                return;
            };
            if let Some(&(_, i)) = shown.iter().find(|(l, _)| *l == to) {
                self.cursor = i;
                return;
            }
            line = to;
        }
    }
}

#[cfg(test)]
mod tests {
    use lgtm_core::diff::{Hunk, Row, RowKind};
    use ratatui::crossterm::event::KeyCode;

    use super::super::tests::{file, press};
    use super::*;
    use crate::tui::highlight::Sides;
    use crate::tui::load::Target;

    /// A review of `a.py` showing `shown` of `src`'s lines, from 1, as context.
    fn review(src: &str, shown: &[u32]) -> Review {
        let lines: Vec<&str> = src.lines().collect();
        let mut f = file("a.py", 0);
        f.hunks.push(Hunk {
            old_start: 1,
            old_lines: 0,
            new_start: 1,
            new_lines: 0,
            section: String::new(),
            rows: shown
                .iter()
                .map(|&n| Row {
                    kind: RowKind::Context,
                    old_line: Some(n),
                    new_line: Some(n),
                    text: lines[n as usize - 1].into(),
                })
                .collect(),
        });
        let mut r = Review::new(1, Target::Revset("@".into()));
        r.loaded(Ok((vec![], vec![f])));
        r.highlighted(0, Sides::of("a.py", Some(src), Some(src)));
        r
    }

    fn line(r: &Review) -> Option<u32> {
        r.side_line(r.cursor).map(|(_, _, l)| l + 1)
    }

    const SRC: &str = "\
def a():
    try:
        x()
    except E:
        y()
    z()

def b():
    pass

def c():
    pass
";

    #[test]
    fn m_steps_to_the_next_item_at_its_level_and_shift_m_back() {
        let mut r = review(SRC, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        r.cursor = 2;
        assert_eq!(line(&r), Some(2));
        press(&mut r, KeyCode::Char('m'));
        assert_eq!(line(&r), Some(4));
        press(&mut r, KeyCode::Char('m'));
        assert_eq!(line(&r), Some(6));
        press(&mut r, KeyCode::Char('M'));
        assert_eq!(line(&r), Some(4));
    }

    #[test]
    fn m_passes_over_items_the_diff_doesnt_show() {
        let mut r = review(SRC, &[1, 2, 11, 12]);
        r.cursor = 1;
        assert_eq!(line(&r), Some(1));
        press(&mut r, KeyCode::Char('m'));
        assert_eq!(line(&r), Some(11));
        // Nothing follows `def c`, so the cursor stays.
        press(&mut r, KeyCode::Char('m'));
        assert_eq!(line(&r), Some(11));
    }
}
