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

/// `path` relative to `root`, when it names a file there that a symlink doesn't lead out of.
/// It may be relative to `root`, with `./` or not, or absolute inside it.
pub fn workspace_file(root: &Path, path: &str) -> Option<String> {
    let path = path.strip_prefix("./").unwrap_or(path);
    let rel = match Path::new(path).strip_prefix(root) {
        Ok(rel) => rel.to_str()?.to_string(),
        Err(_) if Path::new(path).is_absolute() => return None,
        Err(_) => path.to_string(),
    };
    let abs = resolve(root, &rel).ok()?;
    (abs.is_file() && contain(root, &abs).is_ok()).then_some(rel)
}

/// The entries of directory `dir` (empty for the root), directories first, skipping what
/// `.gitignore` files in the workspace ignore. Ignore files above the root don't count, so a
/// workspace inside its repo's ignored `.workspaces/` still lists. The walk starts at the root
/// and follows only the way to `dir`, so ignore files between the two still apply.
pub fn list(root: &Path, dir: &str) -> anyhow::Result<Vec<FileEntry>> {
    let path = if dir.is_empty() {
        root.to_path_buf()
    } else {
        resolve(root, dir)?
    };
    let depth = Path::new(dir).components().count() + 1;
    let target = path.clone();
    let real_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let walk = ignore::WalkBuilder::new(root)
        .max_depth(Some(depth))
        .hidden(false)
        .parents(false)
        .require_git(false)
        .follow_links(true)
        .filter_entry(move |e| {
            let p = e.path();
            e.file_name() != ".jj"
                && e.file_name() != ".git"
                && (target.starts_with(p) || p.parent() == Some(target.as_path()))
                && !(e.path_is_symlink() && leaves(&real_root, p))
        })
        .build();
    let mut out = Vec::new();
    for entry in walk.flatten() {
        if entry.path().parent() != Some(path.as_path()) {
            continue;
        }
        let Ok(rel) = entry.path().strip_prefix(root) else {
            continue;
        };
        out.push(FileEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            path: rel.to_string_lossy().replace('\\', "/"),
            // Follows symlinks, so a link to a directory opens like one.
            dir: entry.path().is_dir(),
        });
    }
    out.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.name.cmp(&b.name)));
    Ok(out)
}

/// Whether `path` resolves outside `root`, which must be canonical. A dangling link doesn't.
fn leaves(root: &Path, path: &Path) -> bool {
    path.canonicalize()
        .is_ok_and(|real| !real.starts_with(root))
}

/// A text file's contents, named `name` in errors.
pub fn read(path: &Path, name: &str) -> anyhow::Result<String> {
    let bytes = read_bytes(path, name, MAX_BYTES)?;
    String::from_utf8(bytes).map_err(|_| anyhow::anyhow!("{name} isn't UTF-8 text"))
}

/// A file's bytes, up to `max` of them, named `name` in errors.
pub fn read_bytes(path: &Path, name: &str, max: u64) -> anyhow::Result<Vec<u8>> {
    let len = std::fs::metadata(path)
        .with_context(|| format!("can't read {name}"))?
        .len();
    if len > max {
        bail!("{name} is too big to open ({} MB)", len / (1024 * 1024));
    }
    std::fs::read(path).with_context(|| format!("can't read {name}"))
}

/// Overwrites a file that exists, keeping its permissions. `name` is for errors.
pub fn write(path: &Path, name: &str, text: &str) -> anyhow::Result<()> {
    if !path.is_file() {
        bail!("{name} is gone");
    }
    std::fs::write(path, text).with_context(|| format!("can't save {name}"))
}

/// Fails when `path`'s folder resolves outside `root` through a symlink, so creating, moving
/// or trashing can't reach past the workspace. `path` itself may be a symlink.
pub fn confine(root: &Path, path: &Path) -> anyhow::Result<()> {
    let root = root.canonicalize()?;
    let mut dir = path.parent();
    while let Some(d) = dir {
        if let Ok(real) = d.canonicalize() {
            if !real.starts_with(&root) {
                bail!("{} leads outside the workspace", path.display());
            }
            return Ok(());
        }
        dir = d.parent();
    }
    bail!("{} isn't in the workspace", path.display())
}

