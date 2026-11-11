//! Edits on a buffer's selection. Each is one undo step; the caller checkpoints first.

use super::buffer::{Buffer, Sel};
use super::motion::indent_unit;

/// Yanked text, and whether it's whole lines, which paste as lines.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Register {
    pub text: String,
    pub lines: bool,
}

/// The selection's text, and whether it's whole lines.
pub fn yank(b: &Buffer) -> Register {
    let (from, to) = (b.sel.from(), b.sel.to());
    let text = b.slice(from, to + 1);
    let lines = from == b.line_start(b.line_of(from)) && text.ends_with('\n');
    Register { text, lines }
}

/// Removes the selection, leaving the cursor where it started.
pub fn delete(b: &mut Buffer) {
    let (from, to) = (b.sel.from(), b.sel.to());
    b.remove(from, (to + 1).min(b.len()));
    b.sel = Sel::point(from);
    b.clamp();
}

/// Removes the selection to type in its place. Whole lines keep their last newline, so the
/// typing goes on a line of its own. Returns where typing goes.
pub fn change(b: &mut Buffer) -> usize {
    let (from, mut to) = (b.sel.from(), b.sel.to() + 1);
    if yank(b).lines {
        to -= 1;
    }
    b.remove(from, to.min(b.len()));
    from
}

/// Pastes `r` after the selection or before it, and selects what was pasted.
pub fn paste(b: &mut Buffer, r: &Register, after: bool) {
    if r.text.is_empty() {
        return;
    }
    let at = match (r.lines, after) {
        (true, true) => {
            let line = b.line_of(b.sel.to());
            let end = b.line_end(line);
            if end >= b.len() {
                // The last line has no newline to paste after.
                b.insert(b.len(), "\n");
            }
            b.line_end(line) + 1
        }
        (true, false) => b.line_start(b.line_of(b.sel.from())),
        (false, true) => (b.sel.to() + 1).min(b.len()),
        (false, false) => b.sel.from(),
    };
    b.insert(at, &r.text);
    let len = r.text.chars().count();
    b.sel = Sel::new(at, at + len - 1);
    b.clamp();
}

/// Replaces every char of the selection but newlines with `c`.
pub fn replace(b: &mut Buffer, c: char) {
    let (from, to) = (b.sel.from(), b.sel.to());
    let text: String = b
        .slice(from, to + 1)
        .chars()
        .map(|x| if x == '\n' || x == '\r' { x } else { c })
        .collect();
    b.remove(from, (to + 1).min(b.len()));
    b.insert(from, &text);
}

/// Toggles the case of every letter in the selection.
pub fn toggle_case(b: &mut Buffer) {
    let (from, to) = (b.sel.from(), b.sel.to());
    let text: String = b
        .slice(from, to + 1)
        .chars()
        .flat_map(|c| {
            if c.is_uppercase() {
                c.to_lowercase().collect::<Vec<_>>()
            } else {
                c.to_uppercase().collect()
            }
        })
        .collect();
    b.remove(from, (to + 1).min(b.len()));
    b.insert(from, &text);
    b.sel = Sel::new(b.sel.anchor, b.sel.head);
    b.clamp();
}

/// Indents the selection's lines one level, or takes one level off. The selection stays on
/// the same text.
pub fn indent(b: &mut Buffer, more: bool) {
    let unit = indent_unit(b);
    let spot = |b: &Buffer, p: usize| (b.line_of(p), p - b.line_start(b.line_of(p)));
    let (anchor, head) = (spot(b, b.sel.anchor), spot(b, b.sel.head));
    let (first, last) = (b.line_of(b.sel.from()), b.line_of(b.sel.to()));
    let mut by = vec![0isize; last + 1 - first];
    for line in first..=last {
        let start = b.line_start(line);
        if more {
            if b.line_end(line) > start {
                b.insert(start, &unit);
                by[line - first] = unit.chars().count() as isize;
            }
        } else {
            let indent = b.indent(line);
            let take = if indent.starts_with('\t') {
                1
            } else {
                indent.chars().take(4).take_while(|c| *c == ' ').count()
            };
            b.remove(start, start + take);
            by[line - first] = -(take as isize);
        }
    }
    let place = |(line, col): (usize, usize)| {
        let col = (col as isize + by[line - first]).max(0) as usize;
        b.line_start(line) + col
    };
    b.sel = Sel::new(place(anchor), place(head));
    b.clamp();
}

