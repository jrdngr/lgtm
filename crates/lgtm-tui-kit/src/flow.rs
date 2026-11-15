//! Laying out a row of pieces that wraps onto more lines when it doesn't fit.

use std::ops::Range;

/// Splits pieces `widths` wide, `gap` apart, into lines at most `width` wide: each line's
/// pieces as a range. A piece wider than `width` gets a line of its own.
pub fn lines(widths: &[u16], gap: u16, width: u16) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let (mut start, mut used) = (0, 0u16);
    for (i, &w) in widths.iter().enumerate() {
        if i > start && used + gap + w > width {
            out.push(start..i);
            (start, used) = (i, 0);
        }
        used += if i > start { gap + w } else { w };
    }
    if start < widths.len() {
        out.push(start..widths.len());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces_wrap_where_the_next_one_would_pass_the_width() {
        assert_eq!(lines(&[3, 4, 2], 2, 20), vec![0..3]);
        assert_eq!(lines(&[3, 4, 2], 2, 9), [0..2, 2..3]);
        assert_eq!(lines(&[3, 4, 2], 2, 8), [0..1, 1..3]);
    }

    #[test]
    fn a_piece_wider_than_the_line_takes_a_line_alone() {
        assert_eq!(lines(&[2, 10, 2], 1, 5), [0..1, 1..2, 2..3]);
        assert!(lines(&[], 1, 5).is_empty());
    }
}
