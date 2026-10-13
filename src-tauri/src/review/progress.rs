//! Review progress for the workspace sidebar, without opening a review.

use serde::Serialize;
use specta::Type;

use super::agent::RowState;
use super::{RepoCtx, Review, View};
use crate::settings;
use crate::store::MarkColor;

/// Changed-row counts, as the review header counts them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProgressDto {
    /// The revset counted: the workspace's latest review, or `@`.
    pub revset: String,
    /// Rows that count as done: marked in a done color, or in a checked or skipped file.
    pub done: u32,
    pub changed: u32,
    /// Marked rows by color, in key order.
    pub marks: Vec<MarkCount>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub struct MarkCount {
    pub color: MarkColor,
    pub rows: u32,
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

        let slots = settings::get().marks;
        let mut p = ProgressDto {
            revset,
            ..ProgressDto::default()
        };
        let mut by_color = std::collections::BTreeMap::<MarkColor, u32>::new();
        for (fi, f) in review.files.iter().enumerate() {
            for row in 0..f.rows.len() {
                let Some(state) = review.row_state(fi, row) else {
                    continue;
                };
                p.changed += 1;
                p.done += u32::from(state.done(&slots));
                if let RowState::Mark(color) = state {
                    *by_color.entry(color).or_default() += 1;
                }
            }
        }
        p.marks = slots
            .iter()
            .filter_map(|s| {
                let rows = by_color.get(&s.color).copied().unwrap_or(0);
                (rows > 0).then_some(MarkCount {
                    color: s.color,
                    rows,
                })
            })
            .collect();
        Ok(p)
    }
}
