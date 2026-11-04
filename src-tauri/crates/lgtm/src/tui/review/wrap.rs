//! Word wrapping for notes and code in the diff.

use std::ops::Range;

/// Columns a char of code takes: a tab four, anything else one.
pub fn cols(c: char) -> usize {
    if c == '\t' { 4 } else { 1 }
}

/// A code line's screen lines when wrapped to `width` columns, as ranges of its chars. Each
/// breaks after its last space that fits, or where the line runs out of room. An empty text
/// is one empty line.
pub fn code_parts(text: &str, width: usize) -> Vec<Range<usize>> {
    let width = width.max(4);
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let (mut end, mut used, mut space) = (start, 0, None);
        while end < chars.len() && used + cols(chars[end]) <= width {
            used += cols(chars[end]);
            if chars[end] == ' ' {
                space = Some(end + 1);
            }
            end += 1;
        }
        if end < chars.len()
            && let Some(after) = space.filter(|&s| s > start + 1)
        {
            end = after;
        }
        let end = end.max(start + 1);
        out.push(start..end);
        start = end;
    }
    if out.is_empty() {
        out.push(0..0);
    }
    out
}

/// `text`'s lines wrapped to `width` columns, breaking between words, or inside a word too long
/// for a line. An empty text is one empty line.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        let mut len = 0;
        for word in para.split(' ') {
            let mut word: Vec<char> = word.chars().collect();
            let gap = usize::from(len > 0);
            if len + gap + word.len() > width && len > 0 {
                out.push(std::mem::take(&mut line));
                len = 0;
            } else if gap == 1 {
                line.push(' ');
                len += 1;
            }
            while word.len() > width - len {
                let rest = word.split_off(width - len);
                line.extend(word);
                out.push(std::mem::take(&mut line));
                len = 0;
                word = rest;
            }
            len += word.len();
            line.extend(word);
        }
        out.push(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_breaks_after_its_last_space_that_fits_or_where_room_runs_out() {
        assert_eq!(code_parts("let a = b;", 6), [0..6, 6..10]);
        assert_eq!(code_parts("let a = b;", 4), [0..4, 4..8, 8..10]);
        assert_eq!(code_parts("abcdefghij", 4), [0..4, 4..8, 8..10]);
        // A tab takes four columns.
        assert_eq!(code_parts("\tab", 5), [0..2, 2..3]);
        assert_eq!(code_parts("", 8), vec![0..0]);
    }

    #[test]
    fn breaks_between_words_and_inside_words_too_long_for_a_line() {
        assert_eq!(wrap("one two three", 7), ["one two", "three"]);
        assert_eq!(wrap("abcdefghij", 4), ["abcd", "efgh", "ij"]);
        assert_eq!(wrap("a\n\nb", 5), ["a", "", "b"]);
        assert_eq!(wrap("", 5), [""]);
        assert_eq!(wrap("ab cdefgh", 4), ["ab", "cdef", "gh"]);
    }
}
