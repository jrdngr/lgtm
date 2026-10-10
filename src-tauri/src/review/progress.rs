//! Review progress for the workspace sidebar, without opening a review.

use serde::Serialize;
use specta::Type;

use super::{RepoCtx, Review, RowRef, View};
use crate::store::MarkState;

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

        let mut p = ProgressDto {
            revset,
            ..ProgressDto::default()
        };
        for (i, file) in review.files.iter().enumerate() {
            let checked = review.checks[i].as_ref().is_some_and(|c| c.current);
            let skipped = loaded.dto.files[i].skipped;
            for k in &file.keyed {
                let mark = review.matched.get(&RowRef::new(i, k.row));
                let count = match (checked, mark.map(|m| m.state)) {
                    (true, _) | (_, Some(MarkState::Reviewed)) => &mut p.reviewed,
                    (_, Some(MarkState::Skipped)) => &mut p.skipped,
                    (_, Some(MarkState::Flagged)) => &mut p.flagged,
                    (_, None) if skipped => &mut p.skipped,
                    (_, None) => &mut p.unreviewed,
                };
                *count += 1;
                p.changed += 1;
            }
        }
        Ok(p)
    }
}
