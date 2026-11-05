//! The files under a root, read from disk. Files git would ignore are listed apart, and
//! ignored folders are read only when asked for, so `target/` or `node_modules/` cost nothing
//! until they're opened.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

/// The most files a walk lists.
const MAX_FILES: usize = 100_000;

/// What's under a root, as paths from it with `/` between parts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Listing {
    pub root: PathBuf,
    pub files: Vec<String>,
    /// Ignored files in folders that aren't ignored.
    pub ignored: Vec<String>,
    /// Ignored folders in folders that aren't ignored, not read.
    pub ignored_dirs: Vec<String>,
}

/// Folders never listed.
fn hidden(name: &std::ffi::OsStr) -> bool {
    name == ".git" || name == ".jj"
}

fn rel(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?.to_string_lossy().into_owned();
    (!rel.is_empty()).then_some(rel)
}

/// Whether `path` is a folder, through a symlink.
fn is_dir(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_dir())
}

fn walker(root: &Path, ignores: bool) -> ignore::Walk {
    WalkBuilder::new(root)
        .standard_filters(ignores)
        .hidden(false)
        .ignore(false)
        .require_git(false)
        .filter_entry(|e| !hidden(e.file_name()))
        .build()
}

/// The files under `root`, with what git ignores apart.
pub fn list(root: &Path) -> Listing {
    let mut files = Vec::new();
    let mut dirs = vec![root.to_path_buf()];
    for entry in walker(root, true).flatten() {
        let Some(path) = rel(root, entry.path()) else {
            continue;
        };
        if entry.file_type().is_some_and(|t| t.is_dir()) {
            dirs.push(entry.into_path());
        } else {
            files.push(path);
            if files.len() >= MAX_FILES {
                break;
            }
        }
    }
    let kept: HashSet<&Path> = dirs.iter().map(PathBuf::as_path).collect();
    let known: HashSet<&str> = files.iter().map(String::as_str).collect();
    let (mut ignored, mut ignored_dirs) = (Vec::new(), Vec::new());
    for dir in &dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if hidden(&entry.file_name()) || kept.contains(path.as_path()) {
                continue;
            }
            let Some(name) = rel(root, &path) else {
                continue;
            };
            if is_dir(&path) {
                ignored_dirs.push(name);
            } else if !known.contains(name.as_str()) {
                ignored.push(name);
            }
        }
    }
    Listing {
        root: root.to_path_buf(),
        files,
        ignored,
        ignored_dirs,
    }
}

/// The files and folders right inside the folder `dir` under `root`.
pub fn read_dir(root: &Path, dir: &str) -> (Vec<String>, Vec<String>) {
    let (mut files, mut dirs) = (Vec::new(), Vec::new());
    let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
        return (files, dirs);
    };
    for entry in entries.flatten() {
        if hidden(&entry.file_name()) {
            continue;
        }
        let name = format!("{dir}/{}", entry.file_name().to_string_lossy());
        match is_dir(&entry.path()) {
            true => dirs.push(name),
            false => files.push(name),
        }
    }
    (files, dirs)
}

/// Every file under `root`, ignored or not.
pub fn everything(root: &Path) -> Vec<String> {
    walker(root, false)
        .flatten()
        .filter(|e| !e.file_type().is_some_and(|t| t.is_dir()))
        .filter_map(|e| rel(root, e.path()))
        .take(MAX_FILES)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignored_files_are_apart_and_ignored_folders_unread() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for (path, text) in [
            (".gitignore", "*.log\ntarget/\n"),
            ("src/a.rs", ""),
            ("src/debug.log", ""),
            ("target/debug/out", ""),
            (".jj/repo/x", ""),
            (".git/HEAD", ""),
        ] {
            std::fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
            std::fs::write(root.join(path), text).unwrap();
        }
        let mut l = list(root);
        l.files.sort();
        assert_eq!(l.files, [".gitignore", "src/a.rs"]);
        assert_eq!(l.ignored, ["src/debug.log"]);
        assert_eq!(l.ignored_dirs, ["target"]);
        assert_eq!(
            read_dir(root, "target"),
            (vec![], vec!["target/debug".into()])
        );
        let mut all = everything(root);
        all.sort();
        assert_eq!(
            all,
            [
                ".gitignore",
                "src/a.rs",
                "src/debug.log",
                "target/debug/out"
            ]
        );
    }
}
