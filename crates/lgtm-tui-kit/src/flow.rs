//! Bars of pieces that wrap onto more rows when they don't fit on one.

use ratatui::style::Style;

use crate::paint::Canvas;

/// Text that stays together on a row, in one or more styles, and the columns before it when
/// it follows another piece on the same row.
#[derive(Clone, Debug, Default)]
pub struct Piece {
    pub gap: u16,
    pub spans: Vec<(String, Style)>,
}

impl Piece {
    pub fn new(gap: u16, text: impl Into<String>, style: Style) -> Self {
        Self {
            gap,
            spans: vec![(text.into(), style)],
        }
    }

    /// The piece with `text` after what it has.
    pub fn then(mut self, text: impl Into<String>, style: Style) -> Self {
        self.spans.push((text.into(), style));
        self
    }

    fn width(&self) -> u16 {
        self.spans
            .iter()
            .map(|(t, _)| t.chars().count() as u16)
            .sum()
    }
}

/// Where each piece of `left` then `right` goes in a bar `width` wide, as (row, column). On
/// one row `left` starts at the left and `right` ends at the right edge; when they don't fit,
/// all of them run from the left and wrap. A piece wider than `width` gets a row of its own.
fn place(width: u16, left: &[Piece], right: &[Piece]) -> Vec<(u16, u16)> {
    let run = |ps: &[Piece]| -> u16 {
        ps.iter()
            .enumerate()
            .map(|(i, p)| p.width() + if i > 0 { p.gap } else { 0 })
            .sum()
    };
    let (l, r) = (run(left), run(right));
    let between = right.first().map_or(0, |p| p.gap);
    let mut out = Vec::new();
    if l + between + r <= width || left.is_empty() && r <= width {
        let mut x = 0;
        for (i, p) in left.iter().enumerate() {
            x += if i > 0 { p.gap } else { 0 };
            out.push((0, x));
            x += p.width();
        }
        let mut x = width - r;
        for (i, p) in right.iter().enumerate() {
            x += if i > 0 { p.gap } else { 0 };
            out.push((0, x));
            x += p.width();
        }
        return out;
    }
    let (mut row, mut x) = (0, 0);
    for (i, p) in left.iter().chain(right).enumerate() {
        if i > 0 && x + p.gap + p.width() > width {
            (row, x) = (row + 1, 0);
        }
        if x > 0 {
            x += p.gap;
        }
        out.push((row, x));
        x += p.width();
    }
    out
}

/// The rows a bar of `left` and `right`, `width` wide, takes.
pub fn rows(width: u16, left: &[Piece], right: &[Piece]) -> u16 {
    place(width, left, right)
        .last()
        .map_or(1, |&(row, _)| row + 1)
}

/// Draws a bar of `left` and `right` from (`x`, `y`), `width` wide. Returns the rows it took.
pub fn draw(
    c: &mut Canvas,
    (x, y): (u16, u16),
    width: u16,
    left: &[Piece],
    right: &[Piece],
) -> u16 {
    let spots = place(width, left, right);
    for (p, &(row, col)) in left.iter().chain(right).zip(&spots) {
        let mut px = x + col;
        for (text, style) in &p.spans {
            px = c.put(px, y + row, text, *style);
        }
    }
    spots.last().map_or(1, |&(row, _)| row + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pieces(spec: &[(u16, &str)]) -> Vec<Piece> {
        spec.iter()
            .map(|&(gap, t)| Piece::new(gap, t, Style::default()))
            .collect()
    }

    #[test]
    fn a_bar_that_fits_keeps_the_right_side_against_the_edge() {
        let left = pieces(&[(0, "abc"), (2, "de")]);
        let right = pieces(&[(3, "fg"), (1, "h")]);
        assert_eq!(place(20, &left, &right), [(0, 0), (0, 5), (0, 16), (0, 19)]);
        assert_eq!(place(14, &left, &right), [(0, 0), (0, 5), (0, 10), (0, 13)]);
    }

    #[test]
    fn a_bar_that_doesnt_fit_wraps_every_piece_from_the_left() {
        let left = pieces(&[(0, "abc"), (2, "de")]);
        let right = pieces(&[(3, "fg"), (1, "h")]);
        assert_eq!(place(9, &left, &right), [(0, 0), (0, 5), (1, 0), (1, 3)]);
        assert_eq!(place(4, &left, &right), [(0, 0), (1, 0), (2, 0), (2, 3)]);
        assert_eq!(rows(4, &left, &right), 3);
    }

    #[test]
    fn a_piece_wider_than_the_bar_takes_a_row_alone() {
        let left = pieces(&[(0, "ab"), (1, "cdefgh"), (1, "i")]);
        assert_eq!(place(4, &left, &[]), [(0, 0), (1, 0), (2, 0)]);
    }
}
