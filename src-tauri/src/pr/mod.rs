//! PR workspaces: checking a GitHub PR out into its own workspace, refetching it, and its
//! discussion.

mod checkout;
mod cleanup;
mod discussion;
mod nearby;
mod publish;
mod pull;
mod refetch;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::github::GhRepo;
use crate::jj::git::remote_bookmark;
use crate::review::RepoCtx;

pub use discussion::DiscussionDto;
pub use nearby::NearbyPrDto;
pub use refetch::HeadDto;

/// A PR workspace's PR, as stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrRecord {
    pub workspace: String,
    /// The repo the PR is in.
    pub repo: GhRepo,
    pub number: u32,
    /// The PR's node id, which mutations take.
    pub id: String,
    pub title: String,
    pub url: String,
    pub author: String,
    pub base_ref: String,
    /// The remote for `repo`, which the base branch is fetched from.
    pub base_remote: String,
    pub head_ref: String,
    /// The remote the head branch was fetched from: the base remote, `gh-<owner>` for a
    /// fork, or `None` when it came from `pull/N/head`.
    pub head_remote: Option<String>,
    /// A fork remote lgtm added for this PR, which goes when the workspace does.
    #[serde(default)]
    pub added_remote: Option<String>,
    /// The head commit as last fetched.
    pub head: String,
}

impl PrRecord {
    /// What the workspace reviews: the PR's commits and the working copy above them, which
    /// matches GitHub's diff.
    pub fn revset(&self) -> String {
        format!("{}..@", remote_bookmark(&self.base_ref, &self.base_remote))
    }
}

/// The bookmark a PR's head is fetched into when its branch can't be fetched.
pub fn fallback_bookmark(number: u32) -> String {
    format!("lgtm/pr-{number}")
}

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
        }
    }
}

impl RepoCtx {
    /// The PR `workspace` was checked out for, if any.
    pub fn pr_record(&self, workspace: Option<&str>) -> anyhow::Result<Option<PrRecord>> {
        let Some(name) = workspace else {
            return Ok(None);
        };
        Ok(self.store.lock().unwrap().pr(name)?)
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
            .ok_or_else(|| anyhow::anyhow!("this workspace isn't a PR's"))
    }
}

#[cfg(test)]
mod tests;
