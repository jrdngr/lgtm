//! Moving the working copy and rewriting commits, for refetching PRs.

use super::Jj;

impl Jj {
    /// Runs a write that can move `@`, and notes where `@` ends up as the app's own move.
    async fn write(&self, args: &[&str]) -> anyhow::Result<()> {
        let mut seen = self.writer.lock().await;
        let done = self.exec(args, false).await.map(drop);
        self.note_working_copy(&mut seen).await;
        done
    }

    /// Puts `@` on a new commit on top of `rev`.
    pub async fn new_on(&self, rev: &str) -> anyhow::Result<()> {
        self.write(&["new", rev]).await
    }

    pub async fn edit(&self, rev: &str) -> anyhow::Result<()> {
        self.write(&["edit", rev]).await
    }

    /// Rebases the commits in `source` and their descendants onto `dest`.
    pub async fn rebase(&self, source: &str, dest: &str) -> anyhow::Result<()> {
        self.write(&["rebase", "-s", source, "-o", dest]).await
    }
}
