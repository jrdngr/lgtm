//! Moving the working copy and rewriting commits, for refetching PRs and resolving conflicts.

use super::Jj;

impl Jj {
    async fn write(&self, args: &[&str]) -> anyhow::Result<()> {
        let _guard = self.writer.lock().await;
        self.exec(args, false).await.map(drop)
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

    /// Moves `@`'s changes into its parent, keeping the parent's description.
    pub async fn squash_into_parent(&self) -> anyhow::Result<()> {
        self.write(&["squash", "--use-destination-message"]).await
    }

    pub async fn abandon(&self, rev: &str) -> anyhow::Result<()> {
        self.write(&["abandon", rev]).await
    }
}
