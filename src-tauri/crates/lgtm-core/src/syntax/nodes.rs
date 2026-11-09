//! Syntax nodes for the editors: growing or shrinking a selection to a node, the function
//! around it, and the lines a fold can hide. Ranges are bytes, the end past the last.

use std::collections::BTreeMap;

use tree_sitter::Node;

use super::parse;

/// The smallest named node bigger than `start..end`, for `Alt-o`.
pub fn grow(path: &str, text: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    let parsed = parse(path, text)?;
    let mut node = parsed.root().named_descendant_for_byte_range(start, end)?;
    loop {
        let (s, e) = (node.start_byte(), node.end_byte());
        if (s < start || e > end) && s <= start && e >= end {
            return Some((s, e));
        }
        node = node.parent()?;
    }
}

/// The first named node inside `start..end` that's smaller than it, for `Alt-i` with no
/// grown selection to go back to.
pub fn shrink(path: &str, text: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    let parsed = parse(path, text)?;
    let mut node = parsed.root().named_descendant_for_byte_range(start, end)?;
    // The node that is the selection, or the smallest that covers it.
    while node.start_byte() > start || node.end_byte() < end {
        node = node.parent()?;
    }
    let mut cursor = node.walk();
    let child = node
        .named_children(&mut cursor)
        .find(|c| c.end_byte() - c.start_byte() < end - start)?;
    Some((child.start_byte(), child.end_byte()))
}

fn is_function(kind: &str) -> bool {
    (kind.contains("function") || kind.contains("method"))
        && !kind.contains("call")
        && !kind.ends_with("_type")
        && !kind.ends_with("signature")
}

/// The function `start..end` is in: all of it, or inside its body's braces.
pub fn function(
    path: &str,
    text: &str,
    start: usize,
    end: usize,
    around: bool,
) -> Option<(usize, usize)> {
    let parsed = parse(path, text)?;
    let mut node = parsed.root().descendant_for_byte_range(start, end)?;
    while !is_function(node.kind()) {
        node = node.parent()?;
    }
    if around {
        return Some((node.start_byte(), node.end_byte()));
    }
    let body = node.child_by_field_name("body")?;
    let (s, e) = (body.start_byte(), body.end_byte());
    let bytes = text.as_bytes();
    let braced = bytes.get(s) == Some(&b'{') && e > 0 && bytes.get(e - 1) == Some(&b'}');
    Some(if braced && e > s + 2 {
        (s + 1, e - 1)
    } else {
        (s, e)
    })
}

/// Foldable spans by start line, the longest for each: named nodes over two lines or more,
/// and runs of comments or imports. None when `path` has no grammar.
pub fn fold_spans(path: &str, text: &str) -> Option<BTreeMap<usize, usize>> {
    let parsed = parse(path, text)?;
    let mut ends = BTreeMap::new();
    let mut stack = vec![parsed.root()];
    while let Some(node) = stack.pop() {
        let mut cursor = node.walk();
        let children: Vec<Node> = node.named_children(&mut cursor).collect();
        let mut run: Option<(&str, usize, usize)> = None;
        for child in &children {
            let (start, end) = (child.start_position().row, last_row(*child));
            add_span(&mut ends, start, end);
            let kind = child.kind();
            let groups =
                kind.contains("comment") || kind.contains("import") || kind == "use_declaration";
            match run {
                Some((k, s, e)) if groups && k == kind && start == e + 1 => {
                    run = Some((k, s, end));
                }
                _ => {
                    if let Some((_, s, e)) = run {
                        add_span(&mut ends, s, e);
                    }
                    run = groups.then_some((kind, start, end));
                }
            }
            stack.push(*child);
        }
        if let Some((_, s, e)) = run {
            add_span(&mut ends, s, e);
        }
    }
    Some(ends)
}

/// Keeps the longest span for each start line.
pub fn add_span(ends: &mut BTreeMap<usize, usize>, start: usize, end: usize) {
    if end > start {
        let e = ends.entry(start).or_insert(end);
        *e = (*e).max(end);
    }
}

/// The node's last row, not counting a row it only ends at the start of.
fn last_row(node: Node) -> usize {
    let end = node.end_position();
    if end.column == 0 && end.row > node.start_position().row {
        end.row - 1
    } else {
        end.row
    }
}
