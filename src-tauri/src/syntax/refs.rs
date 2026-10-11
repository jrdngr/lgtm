//! Identifiers a file mentions, for linking symbols by name.

use tree_sitter::TreeCursor;

use super::Parsed;

/// A name used in the file and the 1-based line it's on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ref {
    pub name: String,
    pub line: u32,
    /// Called or invoked right here, like `name(` or `name!(`.
    pub call: bool,
}

impl Parsed {
    /// Every identifier in the file, in order. Grammars name them `identifier`,
    /// `type_identifier`, `field_identifier` and so on.
    pub fn refs(&self, source: &str) -> Vec<Ref> {
        let mut out = Vec::new();
        let mut cursor = self.tree.walk();
        walk(&mut cursor, source, &mut out);
        out
    }
}

fn walk(cursor: &mut TreeCursor, source: &str, out: &mut Vec<Ref>) {
    loop {
        let node = cursor.node();
        if node.child_count() == 0 {
            if node.is_named() && node.kind().ends_with("identifier") {
                let after = &source[node.end_byte()..];
                out.push(Ref {
                    name: source[node.byte_range()].to_string(),
                    line: node.start_position().row as u32 + 1,
                    call: after.starts_with(['(', '!']) || after.starts_with("::<"),
                });
            }
        } else if cursor.goto_first_child() {
            walk(cursor, source, out);
            cursor.goto_parent();
        }
        if !cursor.goto_next_sibling() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::syntax::parse;

    #[test]
    fn finds_calls_types_and_fields_with_lines() {
        let src = "fn a() {\n    b(S { x: 1 }.y);\n}\n";
        let refs = parse("a.rs", src).unwrap().refs(src);
        let names: Vec<(&str, bool)> = refs.iter().map(|r| (r.name.as_str(), r.call)).collect();
        assert_eq!(
            names,
            [("a", true), ("b", true), ("S", false), ("x", false), ("y", false)]
        );
        assert_eq!(refs[1].line, 2);
    }
}
