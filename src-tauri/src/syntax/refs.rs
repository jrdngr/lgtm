//! Identifiers a file mentions, for linking symbols by name.

use tree_sitter::{Node, TreeCursor};

use super::Parsed;

/// A name used in the file and the 1-based line it's on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ref {
    pub name: String,
    pub line: u32,
    /// Called or invoked right here, like `name(`, `name!(` or `name<T>(`.
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

/// Visits every node with one cursor, without recursion, so deep trees can't overflow the stack.
fn walk(cursor: &mut TreeCursor, source: &str, out: &mut Vec<Ref>) {
    // Ancestor kinds, since `Node::parent` searches down from the root.
    let mut parents: Vec<&str> = Vec::new();
    loop {
        let node = cursor.node();
        if node.child_count() == 0 {
            if let Some(call) = ref_at(node, &parents, source) {
                out.push(Ref {
                    name: source[node.byte_range()].to_string(),
                    line: node.start_position().row as u32 + 1,
                    call,
                });
            }
        } else if cursor.goto_first_child() {
            parents.push(node.kind());
            continue;
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return;
            }
            parents.pop();
        }
    }
}

/// Whether a leaf names something, and if so whether it's called right here: `f(`, a Rust
/// macro `m!(`, a turbofish `f::<T>(`, a TS generic `f<T>(`, or a shell command.
fn ref_at(node: Node, parents: &[&str], source: &str) -> Option<bool> {
    let parent = parents.last().copied();
    if !node.is_named() {
        return None;
    }
    if node.kind() == "word" && parent == Some("command_name") {
        return Some(true);
    }
    if !node.kind().ends_with("identifier") {
        return None;
    }
    let after = &source[node.end_byte()..];
    // A name directly under a call, or the property of a member under one, is its callee;
    // arguments sit in their own node.
    let callee = match parents {
        [.., call, "member_expression"] if node.kind() == "property_identifier" => Some(*call),
        _ => parent,
    };
    let generic_call =
        after.starts_with('<') && matches!(callee, Some("call_expression" | "new_expression"));
    Some(
        after.starts_with('(')
            || after
                .strip_prefix('!')
                .is_some_and(|a| a.starts_with(['(', '[', '{']))
            || after.starts_with("::<")
            || generic_call,
    )
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
            [
                ("a", true),
                ("b", true),
                ("S", false),
                ("x", false),
                ("y", false)
            ]
        );
        assert_eq!(refs[1].line, 2);
    }

    fn calls(path: &str, src: &str) -> Vec<(String, u32, bool)> {
        let refs = parse(path, src).unwrap().refs(src);
        refs.into_iter().map(|r| (r.name, r.line, r.call)).collect()
    }

    #[test]
    fn rust_macros_are_calls() {
        let src = "fn a() { v![1]; m!{}; }\n";
        assert_eq!(
            calls("a.rs", src),
            [
                ("a".into(), 1, true),
                ("v".into(), 1, true),
                ("m".into(), 1, true)
            ]
        );
    }

    #[test]
    fn ts_generic_calls_are_calls_and_non_null_is_not() {
        let src = "f<number>();\nnew C<T>();\nconst a = x!.y;\nReact.useState<string>();\n";
        let got = calls("a.ts", src);
        assert!(got.contains(&("useState".into(), 4, true)));
        assert!(got.contains(&("React".into(), 4, false)));
        assert!(got.contains(&("f".into(), 1, true)));
        assert!(got.contains(&("C".into(), 2, true)));
        assert!(got.contains(&("x".into(), 3, false)));
    }

    #[test]
    fn shell_commands_are_calls() {
        let src = "f() {\n  x\n}\ng() {\n  f\n}\n";
        let got = calls("a.sh", src);
        assert!(got.contains(&("x".into(), 2, true)));
        assert!(got.contains(&("f".into(), 5, true)));
    }

    #[test]
    fn deep_trees_dont_overflow_the_stack() {
        let src = format!("const s = a{};\n", " + a".repeat(30_000));
        let n = std::thread::Builder::new()
            .stack_size(2 << 20)
            .spawn(move || parse("a.js", &src).unwrap().refs(&src).len())
            .unwrap()
            .join()
            .unwrap();
        assert_eq!(n, 30_002);
    }
}