/// Joins the selection's lines, or its line and the next, with single spaces.
pub fn join(b: &mut Buffer) {
    let first = b.line_of(b.sel.from());
    let last = b.line_of(b.sel.to()).max(first + 1).min(b.last_line());
    let mut at = None;
    for _ in first..last {
        let end = b.line_end(first);
        if end >= b.len() {
            break;
        }
        // The newline and the next line's indent become one space, or nothing before an
        // empty line.
        let next = first + 1;
        let skip = b.indent(next).chars().count();
        let empty_next = b.line_end(next) == b.line_start(next) + skip;
        b.remove(end, end + 1 + skip);
        if !empty_next {
            b.insert(end, " ");
        }
        at = Some(end);
    }
    if let Some(at) = at {
        b.sel = Sel::point(at);
        b.clamp();
    }
}

/// The pair a surround char stands for, open and close.
pub fn pair(c: char) -> (char, char) {
    match c {
        '(' | ')' => ('(', ')'),
        '[' | ']' => ('[', ']'),
        '{' | '}' => ('{', '}'),
        '<' | '>' => ('<', '>'),
        c => (c, c),
    }
}

/// Puts the pair for `c` around the selection, and selects it with them.
pub fn surround_add(b: &mut Buffer, c: char) {
    let (open, close) = pair(c);
    let (from, to) = (b.sel.from(), b.sel.to());
    b.insert((to + 1).min(b.len()), &close.to_string());
    b.insert(from, &open.to_string());
    b.sel = Sel::new(from, to + 2);
    b.clamp();
}

/// Where the pair for `c` closest around the selection is, open and close.
pub fn surrounding(b: &Buffer, c: char) -> Option<(usize, usize)> {
    let (open, close) = pair(c);
    let (from, to) = (b.sel.from(), b.sel.to());
    if open == close {
        let before = (0..from).rev().find(|&i| b.char(i) == Some(open))?;
        let after = (to + 1..b.len()).find(|&i| b.char(i) == Some(close))?;
        return Some((before, after));
    }
    let mut depth = 0;
    let mut start = None;
    for i in (0..from).rev() {
        match b.char(i) {
            Some(x) if x == close => depth += 1,
            Some(x) if x == open && depth == 0 => {
                start = Some(i);
                break;
            }
            Some(x) if x == open => depth -= 1,
            _ => {}
        }
    }
    let start = start?;
    let mut depth = 0;
    for i in start + 1..b.len() {
        match b.char(i) {
            Some(x) if x == open => depth += 1,
            Some(x) if x == close && depth == 0 => {
                return (i > to || i >= from).then_some((start, i));
            }
            Some(x) if x == close => depth -= 1,
            _ => {}
        }
    }
    None
}

/// Takes off the pair for `c` around the selection. Returns whether there was one.
pub fn surround_delete(b: &mut Buffer, c: char) -> bool {
    let Some((open, close)) = surrounding(b, c) else {
        return false;
    };
    b.remove(close, close + 1);
    b.remove(open, open + 1);
    b.sel = Sel::new(b.sel.anchor.saturating_sub(1), b.sel.head.saturating_sub(1));
    b.clamp();
    true
}

/// Swaps the pair for `from` around the selection for the pair for `to`.
pub fn surround_replace(b: &mut Buffer, from: char, to: char) -> bool {
    let Some((open, close)) = surrounding(b, from) else {
        return false;
    };
    let (o, c) = pair(to);
    b.remove(close, close + 1);
    b.insert(close, &c.to_string());
    b.remove(open, open + 1);
    b.insert(open, &o.to_string());
    true
}
