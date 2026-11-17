//! Word-wrapped lines: where each row of a line starts, and columns with tabs expanded.

use crate::edit::buffer::Buffer;

/// Columns a tab takes.
pub const TAB: usize = 4;

/// The screen column of char `at` in its line, tabs expanded.
pub fn column(b: &Buffer, at: usize) -> usize {
    let start = b.line_start(b.line_of(at));
    b.text
        .slice(start..at.min(b.len()))
        .chars()
        .map(|c| if c == '\t' { TAB } else { 1 })
        .sum()
}

/// The row of a wrapped line that column `col` is on, and its column there.
pub fn place(starts: &[usize], col: usize) -> (usize, usize) {
    let row = starts.partition_point(|&s| s <= col).saturating_sub(1);
    (row, col - starts.get(row).copied().unwrap_or(0))
}

/// The char at column `col` of `line`, tabs expanded, or the line's end past it.
pub fn char_at(b: &Buffer, line: usize, col: usize) -> usize {
    let start = b.line_start(line);
    let text = b.slice(start, b.line_end(line));
    let text = text.trim_end_matches('\r');
    let mut at = 0;
    for (k, ch) in text.chars().enumerate() {
        at += if ch == '\t' { TAB } else { 1 };
        if at > col {
            return start + k;
        }
    }
    start + text.chars().count()
}

/// The columns line `line`'s rows start at, wrapped at `width` columns.
pub fn row_starts(b: &Buffer, line: usize, width: usize) -> Vec<usize> {
    let text = b.slice(b.line_start(line), b.line_end(line));
    wrap_starts(text.trim_end_matches('\r'), width)
}

/// The columns the rows of `text` start at, wrapped at `width` columns: after the last space
/// that fits, or mid-word when a word is wider than the row. A full last row gets one after
/// it, for the cursor at the line's end.
pub fn wrap_starts(text: &str, width: usize) -> Vec<usize> {
    let width = width.max(1);
    let mut starts = vec![0];
    // Where the row would break: the column after the last space on it.
    let mut after_space = None;
    let mut col = 0;
    for ch in text.chars() {
        let w = if ch == '\t' { TAB } else { 1 };
        let start = *starts.last().unwrap_or(&0);
        // A space may hang past the edge; anything else starts a new row.
        if col + w > start + width && !ch.is_whitespace() {
            let at = after_space.filter(|&a| a > start).unwrap_or(col);
            starts.push(at);
            after_space = None;
        }
        col += w;
        if ch.is_whitespace() {
            after_space = Some(col);
        }
    }
    let last = *starts.last().unwrap_or(&0);
    if col - last >= width {
        starts.push(col);
    }
    starts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_breaks_after_spaces_and_inside_only_words_wider_than_the_row() {
        assert_eq!(wrap_starts("the cat sat", 8), [0, 8]);
        assert_eq!(wrap_starts("ab cdefghij", 5), [0, 3, 8]);
        // Spaces hang past the edge rather than start a row.
        assert_eq!(wrap_starts("abcd  efg", 4), [0, 6]);
        // A full last row leaves a row for the cursor at the end.
        assert_eq!(wrap_starts("abcd", 4), [0, 4]);
        assert_eq!(wrap_starts("", 4), [0]);
    }
}