/// Like `confine`, but `path` itself, when it's a symlink, must also land inside `root`. For
/// reading and writing a file's contents.
pub fn contain(root: &Path, path: &Path) -> anyhow::Result<()> {
    if path.symlink_metadata().is_err() {
        return confine(root, path);
    }
    let real = path
        .canonicalize()
        .with_context(|| format!("{} leads nowhere", path.display()))?;
    if !real.starts_with(root.canonicalize()?) {
        bail!("{} leads outside the workspace", path.display());
    }
    Ok(())
}

/// Writes `bytes` to a new file, or over a file when `replace`. Returns false, writing
/// nothing, when something is there and it can't replace it. Refuses a symlink, since the
/// write would land wherever it points.
pub fn put(path: &Path, name: &str, bytes: &[u8], replace: bool) -> anyhow::Result<bool> {
    if path.is_dir() || (path.exists() && !replace) {
        return Ok(false);
    }
    if path
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink())
    {
        bail!("{name} is a symlink");
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("can't create {name}"))?;
    }
    std::fs::write(path, bytes).with_context(|| format!("can't write {name}"))?;
    Ok(true)
}

/// Makes an empty file, or a folder when `dir`, failing when something is already there.
pub fn create(path: &Path, name: &str, dir: bool) -> anyhow::Result<()> {
    if path.exists() {
        bail!("{name} already exists");
    }
    let made = if dir {
        std::fs::create_dir_all(path)
    } else {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| format!("can't create {name}"))?;
        }
        std::fs::File::create_new(path).map(drop)
    };
    made.with_context(|| format!("can't create {name}"))
}

