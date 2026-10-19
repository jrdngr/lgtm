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
use crate::jj::git::remote_bookmark;
use crate::store::PrRecord;

pub use discussion::DiscussionDto;
pub use nearby::NearbyPrsDto;
pub use preview::PrPreviewDto;
pub use refetch::HeadDto;
pub use send::Fixed;

impl PrRecord {
    /// What the workspace reviews: the PR's own commits, which match GitHub's diff.
    pub fn revset(&self) -> String {
        let base = remote_bookmark(&self.base_ref, &self.base_remote);
        format!("{base}..{}", self.head_revset())
    }

    /// The PR's head. A stack PR's is its bookmark: the local one, else where it was fetched.
    /// A checked-out PR's is where it was fetched to, since its branch name, like a fork's
    /// `main`, can name another branch here.
    pub fn head_revset(&self) -> String {
        match (self.stack, &self.head_remote) {
            (true, _) => head_revset(&self.head_ref),
            (false, Some(remote)) => remote_bookmark(&self.head_ref, remote),
            (false, None) => format!("bookmarks(exact:{})", quoted(&fallback_bookmark(self.number))),
        }
    }
}

fn quoted(name: &str) -> String {
    format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\""))
}

fn head_revset(head: &str) -> String {
    let head = quoted(head);
    format!("coalesce(bookmarks(exact:{head}), remote_bookmarks(exact:{head}))")
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
    /// Picked from the workspace's stack rather than checked out into it.
    pub stack: bool,
    /// Picked from the stack, which `@` has since left.
    pub off: bool,
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
            off: p.off,
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
