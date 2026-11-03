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

/// What a char counts as for WORD motions, which only split at spaces.
fn class_big(c: char) -> Class {
    match class(c) {
        Class::Punct => Class::Word,
        other => other,
    }
}

fn class_at(b: &Buffer, i: usize, big: bool) -> Class {
    let c = b.char(i).unwrap_or('\n');
    if big { class_big(c) } else { class(c) }
}

/// `w`: from `at` to just before the next word's start, as `(anchor, head)`.
pub fn next_word_start(b: &Buffer, at: usize, big: bool) -> (usize, usize) {
    let n = b.len();
    if n == 0 {
        return (0, 0);
    }
    let cls = |i| class_at(b, i, big);
    let mut start = at;
    if at + 1 < n && cls(at) != cls(at + 1) {
        start = at + 1;
    }
    let mut i = start;
    while i < n && cls(i) == Class::Eol {
        i += 1;
        start = i;
    }
    if i >= n {
        return (at, at);
    }
    let kind = cls(i);
    while i < n && cls(i) == kind {
        i += 1;
    }
    while i < n && cls(i) == Class::Space {
        i += 1;
    }
    (start, i - 1)
}

/// `e`: from `at` to the end of the next word, as `(anchor, head)`.
pub fn next_word_end(b: &Buffer, at: usize, big: bool) -> (usize, usize) {
    let n = b.len();
    if n == 0 {
        return (0, 0);
    }
    let cls = |i| class_at(b, i, big);
    let mut start = at;
    if at + 1 < n && cls(at) != cls(at + 1) {
        start = at + 1;
    }
    let mut i = start;
    while i < n && matches!(cls(i), Class::Space | Class::Eol) {
        i += 1;
    }
    if i >= n {
        return (at, at);
    }
    let kind = cls(i);
    while i + 1 < n && cls(i + 1) == kind {
        i += 1;
    }
    (start, i)
}

/// `b`: from `at` back to the start of the word before, as `(anchor, head)`.
pub fn prev_word_start(b: &Buffer, at: usize, big: bool) -> (usize, usize) {
    if at == 0 {
        return (0, 0);
    }
    let cls = |i| class_at(b, i, big);
    let mut start = at;
    if cls(at) != cls(at - 1) {
        start = at - 1;
    }
    let mut i = start;
    while i > 0 && matches!(cls(i), Class::Space | Class::Eol) {
        i -= 1;
    }
    let kind = cls(i);
    while i > 0 && cls(i - 1) == kind {
        i -= 1;
    }
    (start, i)
}

/// `f` `t` `F` `T`: where `c` is on `at`'s line, forward or back, on it or next to it.
pub fn find_char(b: &Buffer, at: usize, c: char, forward: bool, till: bool) -> Option<usize> {
    let line = b.line_of(at);
    let (start, end) = (b.line_start(line), b.line_end(line));
    if forward {
        let from = at + 1 + usize::from(till);
        let i = (from..end).find(|&i| b.char(i) == Some(c))?;
        Some(if till { i - 1 } else { i })
    } else {
        let to = at.saturating_sub(usize::from(till));
        let i = (start..to).rev().find(|&i| b.char(i) == Some(c))?;
        Some(if till { i + 1 } else { i })
    }
}

/// `x`: the selection's lines, newline and all, or one line more when it's that already.
pub fn line_select(b: &Buffer, from: usize, to: usize) -> (usize, usize) {
    let first = b.line_of(from);
    let last = b.line_of(to);
    let end_of = |line| b.line_end(line).min(b.last_char());
    let whole = from == b.line_start(first) && to == end_of(last);
    let last = if whole && last < b.last_line() {
        last + 1
    } else {
        last
    };
    (b.line_start(first), end_of(last))
}

/// The next match of `needle` after `at`, or before it going back, wrapping around, as
/// `(start, end)` with `end` past it. Matching ignores case unless `needle` has a capital.
pub fn search(b: &Buffer, at: usize, needle: &str, forward: bool) -> Option<(usize, usize)> {
    let fold = !needle.chars().any(char::is_uppercase);
    let norm = |c: char| {
        if fold {
            c.to_lowercase().next().unwrap_or(c)
        } else {
            c
        }
    };
    let hay: Vec<char> = b.text.chars().map(norm).collect();
    let pat: Vec<char> = needle.chars().map(norm).collect();
    if pat.is_empty() || pat.len() > hay.len() {
        return None;
    }
    let last = hay.len() - pat.len();
    let hit = |i: usize| hay[i..i + pat.len()] == pat[..];
    let found = if forward {
        (at + 1..=last).chain(0..=at.min(last)).find(|&i| hit(i))
    } else {
        (0..at.min(last + 1))
            .rev()
            .chain((at.min(last)..=last).rev())
            .find(|&i| hit(i))
    };
    found.map(|i| (i, i + pat.len()))
}