/// Moves `from` to `to`, failing when `to` is taken. Names are for errors.
pub fn rename(from: &Path, to: &Path, to_name: &str) -> anyhow::Result<()> {
    if to.exists() {
        bail!("{to_name} already exists");
    }
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("can't move to {to_name}"))?;
    }
    std::fs::rename(from, to).with_context(|| format!("can't move to {to_name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_file_takes_relative_and_inside_absolute_paths_of_files_only() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/a.py"), "x").unwrap();
        let abs = root.join("src/a.py").to_string_lossy().into_owned();
        let found = |p: &str| workspace_file(root, p);
        assert_eq!(found("src/a.py").as_deref(), Some("src/a.py"));
        assert_eq!(found("./src/a.py").as_deref(), Some("src/a.py"));
        assert_eq!(found(&abs).as_deref(), Some("src/a.py"));
        assert_eq!(found("src"), None);
        assert_eq!(found("src/missing.py"), None);
        assert_eq!(found("../a.py"), None);
        assert_eq!(found("/etc/hosts"), None);
    }

    #[test]
    fn put_replaces_a_file_only_when_asked_and_never_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("new/a.png");
        assert!(put(&file, "a.png", b"one", false).unwrap());
        assert!(!put(&file, "a.png", b"two", false).unwrap());
        assert_eq!(std::fs::read(&file).unwrap(), b"one");
        assert!(put(&file, "a.png", b"two", true).unwrap());
        assert_eq!(std::fs::read(&file).unwrap(), b"two");
        assert!(!put(dir.path(), "new", b"x", true).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn put_refuses_symlinks_even_dangling_ones() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = dir.path();
        let rc = outside.path().join("rc");
        std::fs::write(&rc, "old").unwrap();
        std::os::unix::fs::symlink(&rc, root.join("a")).unwrap();
        std::os::unix::fs::symlink(outside.path().join("new"), root.join("b")).unwrap();
        assert!(put(&root.join("a"), "a", b"x", true).is_err());
        assert!(put(&root.join("b"), "b", b"x", false).is_err());
        assert_eq!(std::fs::read(&rc).unwrap(), b"old");
        assert!(!outside.path().join("new").exists());
    }

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
    fn lists_a_workspace_its_repo_ignores() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        let root = repo.join(".workspaces/ws");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(repo.join(".gitignore"), ".workspaces/**\n").unwrap();
        std::fs::write(root.join(".gitignore"), "*.log\n").unwrap();
        for f in ["src/main.rs", "src/debug.log"] {
            std::fs::write(root.join(f), "").unwrap();
        }
        let names = |dir| {
            list(&root, dir)
                .unwrap()
                .into_iter()
                .map(|e| e.path)
                .collect::<Vec<_>>()
        };
        assert_eq!(names(""), ["src", ".gitignore"]);
        assert_eq!(names("src"), ["src/main.rs"]);
    }

    #[cfg(unix)]
    #[test]
    fn lists_a_symlinked_directory_as_a_directory() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("shared")).unwrap();
        std::fs::write(root.join("shared/a.md"), "").unwrap();
        std::os::unix::fs::symlink("shared", root.join("docs")).unwrap();
        let docs = list(root, "")
            .unwrap()
            .into_iter()
            .find(|e| e.name == "docs");
        assert!(docs.unwrap().dir);
        let inside: Vec<_> = list(root, "docs")
            .unwrap()
            .into_iter()
            .map(|e| e.path)
            .collect();
        assert_eq!(inside, ["docs/a.md"]);
    }

    #[cfg(unix)]
    #[test]
    fn doesnt_list_through_links_that_leave_the_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(outside.path().join("secret"), "").unwrap();
        std::fs::write(root.join("a.md"), "").unwrap();
        std::os::unix::fs::symlink(outside.path(), root.join("out")).unwrap();
        std::os::unix::fs::symlink(outside.path().join("secret"), root.join("s")).unwrap();
        std::os::unix::fs::symlink("a.md", root.join("b.md")).unwrap();
        let names: Vec<_> = list(root, "")
            .unwrap()
            .into_iter()
            .map(|e| e.path)
            .collect();
        assert_eq!(names, ["a.md", "b.md"]);
        assert!(list(root, "out").unwrap().is_empty());
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

    #[cfg(unix)]
    #[test]
    fn confines_paths_behind_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::os::unix::fs::symlink(outside.path(), root.join("link")).unwrap();
        assert!(confine(root, &root.join("link/x")).is_err());
        assert!(confine(root, &root.join("link/new/x")).is_err());
        confine(root, &root.join("link")).unwrap();
        confine(root, &root.join("new/x")).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn contains_files_that_link_outside() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(outside.path().join("rc"), "").unwrap();
        std::fs::write(root.join("a.md"), "").unwrap();
        std::os::unix::fs::symlink(outside.path().join("rc"), root.join("notes.md")).unwrap();
        std::os::unix::fs::symlink("a.md", root.join("b.md")).unwrap();
        std::os::unix::fs::symlink("gone", root.join("dangling.md")).unwrap();
        assert!(contain(root, &root.join("notes.md")).is_err());
        assert!(contain(root, &root.join("dangling.md")).is_err());
        contain(root, &root.join("b.md")).unwrap();
        contain(root, &root.join("new.md")).unwrap();
    }

    #[test]
    fn creates_and_renames_without_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        create(&root.join("a/b.txt"), "a/b.txt", false).unwrap();
        assert!(create(&root.join("a/b.txt"), "a/b.txt", false).is_err());
        create(&root.join("c"), "c", true).unwrap();
        assert!(rename(&root.join("a/b.txt"), &root.join("c"), "c").is_err());
        rename(&root.join("a/b.txt"), &root.join("d/e.txt"), "d/e.txt").unwrap();
        assert!(root.join("d/e.txt").is_file() && !root.join("a/b.txt").exists());
    }
}
