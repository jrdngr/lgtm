//! What an agent can do to the review's state: check files and mark lines with its own
//! colors, which the user tells apart from theirs and which never replace theirs.

use anyhow::{Context, bail};

use super::{RepoCtx, RowRef, StateDto};
use crate::anchor::Side;
use crate::store::MarkColor;

/// A line range as get_review writes them: `12` or `12-20`.
fn parse_range(range: &str) -> anyhow::Result<(u32, u32)> {
    let bad = || format!("`{range}` isn't a line or a range like `12-20`");
    let (a, b) = range.split_once('-').unwrap_or((range, range));
    let a: u32 = a.trim().parse().with_context(bad)?;
    let b: u32 = b.trim().parse().with_context(bad)?;
    anyhow::ensure!(a <= b, bad());
    Ok((a, b))
}

fn within(ranges: &[(u32, u32)], line: u32) -> bool {
    ranges.iter().any(|&(a, b)| (a..=b).contains(&line))
}

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
            review.files[file as usize]
                .keyed
                .iter()
                .filter(|k| match k.side {
                    Side::New => within(&new, k.line),
                    Side::Old => within(&old, k.line),
                })
                .map(|k| RowRef::new(file as usize, k.row))
                .collect()
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
        self.files
            .iter()
            .position(|f| f.path == path || f.old_path.as_deref() == Some(path))
            .map(|i| i as u32)
            .with_context(|| format!("`{path}` isn't in the review"))
    }
}
