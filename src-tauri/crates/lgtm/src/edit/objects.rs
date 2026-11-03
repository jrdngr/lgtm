//! Text objects for `mi` and `ma`, and growing or shrinking a selection to syntax nodes.

use lgtm_core::syntax::{self, Parsed};
use tree_sitter::Node;

use super::buffer::{Buffer, Sel};
use super::motion::{Class, class};
use super::ops;

/// The buffer's syntax tree, when its language has a grammar.
pub fn parse(b: &Buffer) -> Option<Parsed> {
    syntax::parse(&b.name, &b.text.to_string())
}

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

fn is_function(kind: &str) -> bool {
    (kind.contains("function") || kind.contains("method"))
        && !kind.contains("call")
        && !kind.ends_with("_type")
        && !kind.ends_with("signature")
}

/// The function the selection is in: all of it, or inside its body's braces.
fn function(b: &Buffer, around: bool) -> Option<(usize, usize)> {
    let parsed = parse(b)?;
    let (start, end) = byte_range(b, b.sel);
    let mut node = parsed.root().descendant_for_byte_range(start, end)?;
    while !is_function(node.kind()) {
        node = node.parent()?;
    }
    if around {
        return Some(char_range(b, node));
    }
    let body = node.child_by_field_name("body")?;
    let (from, to) = char_range(b, body);
    let braced = b.char(from) == Some('{') && b.char(to) == Some('}');
    Some(if braced && to > from + 1 {
        (from + 1, to - 1)
    } else {
        (from, to)
    })
}

/// The selection's bytes, `end` past it.
fn byte_range(b: &Buffer, sel: Sel) -> (usize, usize) {
    let text = &b.text;
    let start = text.char_to_byte(sel.from().min(b.len()));
    let end = text.char_to_byte((sel.to() + 1).min(b.len()));
    (start, end)
}

/// A node's chars, `to` included.
fn char_range(b: &Buffer, node: Node) -> (usize, usize) {
    let from = b.text.byte_to_char(node.start_byte());
    let to = b.text.byte_to_char(node.end_byte()).saturating_sub(1);
    (from, to.max(from))
}

/// The smallest named node bigger than the selection, for `Alt-o`.
pub fn grow(b: &Buffer) -> Option<(usize, usize)> {
    let parsed = parse(b)?;
    let (start, end) = byte_range(b, b.sel);
    let mut node = parsed.root().named_descendant_for_byte_range(start, end)?;
    loop {
        let (s, e) = (node.start_byte(), node.end_byte());
        if (s < start || e > end) && s <= start && e >= end {
            return Some(char_range(b, node));
        }
        node = node.parent()?;
    }
}

/// The first named node inside the selection that's smaller than it, for `Alt-i` with no
/// grown selection to go back to.
pub fn shrink(b: &Buffer) -> Option<(usize, usize)> {
    let parsed = parse(b)?;
    let (start, end) = byte_range(b, b.sel);
    let mut node = parsed.root().named_descendant_for_byte_range(start, end)?;
    // The node that is the selection, or the smallest that covers it.
    while node.start_byte() > start || node.end_byte() < end {
        node = node.parent()?;
    }
    let mut cursor = node.walk();
    let child = node
        .named_children(&mut cursor)
        .find(|c| c.end_byte() - c.start_byte() < end - start)?;
    Some(char_range(b, child))
}
