//! Telling the app's own working-copy moves from ones made elsewhere: a terminal, an agent.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};

use super::{Jj, checkout};

/// Each workspace's `@` change by name, and the operation heads it was read at.
type Listed = Option<(Vec<String>, HashMap<String, String>)>;

/// The last listing per repo store. Every workspace's watcher looks after each operation,
/// so they share one `jj workspace list` per operation instead of a read each.
static LISTED: LazyLock<Mutex<HashMap<PathBuf, Arc<tokio::sync::Mutex<Listed>>>>> =
    LazyLock::new(Mutex::default);

impl Jj {
    async fn working_copy_change(&self) -> anyhow::Result<String> {
        self.read(&["log", "--no-graph", "-r", "@", "-T", "change_id"])
            .await
    }

    /// The `@` change of this workspace, from a listing of every workspace's that's shared
    /// with the repo's other workspaces while the operation stays the same.
    async fn listed_working_copy(&self) -> anyhow::Result<String> {
        let Some(repo) = checkout::repo_dir(&self.root) else {
            return self.working_copy_change().await;
        };
        let entry = LISTED
            .lock()
            .unwrap()
            .entry(repo.clone())
            .or_default()
            .clone();
        let mut listed = entry.lock().await;
        let heads = checkout::op_heads(&repo);
        let fresh = listed
            .as_ref()
            .filter(|(at, _)| !heads.is_empty() && *at == heads);
        let changes = match fresh {
            Some((_, changes)) => changes.clone(),
            None => {
                let out = self
                    .read(&[
                        "workspace",
                        "list",
                        "-T",
                        r#"name ++ "\t" ++ target.change_id() ++ "\n""#,
                    ])
                    .await?;
                let changes: HashMap<String, String> = out
                    .lines()
                    .filter_map(|l| l.split_once('\t'))
                    .map(|(name, change)| (name.to_string(), change.to_string()))
                    .collect();
                *listed = Some((heads, changes.clone()));
                changes
            }
        };
        drop(listed);
        match changes.get(self.name()) {
            Some(change) => Ok(change.clone()),
            None => self.working_copy_change().await,
        }
    }

    /// Notes where `@` is after an app write, under the writer lock, so it isn't reported as
    /// a move made elsewhere.
    pub(super) async fn note_working_copy(&self, seen: &mut Option<String>) {
        match self.working_copy_change().await {
            Ok(change) => *seen = Some(change),
            Err(e) => tracing::warn!(workspace = self.name(), "read @: {e:#}"),
        }
    }

    /// Whether `@` moved to another change since the app last looked, by something other than
    /// the app. The first look only notes where it is.
    pub async fn moved_working_copy(&self) -> anyhow::Result<bool> {
        // Waits out an app write, which notes its own move when it's done.
        let mut seen = self.writer.lock().await;
        let now = self.listed_working_copy().await?;
        let moved = seen.as_ref().is_some_and(|s| *s != now);
        *seen = Some(now);
        Ok(moved)
    }
}

#[cfg(test)]
mod tests {
    use crate::jj::test_support::{jj, repo, write};
    use crate::jj::{Jj, JjAction};

    #[tokio::test]
    async fn reports_moves_made_outside_the_app_only() {
        let (dir, j) = repo();
        let d = dir.path();
        let j = &j;
        write(d, "a.txt", "a\n");
        jj(d, &["commit", "-m", "base"]);
        let base = j.changes("@-").await.unwrap()[0].change_id.clone();
        let moved = async || j.moved_working_copy().await.unwrap();

        assert!(!moved().await);
        // An edit to the working copy isn't a move.
        write(d, "a.txt", "b\n");
        j.snapshot().await.unwrap();
        assert!(!moved().await);

        jj(d, &["new", &base]);
        assert!(moved().await);
        assert!(!moved().await);

        // The app's own moves aren't reported.
        j.act(&JjAction::Edit { rev: base.clone() }).await.unwrap();
        jj(d, &["new", &base]);
        assert!(moved().await);
        j.edit(&base).await.unwrap();
        assert!(!moved().await);
    }

    #[tokio::test]
    async fn workspaces_of_one_repo_each_see_only_their_own_moves() {
        let (dir, main) = repo();
        let d = dir.path();
        let other_dir = tempfile::tempdir().unwrap();
        let other_root = other_dir.path().join("other");
        jj(
            d,
            &[
                "workspace",
                "add",
                "--name",
                "other",
                other_root.to_str().unwrap(),
            ],
        );
        let other = Jj::new(&other_root);
        assert!(!main.moved_working_copy().await.unwrap());
        assert!(!other.moved_working_copy().await.unwrap());

        jj(&other_root, &["new"]);
        assert!(!main.moved_working_copy().await.unwrap());
        assert!(other.moved_working_copy().await.unwrap());
        jj(d, &["new"]);
        assert!(main.moved_working_copy().await.unwrap());
        assert!(!other.moved_working_copy().await.unwrap());
    }
}
