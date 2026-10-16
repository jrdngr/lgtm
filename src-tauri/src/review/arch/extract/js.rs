//! TS and JS: imports, re-exports, `require` and dynamic `import` with relative specifiers.

use tree_sitter::Node;

use super::{Raw, Spec, line, text, unquote, walk};

pub(super) fn imports(root: Node, source: &str, out: &mut Vec<Raw>) {
    walk(root, |node| {
        let spec = match node.kind() {
            "import_statement" | "export_statement" => node.child_by_field_name("source"),
            "call_expression" => {
                let callee = node.child_by_field_name("function");
                let is_load = callee.is_some_and(|f| {
                    f.kind() == "import"
                        || (f.kind() == "identifier" && text(f, source) == "require")
                });
                node.child_by_field_name("arguments")
                    .filter(|_| is_load)
                    .and_then(|a| a.named_child(0))
                    .filter(|a| a.kind() == "string")
            }
            _ => None,
        };
        if let Some(s) = spec {
            let s = unquote(text(s, source));
            if s == "." || s == ".." || s.starts_with("./") || s.starts_with("../") {
                out.push(Raw {
                    spec: Spec::Js(s.to_string()),
                    line: line(node),
                });
            }
        }
        true
    });
}
