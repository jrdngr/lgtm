//! Where motions go: functions from the text and a position to a new position.

use super::buffer::Buffer;

/// The last char a normal-mode cursor can be on in `line`: before its newline, or the
/// newline itself on an empty line.
pub fn line_max(b: &Buffer, line: usize) -> usize {
    let start = b.line_start(line);
    let end = b.line_end(line);
    if end > start { end - 1 } else { start }
}

pub fn left(b: &Buffer, at: usize) -> usize {
    if at > b.line_start(b.line_of(at)) {
        at - 1
    } else {
        at
    }
}

pub fn right(b: &Buffer, at: usize) -> usize {
    (at + 1).min(line_max(b, b.line_of(at))).max(at)
}

/// `by` lines down (up when negative) from `at`, at `column` or `at`'s own. Returns where
/// and the column to keep.
pub fn vertical(b: &Buffer, at: usize, by: isize, column: Option<usize>) -> (usize, usize) {
    let line = b.line_of(at);
    let column = column.unwrap_or(at - b.line_start(line));
    let target = (line as isize + by).clamp(0, b.last_line() as isize) as usize;
    let to = (b.line_start(target) + column).min(line_max(b, target));
    (to, column)
}

pub fn first_non_blank(b: &Buffer, line: usize) -> usize {
    b.line_start(line) + b.indent(line).chars().count()
}

/// Where Ctrl-w deletes back to from the gap `at`: past spaces, then a word or a run of
/// punctuation.
pub fn word_back_from(b: &Buffer, at: usize) -> usize {
    let start = b.line_start(b.line_of(at));
    let mut i = at;
    while i > start && b.char(i - 1).is_some_and(|c| c == ' ' || c == '\t') {
        i -= 1;
    }
    let kind = b.char(i.saturating_sub(1)).map(class);
    while i > start && b.char(i - 1).map(class) == kind {
        i -= 1;
    }
    i
}

/// One level of indent: a tab when the file indents with tabs, else four spaces.
pub fn indent_unit(b: &Buffer) -> String {
    let tabs = (0..=b.last_line())
        .map(|l| b.indent(l))
        .find(|i| !i.is_empty())
        .is_some_and(|i| i.starts_with('\t'));
    if tabs { "\t".into() } else { "    ".into() }
}

/// What a char counts as, for word motions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Space,
    Eol,
    Word,
    Punct,
}

pub fn class(c: char) -> Class {
    match c {
        '\n' | '\r' => Class::Eol,
        c if c.is_whitespace() => Class::Space,
        c if c.is_alphanumeric() || c == '_' => Class::Word,
        _ => Class::Punct,
    }
}
