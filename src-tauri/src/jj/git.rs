//! Git remotes, fetching, and bookmarks, for checking out PRs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

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

type Remotes = Vec<(String, String)>;

/// Each repo's remotes by its git config file, as of that file's last change.
static REMOTES: LazyLock<Mutex<HashMap<PathBuf, (SystemTime, Remotes)>>> =
    LazyLock::new(Mutex::default);

impl Jj {
    /// Remote names and URLs. Read again only when the git config holding them changes.
    pub async fn remotes(&self) -> anyhow::Result<Remotes> {
        let config = self.git_config();
        let changed = config
            .as_deref()
            .and_then(|c| std::fs::metadata(c).and_then(|m| m.modified()).ok());
        if let (Some(config), Some(changed)) = (&config, changed)
            && let Some((at, remotes)) = REMOTES.lock().unwrap().get(config)
            && *at == changed
        {
            return Ok(remotes.clone());
        }
        let out = self.read(&["git", "remote", "list"]).await?;
        let remotes: Remotes = out
            .lines()
            .filter_map(|l| l.split_once(' '))
            .map(|(n, u)| (n.to_string(), u.trim().to_string()))
            .collect();
        if let (Some(config), Some(changed)) = (config, changed) {
            REMOTES
                .lock()
                .unwrap()
                .insert(config, (changed, remotes.clone()));
        }
        Ok(remotes)
    }

    /// The config file of the git repo backing this one, where remotes live.
    fn git_config(&self) -> Option<PathBuf> {
        let store = super::checkout::repo_dir(&self.root)?.join("store");
        let target = std::fs::read_to_string(store.join("git_target")).ok()?;
        Some(store.join(target.trim()).join("config"))
    }

    pub async fn remote_add(&self, name: &str, url: &str) -> anyhow::Result<()> {
        let _guard = self.writer.lock().await;
        let added = self.exec(&["git", "remote", "add", name, url], false).await;
        REMOTES.lock().unwrap().clear();
        added.map(drop)
    }

    pub async fn remote_remove(&self, name: &str) -> anyhow::Result<()> {
        let _guard = self.writer.lock().await;
        let removed = self.exec(&["git", "remote", "remove", name], false).await;
        REMOTES.lock().unwrap().clear();
        removed.map(drop)
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
    use crate::jj::test_support::{jj, repo};

    #[tokio::test]
    async fn remotes_follow_changes_made_outside_the_app() {
        let (dir, j) = repo();
        assert!(j.remotes().await.unwrap().is_empty());
        jj(dir.path(), &["git", "remote", "add", "up", "https://example.com/a.git"]);
        let names = |r: Vec<(String, String)>| r.into_iter().map(|(n, _)| n).collect::<Vec<_>>();
        assert_eq!(names(j.remotes().await.unwrap()), ["up"]);
        j.remote_add("origin", "https://example.com/b.git").await.unwrap();
        assert_eq!(names(j.remotes().await.unwrap()), ["origin", "up"]);
    }

    #[test]
    fn remote_bookmarks_are_quoted() {
        assert_eq!(super::remote_bookmark("a/b", "gh-x"), r#""a/b"@"gh-x""#);
    }
}
