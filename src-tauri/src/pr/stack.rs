//! A PR picked from a workspace's stack instead of checked out into a workspace of its own.
//! It stays the workspace's PR while the review takes in any of its commits, or while it's
//! pinned, and only while the revisions log shows its pill.

use anyhow::bail;

use super::nearby::NearbyPrDto;
use super::{PrDto, PrRecord};
use crate::ctx::RepoCtx;
use crate::github::{self, remote};

impl RepoCtx {
    /// Keeps the workspace's stack PR's head where it was last pushed. Returns the
    /// workspace's PR.
    pub async fn sync_stack_pr(&self, workspace: Option<&str>) -> anyhow::Result<Option<PrDto>> {
        let key = workspace.unwrap_or("");
        let Some(mut pr) = self.store.lock().unwrap().pr(key)? else {
            return Ok(None);
        };
        if pr.stack {
            let stack = self.nearby_prs(workspace, false).await?.prs;
            if let Some(n) = stack.iter().find(|n| n.bookmark == pr.head_ref)
                && n.pushed != pr.head
            {
                pr.head = n.pushed.clone();
                self.store.lock().unwrap().save_pr(&pr)?;
            }
        }
        Ok(Some(PrDto::from(&pr)))
    }

    /// Makes stack PR `number` the workspace's PR, its commits stopping at the open PR below
    /// it on the stack.
    pub async fn review_stack_pr(
        &self,
        workspace: Option<&str>,
        number: u32,
    ) -> anyhow::Result<PrDto> {
        let key = workspace.unwrap_or("");
        if self
            .store
            .lock()
            .unwrap()
            .pr(key)?
            .is_some_and(|p| !p.stack)
        {
            bail!("this workspace was checked out for its own PR");
        }
        let stack = self.nearby_prs(workspace, false).await?.prs;
        let Some(pr) = stack.iter().find(|p| p.number == number) else {
            bail!("PR #{number} isn't on this workspace's stack");
        };
        let below = self.pr_below(workspace, pr, &stack).await?;
        self.save_stack_pr(key, pr, below).await
    }

    /// The bookmark of the nearest other open PR whose head is below `pr`'s.
    async fn pr_below(
        &self,
        workspace: Option<&str>,
        pr: &NearbyPrDto,
        stack: &[NearbyPrDto],
    ) -> anyhow::Result<Option<String>> {
        let ws = self.workspace(workspace).await?;
        let head = &pr.change;
        let mut nearest: Option<&NearbyPrDto> = None;
        for other in stack.iter().filter(|o| o.number != pr.number) {
            let below = format!("({}) & ::({head}) ~ ({head})", other.change);
            if ws.jj.commit_ids(&below).await?.is_empty() {
                continue;
            }
            // A later one is nearer when the one found so far is below it.
            let nearer = match nearest {
                None => true,
                Some(n) => !ws
                    .jj
                    .commit_ids(&format!("({}) & ::({})", n.change, other.change))
                    .await?
                    .is_empty(),
            };
            if nearer {
                nearest = Some(other);
            }
        }
        Ok(nearest.map(|n| n.bookmark.clone()))
    }

    /// Pins stack PR `number`, making it the workspace's PR if it isn't, or unpins it.
    pub async fn pin_stack_pr(
        &self,
        workspace: Option<&str>,
        number: u32,
        pinned: bool,
    ) -> anyhow::Result<PrDto> {
        let key = workspace.unwrap_or("");
        let current = self.store.lock().unwrap().pr(key)?;
        let mut pr = match current {
            Some(p) if p.number == number => p,
            _ => {
                self.review_stack_pr(workspace, number).await?;
                self.store.lock().unwrap().pr(key)?.unwrap()
            }
        };
        if pr.stack {
            pr.pinned = pinned;
            self.store.lock().unwrap().save_pr(&pr)?;
        }
        Ok(PrDto::from(&pr))
    }

    /// Drops the workspace's stack PR once the review, `revset`, takes in none of its commits,
    /// unless it's pinned. Whether the workspace still has a PR.
    pub async fn follow_review(
        &self,
        workspace: Option<&str>,
        revset: &str,
    ) -> anyhow::Result<bool> {
        let key = workspace.unwrap_or("");
        let Some(pr) = self.store.lock().unwrap().pr(key)? else {
            return Ok(false);
        };
        if !pr.stack || pr.pinned {
            return Ok(true);
        }
        let ws = self.workspace(workspace).await?;
        let shared = format!("({}) & ({revset})", pr.revset());
        if !ws.jj.commit_ids(&shared).await?.is_empty() {
            return Ok(true);
        }
        self.store.lock().unwrap().delete_pr(key)?;
        Ok(false)
    }

    /// Drops the workspace's stack PR if it's PR `number`, pinned or not.
    pub fn drop_stack_pr(&self, workspace: Option<&str>, number: u32) -> anyhow::Result<()> {
        let key = workspace.unwrap_or("");
        let store = self.store.lock().unwrap();
        if store
            .pr(key)?
            .is_some_and(|p| p.stack && p.number == number)
        {
            store.delete_pr(key)?;
        }
        Ok(())
    }

    async fn save_stack_pr(
        &self,
        workspace: &str,
        pr: &NearbyPrDto,
        below: Option<String>,
    ) -> anyhow::Result<PrDto> {
        let (base, repo) = self.base_for(None).await?;
        let info = github::pr::info(&repo, pr.number).await?;
        let head_remote = match (&info.head_repository, info.is_cross_repository) {
            (Some(h), true) => Some(remote::fork_remote(&h.owner.login)),
            _ => Some(base.remote.clone()),
        };
        let record = PrRecord {
            workspace: workspace.to_string(),
            repo,
            number: pr.number,
            pinned: false,
            id: info.id,
            title: info.title,
            url: info.url,
            author: github::login(&info.author),
            base_ref: info.base_ref_name,
            base_remote: base.remote,
            head_ref: info.head_ref_name,
            head_remote,
            added_remote: None,
            head: pr.pushed.clone(),
            stack: true,
            below,
        };
        self.store.lock().unwrap().save_pr(&record)?;
        Ok(PrDto::from(&record))
    }
}
