//! Git remotes, fetching, and bookmarks, for checking out PRs.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

use super::Jj;

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
}

#[cfg(test)]
mod tests {
    use crate::jj::test_support::{jj, repo};

    #[tokio::test]
    async fn remotes_follow_changes_made_outside_the_app() {
        let (dir, j) = repo();
        assert!(j.remotes().await.unwrap().is_empty());
        jj(
            dir.path(),
            &["git", "remote", "add", "up", "https://example.com/a.git"],
        );
        let names = |r: Vec<(String, String)>| r.into_iter().map(|(n, _)| n).collect::<Vec<_>>();
        assert_eq!(names(j.remotes().await.unwrap()), ["up"]);
    }
}
