//! The file list as a tree, in path order like the diff, with a chain of directories that hold
//! only one directory on one line.

use std::collections::BTreeMap;

/// A line of the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// A directory: what it shows as, its path, and how deep it is.
    Dir {
        name: String,
        path: String,
        depth: usize,
    },
    /// A review file by index, and its name.
    File {
        file: usize,
        name: String,
        depth: usize,
    },
    /// The heading over reviewed files, set apart, with how many there are.
    Reviewed { count: usize, open: bool },
}

#[derive(Default)]
struct Node {
    dirs: BTreeMap<String, Node>,
    files: Vec<(String, usize)>,
}

/// The tree of review files, each by index with its path, `depth` deep.
pub fn tree<'a>(files: impl IntoIterator<Item = (usize, &'a str)>, depth: usize) -> Vec<Entry> {
    let mut root = Node::default();
    for (i, path) in files {
        let mut parts: Vec<&str> = path.split('/').collect();
        let name = parts.pop().unwrap_or_default();
        let mut node = &mut root;
        for dir in parts {
            node = node.dirs.entry(dir.to_string()).or_default();
        }
        node.files.push((name.to_string(), i));
    }
    let mut out = Vec::new();
    emit(&root, "", depth, &mut out);
    out
}

/// Whether `path` is inside the directory `dir`.
pub fn inside(path: &str, dir: &str) -> bool {
    path.strip_prefix(dir)
        .is_some_and(|rest| rest.starts_with('/'))
}

fn emit(node: &Node, parent: &str, depth: usize, out: &mut Vec<Entry>) {
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
        let path = if parent.is_empty() {
            name.clone()
        } else {
            format!("{parent}/{name}")
        };
        out.push(Entry::Dir {
            name,
            path: path.clone(),
            depth,
        });
        emit(dir, &path, depth + 1, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_go_in_path_order_and_single_directory_chains_join() {
        let paths = [
            "src/tui/review/a.rs",
            "README.md",
            "src/tui/review/b.rs",
            "src/main.rs",
        ];
        let got = tree(paths.into_iter().enumerate(), 0);
        let dir = |name: &str, path: &str, depth| Entry::Dir {
            name: name.into(),
            path: path.into(),
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
                dir("src", "src", 0),
                file(3, "main.rs", 1),
                dir("tui/review", "src/tui/review", 1),
                file(0, "a.rs", 2),
                file(2, "b.rs", 2),
            ]
        );
        assert!(inside("src/tui/a.rs", "src"));
        assert!(!inside("srcs/a.rs", "src"));
    }
}
