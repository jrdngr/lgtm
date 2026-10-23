//! What `jj git push` would leave out. Without arguments it pushes only bookmarks that
//! already track the remote, so a new bookmark needs `--bookmark`, which starts tracking it.

use serde::Serialize;
use specta::Type;

use super::Jj;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PushPlan {
    pub remote: String,
    /// The revset plain push takes bookmarks from: the user's `revsets.git-push`, else jj's
    /// default. Its `remote` is bound with [`remote_binding`].
    pub range: String,
    /// Local bookmarks in the push range that don't track `remote` yet.
    pub new_bookmarks: Vec<String>,
}

/// jj's default for `revsets.git-push`.
const DEFAULT_RANGE: &str = "remote_bookmarks(remote=exact:remote)..@";

/// The `--config` that binds the `remote` a push range names, as jj does when it pushes.
pub fn remote_binding(remote: &str) -> String {
    let quoted = remote.replace('\\', "\\\\").replace('"', "\\\"");
    format!("revset-aliases.remote='\"{quoted}\"'")
}

impl Jj {
    /// The remote `jj git push` uses: `git.push`, else the only remote, else `origin`.
    async fn push_remote(&self) -> String {
        match self.read(&["config", "get", "git.push"]).await {
            Ok(r) if !r.trim().is_empty() => return r.trim().to_string(),
            _ => {}
        }
        match self.remotes().await.as_deref() {
            Ok([(only, _)]) => only.clone(),
            _ => "origin".to_string(),
        }
    }

    pub async fn push_plan(&self) -> anyhow::Result<PushPlan> {
        let remote = self.push_remote().await;
        let range = match self.read(&["config", "get", "revsets.git-push"]).await {
            Ok(r) if !r.trim().is_empty() => r.trim().to_string(),
            _ => DEFAULT_RANGE.to_string(),
        };
        let names = self
            .read(&[
                "log",
                "--no-graph",
                "--config",
                &remote_binding(&remote),
                "-r",
                &range,
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
            range,
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
    async fn a_new_bookmark_is_named_and_pushed_with_the_tracked_ones_to_the_only_remote() {
        let (dir, j) = repo();
        let remote = tempfile::tempdir().unwrap();
        let ok = std::process::Command::new("git")
            .args(["init", "-q", "--bare"])
            .arg(remote.path())
            .status()
            .unwrap();
        assert!(ok.success());
        jj(dir.path(), &["git", "remote", "add", "up", remote.path().to_str().unwrap()]);
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
                remote: "up".into(),
                range: DEFAULT_RANGE.into(),
                new_bookmarks: vec!["feat".into()],
            }
        );

        j.act(&JjAction::Push {
            remote: Some(plan.remote),
            range: Some(plan.range),
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
