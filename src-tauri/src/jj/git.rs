//! Git remotes, fetching, and bookmarks, for checking out PRs.

use std::path::{Path, PathBuf};

use anyhow::Context;
use tokio::process::Command;

use super::Jj;

/// An exact bookmark name as a string pattern.
fn exact(name: &str) -> String {
    format!("exact:{name}")
}

/// A remote bookmark as a revset, quoted so names with `/` or `-` parse.
pub fn remote_bookmark(name: &str, remote: &str) -> String {
    let q = |s: &str| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""));
    format!("{}@{}", q(name), q(remote))
}

impl Jj {
    /// Remote names and URLs.
    pub async fn remotes(&self) -> anyhow::Result<Vec<(String, String)>> {
        let out = self.read(&["git", "remote", "list"]).await?;
        Ok(out
            .lines()
            .filter_map(|l| l.split_once(' '))
            .map(|(n, u)| (n.to_string(), u.trim().to_string()))
            .collect())
    }

    pub async fn remote_add(&self, name: &str, url: &str) -> anyhow::Result<()> {
        let _guard = self.writer.lock().await;
        self.exec(&["git", "remote", "add", name, url], false)
            .await
            .map(drop)
    }

    pub async fn remote_remove(&self, name: &str) -> anyhow::Result<()> {
        let _guard = self.writer.lock().await;
        self.exec(&["git", "remote", "remove", name], false)
            .await
            .map(drop)
    }

    /// Fetches `branches` from `remote`.
    pub async fn fetch(&self, remote: &str, branches: &[&str]) -> anyhow::Result<()> {
        let mut args = vec![
            "git".to_string(),
            "fetch".into(),
            "--remote".into(),
            remote.into(),
        ];
        for b in branches {
            args.extend(["-b".into(), exact(b)]);
        }
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let _guard = self.writer.lock().await;
        self.exec(&args, false).await.map(drop)
    }

    /// The git repository jj stores commits in, colocated or not.
    pub fn git_dir(&self) -> anyhow::Result<PathBuf> {
        let store = self.root.join(".jj/repo/store");
        let target = std::fs::read_to_string(store.join("git_target"))
            .context("this repo isn't backed by git")?;
        Ok(store.join(target.trim()))
    }

    /// Fetches `refspec` from `remote` with git itself, for refs jj can't fetch (like
    /// `pull/N/head`), then imports them.
    pub async fn git_fetch_ref(&self, remote: &str, refspec: &str) -> anyhow::Result<()> {
        let git_dir = self.git_dir()?;
        let _guard = self.writer.lock().await;
        let out = Command::new("git")
            .arg("--git-dir")
            .arg(&git_dir)
            .args(["fetch", "--no-tags", remote, refspec])
            .stdin(std::process::Stdio::null())
            .output()
            .await
            .context("run git")?;
        anyhow::ensure!(
            out.status.success(),
            "git fetch {remote} {refspec}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
        self.exec(&["git", "import"], false).await.map(drop)
    }

    /// Local bookmarks matching the glob.
    pub async fn bookmarks(&self, glob: &str) -> anyhow::Result<Vec<String>> {
        let pattern = format!("glob:{glob}");
        let out = self
            .read(&["bookmark", "list", "-T", r#"name ++ "\n""#, &pattern])
            .await?;
        let mut names: Vec<String> = out.lines().map(str::to_string).collect();
        names.dedup();
        Ok(names)
    }

    pub async fn bookmark_delete(&self, name: &str) -> anyhow::Result<()> {
        let _guard = self.writer.lock().await;
        self.exec(&["bookmark", "delete", &exact(name)], false)
            .await
            .map(drop)
    }

    /// Whether `commit` is in the repo.
    pub async fn has_commit(&self, commit: &str) -> bool {
        self.commit_ids(&format!("present({commit})"))
            .await
            .is_ok_and(|c| !c.is_empty())
    }
}

/// Prunes stale git worktree entries in the repo at `root`, if it has a `.git`. Best effort.
pub fn worktree_prune_blocking(root: &Path) {
    if !root.join(".git").exists() {
        return;
    }
    std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["worktree", "prune"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok();
}

#[cfg(test)]
mod tests {
    #[test]
    fn remote_bookmarks_are_quoted() {
        assert_eq!(super::remote_bookmark("a/b", "gh-x"), r#""a/b"@"gh-x""#);
    }
}
