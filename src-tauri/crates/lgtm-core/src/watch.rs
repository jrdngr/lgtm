//! Telling what a change under a jj workspace means: a new operation, an edit to the working
//! copy, or nothing (jj's and git's internals, and ignored files).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ignore::gitignore::Gitignore;

/// What a change under a workspace means.
#[derive(Debug, PartialEq, Eq)]
pub enum Ping {
    /// A jj operation landed.
    Op,
    /// A file of the working copy changed.
    Edit,
}

/// Whether a path is ignored by the `.gitignore` files above it. Matchers load lazily.
pub struct Ignores {
    root: PathBuf,
    by_dir: HashMap<PathBuf, Option<Gitignore>>,
}

impl Ignores {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            by_dir: HashMap::new(),
        }
    }

    fn ignored(&mut self, path: &Path) -> bool {
        let is_dir = path.is_dir();
        let Ok(rel) = path.strip_prefix(&self.root) else {
            return true;
        };
        let mut dir = self.root.clone();
        for part in rel.parent().into_iter().flat_map(Path::components) {
            if self.check(&dir, path, is_dir) {
                return true;
            }
            dir.push(part);
        }
        self.check(&dir, path, is_dir)
    }

    fn check(&mut self, dir: &Path, path: &Path, is_dir: bool) -> bool {
        let gi = self.by_dir.entry(dir.to_path_buf()).or_insert_with(|| {
            let file = dir.join(".gitignore");
            file.is_file().then(|| Gitignore::new(file).0)
        });
        gi.as_ref()
            .is_some_and(|gi| gi.matched_path_or_any_parents(path, is_dir).is_ignore())
    }
}

pub fn classify(root: &Path, ignores: &mut Ignores, path: &Path) -> Option<Ping> {
    let rel = path.strip_prefix(root).ok()?;
    if rel.starts_with(".jj/repo/op_heads/heads") {
        return Some(Ping::Op);
    }
    // Its own and any nested workspace's or repo's internals.
    if rel
        .components()
        .any(|c| c.as_os_str() == ".jj" || c.as_os_str() == ".git")
    {
        return None;
    }
    if rel.file_name().is_some_and(|n| n == ".gitignore") {
        ignores.by_dir.clear();
        return Some(Ping::Edit);
    }
    (!ignores.ignored(path)).then_some(Ping::Edit)
}

/// A secondary workspace's `.jj/repo` is a file pointing at the main repo, whose operations
/// land outside the workspace. Returns that repo's op heads directory, to watch as well.
pub fn shared_op_heads(root: &Path) -> Option<PathBuf> {
    let link = std::fs::read_to_string(root.join(".jj/repo")).ok()?;
    let repo = root.join(".jj").join(link.trim()).canonicalize().ok()?;
    Some(repo.join("op_heads/heads"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_ops_edits_and_ignored_paths() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join(".gitignore"), "/target\n*.log\n").unwrap();
        std::fs::create_dir_all(root.join("web")).unwrap();
        std::fs::write(root.join("web/.gitignore"), "dist/\n").unwrap();
        std::fs::create_dir_all(root.join("web/dist")).unwrap();
        let mut ignores = Ignores::new(&root);
        let mut c = |p: &str| classify(&root, &mut ignores, &root.join(p));
        assert_eq!(c(".jj/repo/op_heads/heads/abc"), Some(Ping::Op));
        assert_eq!(c(".jj/repo/store/x"), None);
        assert_eq!(c(".git/index"), None);
        assert_eq!(c(".workspaces/ws/.jj/working_copy/checkout"), None);
        assert_eq!(c("vendor/lib/.git/HEAD"), None);
        assert_eq!(c("src/main.rs"), Some(Ping::Edit));
        assert_eq!(c("target/debug/x"), None);
        assert_eq!(c("a/b.log"), None);
        assert_eq!(c("web/dist/app.js"), None);
        assert_eq!(c("web/src/app.ts"), Some(Ping::Edit));
        assert_eq!(c(".gitignore"), Some(Ping::Edit));
    }

    #[test]
    fn a_secondary_workspace_also_watches_the_main_repos_op_heads() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let (main, ws) = (root.join("main"), root.join("ws"));
        std::fs::create_dir_all(main.join(".jj/repo/op_heads/heads")).unwrap();
        std::fs::create_dir_all(ws.join(".jj")).unwrap();
        std::fs::write(ws.join(".jj/repo"), "../../main/.jj/repo").unwrap();
        assert_eq!(
            shared_op_heads(&ws),
            Some(main.join(".jj/repo/op_heads/heads"))
        );
        assert_eq!(shared_op_heads(&main), None);
    }
}
