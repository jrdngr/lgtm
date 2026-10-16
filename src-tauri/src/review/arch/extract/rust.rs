//! Rust: `use` trees and expression paths that start with `crate`, `super` or `self`.

use tree_sitter::Node;

use super::{Raw, Spec, line, text, walk};

const ANCHORS: &[&str] = &["crate", "super", "self"];

pub(super) fn imports(root: Node, source: &str, out: &mut Vec<Raw>) {
    walk(root, |node| match node.kind() {
        "use_declaration" => {
            if let Some(arg) = node.child_by_field_name("argument") {
                let mut paths = Vec::new();
                expand(arg, source, &[], &mut paths);
                for (segs, at) in paths {
                    push(segs, at, out);
                }
            }
            false
        }
        // `mod` declarations name the module tree, not a dependency.
        "mod_item" => node.child_by_field_name("body").is_some(),
        "scoped_identifier" | "scoped_type_identifier" => {
            push(segments(text(node, source)), line(node), out);
            false
        }
        _ => true,
    });
}

fn push(segs: Vec<String>, line: u32, out: &mut Vec<Raw>) {
    if segs.first().is_some_and(|s| ANCHORS.contains(&s.as_str())) {
        out.push(Raw {
            spec: Spec::Rust(segs),
            line,
        });
    }
}

fn segments(path: &str) -> Vec<String> {
    path.split("::")
        .map(|s| s.split_whitespace().collect::<String>())
        .filter(|s| !s.is_empty())
        .collect()
}

/// The full paths in a use tree, each with its line.
fn expand(node: Node, source: &str, prefix: &[String], out: &mut Vec<(Vec<String>, u32)>) {
    let with = |segs: Vec<String>| [prefix, &segs].concat();
    match node.kind() {
        "use_as_clause" => {
            if let Some(path) = node.child_by_field_name("path") {
                expand(path, source, prefix, out);
            }
        }
        "use_wildcard" => {
            let t = text(node, source);
            out.push((with(segments(t.trim_end_matches('*'))), line(node)));
        }
        "scoped_use_list" => {
            let base = node
                .child_by_field_name("path")
                .map_or(vec![], |p| segments(text(p, source)));
            if let Some(list) = node.child_by_field_name("list") {
                expand(list, source, &with(base), out);
            }
        }
        "use_list" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                expand(child, source, prefix, out);
            }
        }
        // `self` in a list names the list's own path.
        "self" if !prefix.is_empty() => out.push((prefix.to_vec(), line(node))),
        "line_comment" | "block_comment" => {}
        _ => out.push((with(segments(text(node, source))), line(node))),
    }
}
