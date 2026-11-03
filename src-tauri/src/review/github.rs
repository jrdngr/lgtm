//! GitHub review threads on a review's rows: placing pulled threads, and where a local comment
//! goes on the PR's diff when it's published. GitHub's lines are the PR head's, and the review's
//! new side can differ from it by edits in the working copy, so lines are remapped between them.

use std::collections::HashMap;

use anyhow::{Context, bail};

use lgtm_core::github::pulled::{self, PulledThread};
use lgtm_core::review::place::hashes;

use super::{RepoCtx, Review, StateDto, now_ms};
use crate::anchor::{Side, remap_lines};
use crate::diff::RowKind;

/// Where a comment goes on the PR head's diff.
#[derive(Debug, PartialEq, Eq)]
pub struct Spot {
    pub path: String,
    pub side: Side,
    pub line: u32,
    pub start: Option<(Side, u32)>,
}

/// Maps 1-based lines of `from` to lines of `to`.
fn line_map(from: &[i64], to: &[i64]) -> HashMap<u32, u32> {
    remap_lines(from, to)
}

impl Review {
    /// Where the comment's first and last changed rows are on the PR head. `head_text` is its
    /// file at the head.
    fn spot(&self, comment: u32, head_text: Option<&str>) -> anyhow::Result<Spot> {
        let c = self
            .comments
            .iter()
            .find(|c| c.id == comment)
            .context("the comment isn't in this review")?;
        let span = c
            .rows
            .context("the code this comment was on is gone, so it can't go on GitHub")?;
        let f = &self.files[c.file as usize];
        let changed: Vec<usize> = (span.start..=span.end)
            .map(|r| r as usize)
            .filter(|&r| f.rows.get(r).is_some_and(|p| p.kind != RowKind::Context))
            .collect();
        let back = match (head_text, f.new.index.as_ref()) {
            (Some(text), Some(ours)) => Some(line_map(&ours.hashes, &hashes(text, &f.path))),
            _ => None,
        };
        let at = |r: usize| -> anyhow::Result<(Side, u32)> {
            let p = &f.rows[r];
            if p.kind == RowKind::Removed {
                return Ok((Side::Old, p.old_line.context("removed row without a line")?));
            }
            let line = p.new_line.context("added row without a line")?;
            match &back {
                Some(map) => map.get(&line).map(|l| (Side::New, *l)).with_context(|| {
                    format!(
                        "line {line} of {} isn't in the PR's head, so it can't go on GitHub",
                        f.path
                    )
                }),
                None => Ok((Side::New, line)),
            }
        };
        let (Some(&first), Some(&last)) = (changed.first(), changed.last()) else {
            bail!("the comment covers no changed rows");
        };
        let (side, line) = at(last)?;
        let start = (first != last).then(|| at(first)).transpose()?;
        Ok(Spot {
            path: f.path.clone(),
            side,
            line,
            start,
        })
    }
}

impl RepoCtx {
    /// Adds pulled threads as comments, and updates the ones already pulled. `head_texts` are
    /// the threads' files at the PR head.
    pub fn add_threads(
        &self,
        id: u32,
        threads: Vec<PulledThread>,
        head_texts: &HashMap<String, String>,
        label: &str,
    ) -> anyhow::Result<StateDto> {
        self.comment_edit(id, |review, store| {
            pulled::add_threads(
                store,
                &review.placer(),
                threads,
                head_texts,
                label,
                now_ms(),
            )
        })
    }

    /// Where a comment goes on the PR head's diff. `head_text` is its file at the head.
    pub fn comment_spot(
        &self,
        id: u32,
        comment: u32,
        head_text: Option<&str>,
    ) -> anyhow::Result<Spot> {
        self.review(id)?.lock().unwrap().spot(comment, head_text)
    }
}
