//! What an agent's write about a revset checks: that the review open in its workspace is of
//! that revset, and still shows the commits the revset resolves to.

use super::{RepoCtx, Review};
use crate::jj::Range;
use crate::watch;

/// The review an agent's write is for, and the commits its revset resolved to just before.
pub struct At {
    pub id: u32,
    pub whole: Range,
}

impl RepoCtx {
    /// The review open in `workspace`, for an agent's write about `revset`. The focus may have
    /// moved to another review since the agent was asked, so then the write is refused.
    pub fn current_review_of(&self, workspace: Option<&str>, revset: &str) -> anyhow::Result<u32> {
        let id = self.current_review(workspace)?;
        let shown = self.review(id)?.lock().unwrap().revset.clone();
        anyhow::ensure!(
            shown == revset,
            "lgtm's review moved to `{shown}`, not `{revset}`, so nothing was changed"
        );
        Ok(id)
    }

    /// [`Self::current_review_of`], with the commits `revset` resolves to now, for the write
    /// to pass to [`Self::check_at`] under the review's lock. It snapshots first, as the file
    /// watcher may not have yet.
    pub async fn current_review_at(
        &self,
        workspace: Option<&str>,
        revset: &str,
    ) -> anyhow::Result<At> {
        let id = self.current_review_of(workspace, revset)?;
        let ws = self.review(id)?.lock().unwrap().ws.clone();
        ws.snapshot().await?;
        let whole = ws.jj.resolve(revset).await?;
        Ok(At { id, whole })
    }

    /// Refuses a write unless `review` still shows the commits `at` resolved. When it
    /// doesn't, the window's reviews reload, as after a jj operation.
    pub fn check_at(&self, review: &Review, at: &At) -> anyhow::Result<()> {
        if review.whole == at.whole {
            return Ok(());
        }
        let emit = self.emit.lock().unwrap().clone();
        if let Some(emit) = emit {
            emit(review.ws.name.clone(), watch::Event::RepoChanged { snapshot: false });
        }
        anyhow::bail!(
            "lgtm's review of `{}` is reloading to show the revset's new commits, so nothing \
             was changed; try again once it has",
            review.revset
        )
    }

    /// [`Self::check_at`] on the review `at` is for.
    pub fn ensure_at(&self, at: &At) -> anyhow::Result<()> {
        self.check_at(&self.review(at.id)?.lock().unwrap(), at)
    }
}
