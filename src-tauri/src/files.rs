//! A workspace's working-copy files, for the file tree and the editor.

use std::path::{Component, Path, PathBuf};

use anyhow::{Context, bail};
use serde::Serialize;
use specta::Type;

/// Past this the editor won't open a file.
const MAX_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    /// Relative to the workspace root, `/`-separated.
    pub path: String,
    pub dir: bool,
}

/// `rel` under `root`, refusing paths that leave it or reach into `.jj` or `.git`.
pub fn resolve(root: &Path, rel: &str) -> anyhow::Result<PathBuf> {
    let rel = Path::new(rel);
    let mut parts = rel.components();
    if !parts.all(|c| matches!(c, Component::Normal(_))) {
        bail!("{} isn't a path in the workspace", rel.display());
    }
    if rel.starts_with(".jj") || rel.starts_with(".git") {
        bail!("{} is jj's or git's", rel.display());
    }
    Ok(root.join(rel))
}

/// The entries of directory `dir` (empty for the root), directories first, skipping what
/// `.gitignore` files ignore.
pub fn list(root: &Path, dir: &str) -> anyhow::Result<Vec<FileEntry>> {
    let path = if dir.is_empty() {
        root.to_path_buf()
    } else {
        resolve(root, dir)?
    };
    let walk = ignore::WalkBuilder::new(&path)
        .max_depth(Some(1))
        .hidden(false)
        .require_git(false)
        .filter_entry(|e| e.file_name() != ".jj" && e.file_name() != ".git")
        .build();
    let mut out = Vec::new();
    for entry in walk.flatten().filter(|e| e.depth() == 1) {
        let Ok(rel) = entry.path().strip_prefix(root) else {
            continue;
        };
        out.push(FileEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            path: rel.to_string_lossy().replace('\\', "/"),
            dir: entry.file_type().is_some_and(|t| t.is_dir()),
        });
    }
    out.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.name.cmp(&b.name)));
    Ok(out)
}

/// A text file's contents, named `name` in errors.
pub fn read(path: &Path, name: &str) -> anyhow::Result<String> {
    let len = std::fs::metadata(path)
        .with_context(|| format!("can't read {name}"))?
        .len();
    if len > MAX_BYTES {
        bail!("{name} is too big to edit ({} MB)", len / (1024 * 1024));
    }
    let bytes = std::fs::read(path).with_context(|| format!("can't read {name}"))?;
    String::from_utf8(bytes).map_err(|_| anyhow::anyhow!("{name} isn't UTF-8 text"))
}

/// Overwrites a file that exists, keeping its permissions. `name` is for errors.
pub fn write(path: &Path, name: &str, text: &str) -> anyhow::Result<()> {
    if !path.is_file() {
        bail!("{name} is gone");
    }
    std::fs::write(path, text).with_context(|| format!("can't save {name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_directories_first_without_ignored_files_or_jj() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for d in [".jj/repo", "src", "target"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        for f in [".gitignore", "b.txt", "a.log", "src/main.rs", "target/x"] {
            std::fs::write(root.join(f), "/target\n*.log\n").unwrap();
        }
        let names = |dir| {
            list(root, dir)
                .unwrap()
                .into_iter()
                .map(|e| e.path)
                .collect::<Vec<_>>()
        };
        assert_eq!(names(""), ["src", ".gitignore", "b.txt"]);
        assert_eq!(names("src"), ["src/main.rs"]);
    }

    #[test]
    fn refuses_paths_outside_the_workspace() {
        let dir = tempfile::tempdir().unwrap();
        for bad in ["../x", "/etc/hosts", "a/../../x", ".jj/repo", ".git/config"] {
            assert!(resolve(dir.path(), bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn saves_only_files_that_exist_and_reads_only_text() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert!(write(&root.join("new.txt"), "new.txt", "x").is_err());
        let a = root.join("a.txt");
        std::fs::write(&a, "old").unwrap();
        write(&a, "a.txt", "new").unwrap();
        assert_eq!(read(&a, "a.txt").unwrap(), "new");
        std::fs::write(root.join("bin"), [0xff, 0xfe]).unwrap();
        assert!(read(&root.join("bin"), "bin").is_err());
    }
}
