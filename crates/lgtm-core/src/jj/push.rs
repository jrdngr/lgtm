//! What `jj git push` of some commits' bookmarks would leave out. It pushes only bookmarks
//! that already track the remote, so a new bookmark needs `--bookmark`, which starts tracking
//! it.

use serde::Serialize;

use super::Jj;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PushPlan {
    pub remote: String,
    /// The revset the push takes bookmarks from: the one asked for, else plain push's, the
    /// user's `revsets.git-push` or jj's default. Its `remote` is bound with
    /// [`remote_binding`].
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

/// Why `jj git push` pushed nothing or left bookmarks out, like one on a commit with no
/// description, or no bookmarks in the range. jj only warns and exits 0.
pub fn refusals(args: &[&str], stderr: &str) -> Option<String> {
    if args.get(..2) != Some(&["git", "push"]) {
        return None;
    }
    let warnings: Vec<&str> = stderr
        .lines()
        .filter_map(|l| l.strip_prefix("Warning: "))
        .map(str::trim)
        .collect();
    let refused = warnings.iter().any(|w| w.starts_with("Won't push "));
    let nothing = stderr.lines().any(|l| l.trim() == "Nothing changed.");
    if !refused && !nothing {
        return None;
    }
    let mut why = warnings.join("\n");
    if nothing {
        if !why.is_empty() {
            why.push('\n');
        }
        why.push_str("Nothing changed.");
    }
    Some(why)
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

    /// What a push of `revs`' bookmarks, or a plain push's, would do.
    pub async fn push_plan(&self, revs: Option<&str>) -> anyhow::Result<PushPlan> {
        let remote = self.push_remote().await;
        let range = match (
            revs,
            self.read(&["config", "get", "revsets.git-push"]).await,
        ) {
            (Some(revs), _) => revs.to_string(),
            (None, Ok(r)) if !r.trim().is_empty() => r.trim().to_string(),
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
        jj(
            dir.path(),
            &[
                "git",
                "remote",
                "add",
                "up",
                remote.path().to_str().unwrap(),
            ],
        );
        write(dir.path(), "a", "a\n");
        jj(dir.path(), &["commit", "-m", "a"]);
        jj(dir.path(), &["bookmark", "create", "main", "-r", "@-"]);
        jj(dir.path(), &["git", "push", "-b", "main"]);

        write(dir.path(), "b", "b\n");
        jj(dir.path(), &["commit", "-m", "b"]);
        jj(dir.path(), &["bookmark", "create", "feat", "-r", "@-"]);
        jj(dir.path(), &["bookmark", "set", "main", "-r", "@-"]);
        let plan = j.push_plan(None).await.unwrap();
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
        let after = j.push_plan(None).await.unwrap();
        assert!(after.new_bookmarks.is_empty());
        let heads = std::process::Command::new("git")
            .args(["for-each-ref", "--format=%(refname:short)"])
            .current_dir(remote.path())
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&heads.stdout), "feat\nmain\n");
    }

    #[tokio::test]
    async fn a_push_jj_refuses_fails_with_why() {
        let (dir, j) = repo();
        let remote = tempfile::tempdir().unwrap();
        let ok = std::process::Command::new("git")
            .args(["init", "-q", "--bare"])
            .arg(remote.path())
            .status()
            .unwrap();
        assert!(ok.success());
        jj(
            dir.path(),
            &[
                "git",
                "remote",
                "add",
                "origin",
                remote.path().to_str().unwrap(),
            ],
        );
        write(dir.path(), "a", "a\n");
        jj(dir.path(), &["commit", "-m", "a"]);
        jj(dir.path(), &["bookmark", "create", "main", "-r", "@-"]);
        jj(dir.path(), &["git", "push", "-b", "main"]);
        write(dir.path(), "b", "b\n");
        jj(dir.path(), &["new"]);
        jj(dir.path(), &["bookmark", "set", "main", "-r", "@-"]);

        let push = JjAction::Push {
            remote: None,
            range: None,
            new_bookmarks: vec![],
        };
        let err = j.act(&push).await.unwrap_err().to_string();
        let first = err.lines().next().unwrap_or("");
        assert!(
            first.starts_with("Won't push bookmark main: commit "),
            "{err}"
        );
        assert!(first.ends_with("has no description"), "{err}");
    }

    /// A repo whose first commit is pushed to `origin` on `main`.
    fn pushed_repo() -> (tempfile::TempDir, tempfile::TempDir, crate::jj::Jj) {
        let (dir, j) = repo();
        let remote = tempfile::tempdir().unwrap();
        let ok = std::process::Command::new("git")
            .args(["init", "-q", "--bare"])
            .arg(remote.path())
            .status()
            .unwrap();
        assert!(ok.success());
        let url = remote.path().to_str().unwrap();
        jj(dir.path(), &["git", "remote", "add", "origin", url]);
        write(dir.path(), "a", "a\n");
        jj(dir.path(), &["commit", "-m", "a"]);
        jj(dir.path(), &["bookmark", "create", "main", "-r", "@-"]);
        jj(dir.path(), &["git", "push", "-b", "main"]);
        (dir, remote, j)
    }

    fn remote_ref(remote: &std::path::Path, name: &str) -> String {
        let out = std::process::Command::new("git")
            .args(["rev-parse", name])
            .current_dir(remote)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    #[tokio::test]
    async fn a_stacked_bookmark_below_a_pushed_one_pushes_by_its_commit() {
        let (dir, remote, j) = pushed_repo();
        write(dir.path(), "b", "b\n");
        jj(dir.path(), &["commit", "-m", "b"]);
        jj(dir.path(), &["bookmark", "create", "low", "-r", "@-"]);
        write(dir.path(), "c", "c\n");
        jj(dir.path(), &["commit", "-m", "c"]);
        jj(dir.path(), &["bookmark", "create", "high", "-r", "@-"]);
        jj(dir.path(), &["git", "push", "-b", "low", "-b", "high"]);
        // `low` changes and only `high` on top of it goes, so `low` is under a pushed
        // bookmark, where plain push doesn't look.
        jj(dir.path(), &["edit", "low"]);
        write(dir.path(), "b", "b2\n");
        jj(dir.path(), &["git", "push", "-b", "high"]);

        let plain = JjAction::Push {
            remote: None,
            range: None,
            new_bookmarks: vec![],
        };
        let err = j.act(&plain).await.unwrap_err().to_string();
        assert!(err.ends_with("Nothing changed."), "{err}");

        let plan = j.push_plan(Some("low")).await.unwrap();
        assert!(plan.new_bookmarks.is_empty());
        j.act(&JjAction::Push {
            remote: Some(plan.remote),
            range: Some(plan.range),
            new_bookmarks: plan.new_bookmarks,
        })
        .await
        .unwrap();
        let local = std::process::Command::new("jj")
            .arg("-R")
            .arg(dir.path())
            .args(["log", "--no-graph", "-r", "low", "-T", "commit_id"])
            .output()
            .unwrap();
        let local = String::from_utf8_lossy(&local.stdout);
        assert_eq!(remote_ref(remote.path(), "low"), local.trim());
    }
}
