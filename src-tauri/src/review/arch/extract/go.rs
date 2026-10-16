//! Go: import paths.

use tree_sitter::Node;

use super::{Raw, Spec, line, text, unquote, walk};

pub(super) fn imports(root: Node, source: &str, out: &mut Vec<Raw>) {
    walk(root, |node| {
        if node.kind() != "import_spec" {
            // Imports sit at the top level, so function bodies needn't be walked.
            return !matches!(node.kind(), "function_declaration" | "method_declaration");
        }
        if let Some(path) = node.child_by_field_name("path") {
            out.push(Raw {
                spec: Spec::Go(unquote(text(path, source)).to_string()),
                line: line(node),
            });
        }
        false
    });
}
