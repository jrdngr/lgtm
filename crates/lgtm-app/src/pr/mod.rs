//! PR workspaces: checking a GitHub PR out into its own workspace, or picking one from a
//! workspace's stack, refetching it, and its discussion.

mod checkout;
mod cleanup;
mod discussion;
mod nearby;
mod preview;
mod pull;
mod refetch;
mod send;
mod stack;

use serde::Serialize;
use specta::Type;

use crate::ctx::RepoCtx;
use crate::store::PrRecord;

pub use discussion::DiscussionDto;
pub use nearby::NearbyPrsDto;
pub use preview::PrPreviewDto;
pub use refetch::HeadDto;
pub use send::Fixed;

pub use crate::store::fallback_bookmark;
#[cfg(test)]
use crate::store::head_bookmark_revset as head_revset;

pub fn workspace_name(number: u32) -> String {
    format!("pr-{number}")
}

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PrDto {
    pub number: u32,
    /// `owner/name`.
    pub repo: String,
    pub title: String,
    pub url: String,
    pub author: String,
    pub base_ref: String,
    pub head_ref: String,
    /// The head commit as last fetched.
    pub head: String,
    /// The revset the workspace reviews.
    pub revset: String,
    /// Picked from the workspace's stack rather than checked out into it.
    pub stack: bool,
    /// A stack PR the user pinned.
    pub pinned: bool,
}

impl From<&PrRecord> for PrDto {
    fn from(p: &PrRecord) -> Self {
        Self {
            number: p.number,
            repo: p.repo.to_string(),
            title: p.title.clone(),
            url: p.url.clone(),
            author: p.author.clone(),
            base_ref: p.base_ref.clone(),
            head_ref: p.head_ref.clone(),
            head: p.head.clone(),
            revset: p.revset(),
            stack: p.stack,
            pinned: p.pinned,
        }
    }
}

impl RepoCtx {
    /// The PR `workspace` (`None` for the main one) was checked out for or picked, if any.
    pub fn pr_record(&self, workspace: Option<&str>) -> anyhow::Result<Option<PrRecord>> {
        Ok(self.store.lock().unwrap().pr(workspace.unwrap_or(""))?)
    }

    /// The PR of the review's workspace, when the review shows the whole PR in the Combined
    /// view: only there do GitHub's lines match the review's rows.
    fn whole_pr(&self, id: u32) -> anyhow::Result<PrRecord> {
        let ws = self.review_workspace(id)?;
        let pr = self.need_pr(ws.name.as_deref())?;
        anyhow::ensure!(
            self.shows_whole(id, &pr.revset())?,
            "open the whole PR ({}) in the Combined view first, where GitHub's lines match",
            pr.revset()
        );
        Ok(pr)
    }

    fn need_pr(&self, workspace: Option<&str>) -> anyhow::Result<PrRecord> {
        self.pr_record(workspace)?
            .ok_or_else(|| anyhow::anyhow!("this workspace has no PR"))
    }
}

#[cfg(test)]
mod tests;
