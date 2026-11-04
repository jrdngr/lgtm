//! What an agent can do to the review's state: check files and mark lines with its own
//! colors, which the user tells apart from theirs and which never replace theirs.

use anyhow::{Context, bail};

use super::{RepoCtx, RowRef, StateDto};
use crate::store::MarkColor;
use lgtm_core::review::view::parse_range;

impl RepoCtx {
    /// Checks or unchecks the files at `paths` in the review of `revset`, as an agent.
    pub async fn agent_check_files(
        &self,
        workspace: Option<&str>,
        revset: &str,
        paths: &[String],
        checked: bool,
    ) -> anyhow::Result<(u32, StateDto)> {
        let at = self.current_review_at(workspace, revset).await?;
        let files = {
            let review = self.review(at.id)?;
            let review = review.lock().unwrap();
            self.check_at(&review, &at)?;
            paths
                .iter()
                .map(|p| review.file_index(p))
                .collect::<anyhow::Result<Vec<u32>>>()?
        };
        Ok((at.id, self.agent_set_checked(at.id, &files, checked)?))
    }

    /// Marks the changed rows of `path` on new-side `lines` and old-side `removed` lines
    /// `color`, an agent's, or clears the agent's marks on them when it's `None`.
    pub async fn agent_mark_lines(
        &self,
        workspace: Option<&str>,
        revset: &str,
        path: &str,
        lines: &[String],
        removed: &[String],
        color: Option<MarkColor>,
    ) -> anyhow::Result<(u32, StateDto)> {
        let new: Vec<(u32, u32)> = lines
            .iter()
            .map(|r| parse_range(r))
            .collect::<Result<_, _>>()?;
        let old: Vec<(u32, u32)> = removed
            .iter()
            .map(|r| parse_range(r))
            .collect::<Result<_, _>>()?;
        let at = self.current_review_at(workspace, revset).await?;
        let rows: Vec<RowRef> = {
            let review = self.review(at.id)?;
            let review = review.lock().unwrap();
            self.check_at(&review, &at)?;
            let file = review.file_index(path)?;
            review.view().rows_on(file as usize, &new, &old)
        };
        if rows.is_empty() {
            bail!("`{path}` has no changed rows on those lines");
        }
        Ok((at.id, self.agent_mark(at.id, &rows, color)?))
    }
}

impl super::Review {
    /// The index of the file at `path`, under its new name or its old one.
    fn file_index(&self, path: &str) -> anyhow::Result<u32> {
        self.view()
            .file_index(path)
            .map(|i| i as u32)
            .with_context(|| format!("`{path}` isn't in the review"))
    }
}
