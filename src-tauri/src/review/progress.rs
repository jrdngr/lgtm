//! Review progress for the workspace sidebar, without opening a review.

use serde::Serialize;
use specta::Type;

use super::{RepoCtx, Review, View};

/// Changed-row counts by state, as the review header counts them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProgressDto {
    /// The revset counted: the workspace's latest review, or `@`.
    pub revset: String,
    pub reviewed: u32,
    pub skipped: u32,
    pub flagged: u32,
    pub unreviewed: u32,
    pub changed: u32,
}

impl RepoCtx {
    /// Progress of the workspace's latest review, or of `@` if it has none.
    pub async fn progress(&self, workspace: Option<&str>) -> anyhow::Result<ProgressDto> {
        let ws = self.workspace(workspace).await?;
        let revset = self
            .store
            .lock()
            .unwrap()
            .last_revset(ws.key())?
            .unwrap_or_else(|| "@".into());
        let loaded = self.load(&ws, &revset, &View::Combined, None).await?;
        let label = ws.label(&revset);
        let mut review = Review::new(ws, &revset, label, loaded.cfg);
        review.files = loaded.files;
        review.rematch(&self.store.lock().unwrap())?;

        let c = review.summary().rows;
        Ok(ProgressDto {
            revset,
            reviewed: c.reviewed,
            skipped: c.skipped,
            flagged: c.flagged,
            unreviewed: c.unreviewed,
            changed: c.reviewed + c.skipped + c.flagged + c.unreviewed,
        })
    }
}
