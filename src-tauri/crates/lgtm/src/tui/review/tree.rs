//! The file list as a tree, in path order like the diff, with a chain of directories that hold
//! only one directory on one line.

use std::collections::BTreeMap;

/// A line of the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    Dir {
        name: String,
        depth: usize,
    },
    /// A review file by index, and its name.
    File {
        file: usize,
        name: String,
        depth: usize,
    },
}

#[derive(Default)]
struct Node {
    dirs: BTreeMap<String, Node>,
    files: Vec<(String, usize)>,
}

/// The tree of `paths`, each a review file in order.
pub fn tree<'a>(paths: impl IntoIterator<Item = &'a str>) -> Vec<Entry> {
    let mut root = Node::default();
    for (i, path) in paths.into_iter().enumerate() {
        let mut parts: Vec<&str> = path.split('/').collect();
        let name = parts.pop().unwrap_or_default();
        let mut node = &mut root;
        for dir in parts {
            node = node.dirs.entry(dir.to_string()).or_default();
        }
        node.files.push((name.to_string(), i));
    }
    let mut out = Vec::new();
    emit(&root, 0, &mut out);
    out
}

fn emit(node: &Node, depth: usize, out: &mut Vec<Entry>) {
    // Directories and files by name, together, as jj orders paths.
    let mut names: Vec<(&str, Option<&Node>, usize)> = node
        .dirs
        .iter()
        .map(|(name, dir)| (name.as_str(), Some(dir), 0))
        .chain(node.files.iter().map(|(name, i)| (name.as_str(), None, *i)))
        .collect();
    names.sort_by_key(|&(name, _, _)| name);
    for (name, dir, file) in names {
        let Some(mut dir) = dir else {
            out.push(Entry::File {
                file,
                name: name.to_string(),
                depth,
            });
            continue;
        };
        let mut name = name.to_string();
        while dir.files.is_empty() && dir.dirs.len() == 1 {
            let (next, inner) = dir.dirs.iter().next().expect("one dir");
            name = format!("{name}/{next}");
            dir = inner;
        }
        out.push(Entry::Dir { name, depth });
        emit(dir, depth + 1, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_go_in_path_order_and_single_directory_chains_join() {
        let got = tree([
            "src/tui/review/a.rs",
            "README.md",
            "src/tui/review/b.rs",
            "src/main.rs",
        ]);
        let dir = |name: &str, depth| Entry::Dir {
            name: name.into(),
            depth,
        };
        let file = |file, name: &str, depth| Entry::File {
            file,
            name: name.into(),
            depth,
        };
        assert_eq!(
            got,
            [
                file(1, "README.md", 0),
                dir("src", 0),
                file(3, "main.rs", 1),
                dir("tui/review", 1),
                file(0, "a.rs", 2),
                file(2, "b.rs", 2),
            ]
        );
    }
}
