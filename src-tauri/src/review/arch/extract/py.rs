//! Python: `import a.b` and `from .a import b`.

use tree_sitter::Node;

use super::{Raw, Spec, line, text, walk};

pub(super) fn imports(root: Node, source: &str, out: &mut Vec<Raw>) {
    walk(root, |node| {
        match node.kind() {
            "import_statement" => {
                let mut cursor = node.walk();
                for name in node.children_by_field_name("name", &mut cursor) {
                    out.push(Raw {
                        spec: Spec::Py {
                            level: 0,
                            module: dotted(module_of(name), source),
                            names: vec![],
                        },
                        line: line(node),
                    });
                }
            }
            "import_from_statement" => {
                let Some(module) = node.child_by_field_name("module_name") else {
                    return false;
                };
                let (level, module) = if module.kind() == "relative_import" {
                    let t = text(module, source);
                    let level = t.chars().take_while(|&c| c == '.').count() as u32;
                    let rest = t.trim_start_matches('.');
                    (
                        level,
                        rest.split('.')
                            .filter(|s| !s.is_empty())
                            .map(str::to_string)
                            .collect(),
                    )
                } else {
                    (0, dotted(module, source))
                };
                let mut cursor = node.walk();
                let names = node
                    .children_by_field_name("name", &mut cursor)
                    .map(|n| text(module_of(n), source).to_string())
                    .collect();
                out.push(Raw {
                    spec: Spec::Py {
                        level,
                        module,
                        names,
                    },
                    line: line(node),
                });
            }
            _ => return true,
        }
        false
    });
}

/// An `a as b` clause's module.
fn module_of(node: Node) -> Node {
    match node.kind() {
        "aliased_import" => node.child_by_field_name("name").unwrap_or(node),
        _ => node,
    }
}

fn dotted(node: Node, source: &str) -> Vec<String> {
    text(node, source)
        .split('.')
        .map(|s| s.trim().to_string())
        .collect()
}
