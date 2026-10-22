//! What `jj git push` would leave out. Without arguments it pushes only bookmarks that
//! already track the remote, so a new bookmark needs `--bookmark`, which starts tracking it.

use serde::Serialize;
use specta::Type;

use super::Jj;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PushPlan {
    pub remote: String,
    /// Local bookmarks in the push range that don't track `remote` yet.
    pub new_bookmarks: Vec<String>,
}

/// The commits plain `jj git push` pushes bookmarks in.
pub fn push_range(remote: &str) -> String {
    format!("remote_bookmarks(remote=exact:{remote:?})..@")
}

impl Jj {
    /// The remote `jj git push` uses: `git.push`, else `origin`.
    async fn push_remote(&self) -> String {
        match self.read(&["config", "get", "git.push"]).await {
            Ok(r) if !r.trim().is_empty() => r.trim().to_string(),
            _ => "origin".to_string(),
        }
    }

    pub async fn push_plan(&self) -> anyhow::Result<PushPlan> {
        let remote = self.push_remote().await;
        let names = self
            .read(&[
                "log",
                "--no-graph",
                "-r",
                &push_range(&remote),
                "-T",
                r#"local_bookmarks.map(|b| b.name() ++ "\n").join("")"#,
            ])
            .await?;
        let tracked = self
            .read(&[
                "bookmark",
                "list",
                "--all-remotes",
                "-T",
                r#"if(remote && tracked, name ++ "\t" ++ remote ++ "\n")"#,
            ])
            .await?;
        let tracks = |name: &str| {
            tracked
                .lines()
                .any(|l| l.split_once('\t') == Some((name, remote.as_str())))
        };
        let mut new_bookmarks: Vec<String> = names
            .lines()
            .filter(|n| !n.is_empty() && !tracks(n))
            .map(str::to_string)
            .collect();
        new_bookmarks.sort();
        new_bookmarks.dedup();
        Ok(PushPlan {
            remote,
            new_bookmarks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jj::JjAction;
    use crate::jj::test_support::{jj, repo, write};

    #[tokio::test]
    async fn a_new_bookmark_is_named_and_pushed_with_the_tracked_ones() {
        let (dir, j) = repo();
        let remote = tempfile::tempdir().unwrap();
        let ok = std::process::Command::new("git")
            .args(["init", "-q", "--bare"])
            .arg(remote.path())
            .status()
            .unwrap();
        assert!(ok.success());
        jj(dir.path(), &["git", "remote", "add", "origin", remote.path().to_str().unwrap()]);
        write(dir.path(), "a", "a\n");
        jj(dir.path(), &["commit", "-m", "a"]);
        jj(dir.path(), &["bookmark", "create", "main", "-r", "@-"]);
        jj(dir.path(), &["git", "push", "-b", "main"]);

        write(dir.path(), "b", "b\n");
        jj(dir.path(), &["commit", "-m", "b"]);
        jj(dir.path(), &["bookmark", "create", "feat", "-r", "@-"]);
        jj(dir.path(), &["bookmark", "set", "main", "-r", "@-"]);
        let plan = j.push_plan().await.unwrap();
        assert_eq!(
            plan,
            PushPlan {
                remote: "origin".into(),
                new_bookmarks: vec!["feat".into()],
            }
        );

        j.act(&JjAction::Push {
            remote: Some(plan.remote),
            new_bookmarks: plan.new_bookmarks,
        })
        .await
        .unwrap();
        let after = j.push_plan().await.unwrap();
        assert!(after.new_bookmarks.is_empty());
        let heads = std::process::Command::new("git")
            .args(["for-each-ref", "--format=%(refname:short)"])
            .current_dir(remote.path())
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&heads.stdout), "feat\nmain\n");
    }
}
