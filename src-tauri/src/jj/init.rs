//! Turning a plain git repo into a colocated jj repo.

use std::path::{Path, PathBuf};

/// Nearest ancestor of `path` (inclusive) with a `.git` directory or worktree file.
pub fn git_root(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|p| p.join(".git").exists())
        .map(Path::to_path_buf)
}

/// `jj git init --colocate` at `root`. The git repo keeps working as before.
pub fn init_colocated(root: &Path) -> anyhow::Result<()> {
    let out = std::process::Command::new("jj")
        .args(["git", "init", "--colocate", "--no-pager", "--color=never"])
        .current_dir(root)
        .stdin(std::process::Stdio::null())
        .output()?;
    anyhow::ensure!(
        out.status.success(),
        "jj git init: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_colocates_a_git_repo() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let ok = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .status()
            .unwrap()
            .success();
        assert!(ok);
        std::fs::create_dir(root.join("src")).unwrap();

        assert_eq!(git_root(&root.join("src")), Some(root.clone()));
        init_colocated(&root).unwrap();
        assert!(root.join(".jj").is_dir());
        assert!(root.join(".git").is_dir());
    }
}
