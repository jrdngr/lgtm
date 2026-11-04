//! Stepping between syntax items: the outermost node a line starts, and the next or previous
//! one at its level. Clauses like `except`, `elif` or `finally`, which start a line at their
//! statement's indent, are steps of their own between the statement and what follows it.

use tree_sitter::{Node, Point};

use super::parse;

/// For each line from 0, the line its item steps to, if any.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Items {
    pub next: Vec<Option<u32>>,
    pub prev: Vec<Option<u32>>,
}

/// The steps from each line of `source`, none when `path` has no grammar.
pub fn items(path: &str, source: &str) -> Items {
    let Some(parsed) = parse(path, source) else {
        return Items::default();
    };
    let firsts: Vec<Option<usize>> = source
        .lines()
        .map(|l| l.find(|c: char| !c.is_whitespace()))
        .collect();
    let lines = Lines(&firsts);
    let root = parsed.root();
    let mut out = Items::default();
    for (row, first) in firsts.iter().enumerate() {
        let col = first.unwrap_or(0);
        let at = Point::new(row, col);
        let item = root
            .descendant_for_point_range(at, at)
            .and_then(|n| lines.item_of(n));
        let line = |n: Option<Node>| n.map(|n| n.start_position().row as u32);
        out.next.push(line(item.and_then(|i| lines.next(i))));
        out.prev.push(line(item.and_then(|i| lines.prev(i))));
    }
    out
}

/// Whether `n` holds a run of statements or members, like a block or a class body, rather
/// than being one.
fn holds_lines(n: Node) -> bool {
    let k = n.kind();
    k.contains("block") || k.ends_with("_list") || k.ends_with("body")
}

/// Each line's first non-blank byte.
struct Lines<'a>(&'a [Option<usize>]);

impl Lines<'_> {
    /// Whether `n` starts its line, at its first non-blank char.
    fn starts_line(&self, n: Node) -> bool {
        let p = n.start_position();
        self.0.get(p.row).copied().flatten() == Some(p.column)
    }

    /// The item for a node at a line's start: the outermost named node starting there, or the
    /// nearest enclosing node that starts a line.
    fn item_of<'t>(&self, mut n: Node<'t>) -> Option<Node<'t>> {
        let start = n.start_position();
        while let Some(p) = n.parent().filter(|p| p.parent().is_some()) {
            let item = n.is_named() && self.starts_line(n);
            if (p.start_position() == start && !(item && holds_lines(p))) || !item {
                n = p;
            } else {
                break;
            }
        }
        (n.parent().is_some() && n.is_named() && self.starts_line(n)).then_some(n)
    }

    /// Whether `n` is an item at `col`: named, not a comment, and starting a line there.
    fn level(&self, n: Node, col: usize) -> bool {
        n.is_named()
            && !n.kind().contains("comment")
            && self.starts_line(n)
            && n.start_position().column == col
    }

    /// `n`'s clauses: its children that start later lines at its own indent.
    fn clauses<'t>(&self, n: Node<'t>) -> Vec<Node<'t>> {
        let col = n.start_position().column;
        let row = n.start_position().row;
        let mut c = n.walk();
        n.named_children(&mut c)
            .filter(|k| k.start_position().row > row && self.level(*k, col))
            .collect()
    }

    /// The statement `n` is a clause of.
    fn owner<'t>(&self, n: Node<'t>) -> Option<Node<'t>> {
        let p = n.parent()?;
        let col = n.start_position().column;
        (p.parent().is_some()
            && self.starts_line(p)
            && p.start_position().column == col
            && p.start_position().row < n.start_position().row)
            .then_some(p)
    }

    fn sibling<'t>(&self, n: Node<'t>, forward: bool) -> Option<Node<'t>> {
        let col = n.start_position().column;
        let mut s = n;
        loop {
            s = if forward {
                s.next_named_sibling()?
            } else {
                s.prev_named_sibling()?
            };
            if self.level(s, col) {
                return Some(s);
            }
        }
    }

    fn next<'t>(&self, n: Node<'t>) -> Option<Node<'t>> {
        if let Some(first) = self.clauses(n).first() {
            return Some(*first);
        }
        let mut at = n;
        loop {
            if let Some(s) = self.sibling(at, true) {
                return Some(s);
            }
            at = self.owner(at)?;
        }
    }

    fn prev<'t>(&self, n: Node<'t>) -> Option<Node<'t>> {
        match self.sibling(n, false) {
            Some(s) => Some(self.clauses(s).last().copied().unwrap_or(s)),
            None => self.owner(n),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steps(path: &str, source: &str) -> Vec<(Option<u32>, Option<u32>)> {
        let i = items(path, source);
        i.next.into_iter().zip(i.prev).collect()
    }

    #[test]
    fn python_steps_through_top_level_items_and_a_tries_clauses() {
        let src = "\
import os

def a():
    try:
        x()
    except E:
        y()
    finally:
        z()
    w()

# note
def b():
    pass
";
        let s = steps("a.py", src);
        // import → def a → def b, skipping the comment.
        assert_eq!(s[0], (Some(2), None));
        assert_eq!(s[2], (Some(12), Some(0)));
        assert_eq!(s[12], (None, Some(2)));
        // try → except → finally → w(), and back.
        assert_eq!(s[3], (Some(5), None));
        assert_eq!(s[5], (Some(7), Some(3)));
        assert_eq!(s[7], (Some(9), Some(5)));
        assert_eq!(s[9], (None, Some(7)));
        // A line inside the try's body steps within the body.
        assert_eq!(s[4], (None, None));
        // A blank line has no item.
        assert_eq!(s[1], (None, None));
    }

    #[test]
    fn rust_steps_between_items_and_from_a_closing_brace_line_to_nothing() {
        let src = "\
fn a() {
    let x = 1;
    let y = 2;
}

struct S;
";
        let s = steps("a.rs", src);
        assert_eq!(s[0], (Some(5), None));
        assert_eq!(s[1], (Some(2), None));
        assert_eq!(s[2], (None, Some(1)));
        assert_eq!(s[5], (None, Some(0)));
    }

    #[test]
    fn a_file_without_a_grammar_has_no_steps() {
        assert_eq!(items("a.unknown", "x\ny\n"), Items::default());
    }
}
