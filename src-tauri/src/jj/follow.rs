//! Telling the app's own working-copy moves from ones made elsewhere: a terminal, an agent.

use super::Jj;

impl Jj {
    async fn working_copy_change(&self) -> anyhow::Result<String> {
        self.read(&["log", "--no-graph", "-r", "@", "-T", "change_id"])
            .await
    }

    /// Notes where `@` is after an app write, under the writer lock, so it isn't reported as
    /// a move made elsewhere.
    pub(super) async fn note_working_copy(&self, seen: &mut Option<String>) {
        match self.working_copy_change().await {
            Ok(change) => *seen = Some(change),
            Err(e) => tracing::warn!(workspace = %self.root.display(), "read @: {e:#}"),
        }
    }

    /// Whether `@` moved to another change since the app last looked, by something other than
    /// the app. The first look only notes where it is.
    pub async fn moved_working_copy(&self) -> anyhow::Result<bool> {
        // Waits out an app write, which notes its own move when it's done.
        let mut seen = self.writer.lock().await;
        let now = self.working_copy_change().await?;
        let moved = seen.as_ref().is_some_and(|s| *s != now);
        *seen = Some(now);
        Ok(moved)
    }
}

#[cfg(test)]
mod tests {
    use crate::jj::JjAction;
    use crate::jj::test_support::{jj, repo, write};

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
}
