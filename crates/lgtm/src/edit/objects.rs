//! Text objects for `mi` and `ma`, and growing or shrinking a selection to syntax nodes.

use lgtm_core::syntax::nodes;

use super::buffer::{Buffer, Sel};
use super::motion::{Class, class};
use super::ops;

/// The chars of the object `c` around the selection, inside or around it, as `(from, to)`
/// with `to` included.
pub fn object(b: &Buffer, c: char, around: bool) -> Option<(usize, usize)> {
    match c {
        'w' | 'W' => word(b, c == 'W', around),
        'f' => function(b, around),
        '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | '"' | '\'' | '`' => {
            let (open, close) = ops::surrounding(b, c)?;
            if around {
                Some((open, close))
            } else if close > open + 1 {
                Some((open + 1, close - 1))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// The word under the cursor, and with `around` the spaces after it, or before it when
/// none follow.
fn word(b: &Buffer, big: bool, around: bool) -> Option<(usize, usize)> {
    let at = b.sel.head;
    let kind = |i: usize| {
        let k = class(b.char(i)?);
        Some(if big && k == Class::Punct {
            Class::Word
        } else {
            k
        })
    };
    let here = kind(at)?;
    if here == Class::Eol {
        return None;
    }
    let (mut from, mut to) = (at, at);
    while from > 0 && kind(from - 1) == Some(here) {
        from -= 1;
    }
    while kind(to + 1) == Some(here) {
        to += 1;
    }
    if around {
        let mut end = to;
        while kind(end + 1) == Some(Class::Space) {
            end += 1;
        }
        if end > to {
            to = end;
        } else {
            while from > 0 && kind(from - 1) == Some(Class::Space) {
                from -= 1;
            }
        }
    }
    Some((from, to))
}

/// The function the selection is in: all of it, or inside its body's braces.
fn function(b: &Buffer, around: bool) -> Option<(usize, usize)> {
    let (start, end) = byte_range(b, b.sel);
    let text = b.text.to_string();
    nodes::function(&b.name, &text, start, end, around).map(|r| char_range(b, r))
}

/// The selection's bytes, `end` past it.
fn byte_range(b: &Buffer, sel: Sel) -> (usize, usize) {
    let text = &b.text;
    let start = text.char_to_byte(sel.from().min(b.len()));
    let end = text.char_to_byte((sel.to() + 1).min(b.len()));
    (start, end)
}

/// Bytes `start..end` as chars, `to` included.
fn char_range(b: &Buffer, (start, end): (usize, usize)) -> (usize, usize) {
    let from = b.text.byte_to_char(start);
    let to = b.text.byte_to_char(end).saturating_sub(1);
    (from, to.max(from))
}

/// The smallest named node bigger than the selection, for `Alt-o`.
pub fn grow(b: &Buffer) -> Option<(usize, usize)> {
    let (start, end) = byte_range(b, b.sel);
    nodes::grow(&b.name, &b.text.to_string(), start, end).map(|r| char_range(b, r))
}

/// The first named node inside the selection that's smaller than it, for `Alt-i` with no
/// grown selection to go back to.
pub fn shrink(b: &Buffer) -> Option<(usize, usize)> {
    let (start, end) = byte_range(b, b.sel);
    nodes::shrink(&b.name, &b.text.to_string(), start, end).map(|r| char_range(b, r))
}
