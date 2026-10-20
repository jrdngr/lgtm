//! A PR picked from a workspace's stack instead of checked out into a workspace of its own:
//! the PR of the nearest bookmark at or below `@`, kept while `@` builds on it.

use std::collections::HashMap;
use std::sync::Mutex;

use anyhow::{Context, bail};

use super::nearby::NearbyPrDto;
use super::{PrDto, PrRecord};
use crate::ctx::RepoCtx;
use crate::github::{self, remote};

/// The bookmark of a PR deselected in a workspace, by repo and workspace. It isn't picked
/// again until another PR is nearest `@`.
static DISMISSED: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

/// The PR `@` builds on: the nearest one at or below it.
fn nearest(stack: &[NearbyPrDto]) -> Option<&NearbyPrDto> {
    stack.iter().find(|p| p.under)
}

impl RepoCtx {
    fn dismissed_key(&self, workspace: &str) -> String {
        format!("{}\n{workspace}", self.root.display())
    }

    fn set_dismissed(&self, workspace: &str, bookmark: Option<&str>) {
        let key = self.dismissed_key(workspace);
        let mut all = DISMISSED.lock().unwrap();
        let all = all.get_or_insert_default();
        match bookmark {
            Some(b) => all.insert(key, b.to_string()),
            None => all.remove(&key),
        };
    }

    fn dismissed(&self, workspace: &str) -> Option<String> {
        let key = self.dismissed_key(workspace);
        DISMISSED.lock().unwrap().as_ref()?.get(&key).cloned()
    }

    /// Brings the workspace's stack PR in line with `@`: switches to the nearest PR under `@`
    /// unless that one was deselected, and flags the PR as off the stack while no other one
    /// takes its place. A workspace checked out for a PR keeps it. Returns the workspace's PR.
    pub async fn sync_stack_pr(&self, workspace: Option<&str>) -> anyhow::Result<Option<PrDto>> {
        let key = workspace.unwrap_or("");
        let current = self.store.lock().unwrap().pr(key)?;
        if let Some(pr) = current.as_ref().filter(|p| !p.stack) {
            return Ok(Some(PrDto::from(pr)));
        }
        let stack = self.nearby_prs(workspace, false).await?.prs;
        let near = nearest(&stack);
        let dismissed = self.dismissed(key);
        if dismissed.is_some() && dismissed.as_deref() != near.map(|p| p.bookmark.as_str()) {
            self.set_dismissed(key, None);
        }
        let dismissed = self.dismissed(key);
        let next = near.filter(|n| dismissed.as_deref() != Some(n.bookmark.as_str()));
        if let Some(mut pr) = current {
            let was = (pr.head.clone(), pr.off);
            match (near, next) {
                (Some(n), _) if n.bookmark == pr.head_ref => {
                    // A push moved what GitHub shows.
                    pr.head = n.pushed.clone();
                    pr.off = false;
                }
                (_, Some(n)) => {
                    self.store.lock().unwrap().delete_pr(key)?;
                    return self.save_stack_pr(key, n).await.map(Some);
                }
                _ => pr.off = true,
            }
            if (pr.head.clone(), pr.off) != was {
                self.store.lock().unwrap().save_pr(&pr)?;
            }
            return Ok(Some(PrDto::from(&pr)));
        }
        match next {
            Some(n) => self.save_stack_pr(key, n).await.map(Some),
            None => Ok(None),
        }
    }

    /// Picks the stack PR numbered `number`, which `@` has to build on.
    pub async fn select_stack_pr(
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
        if !pr.under {
            bail!("edit {} first: @ doesn't build on PR #{number}", pr.change);
        }
        self.set_dismissed(key, None);
        self.save_stack_pr(key, pr).await
    }

    /// Edits the change of stack PR `number`, then picks it.
    pub async fn edit_stack_pr(
        &self,
        workspace: Option<&str>,
        number: u32,
    ) -> anyhow::Result<PrDto> {
        let stack = self.nearby_prs(workspace, false).await?.prs;
        let Some(pr) = stack.iter().find(|p| p.number == number) else {
            bail!("PR #{number} isn't on this workspace's stack");
        };
        let ws = self.workspace(workspace).await?;
        ws.jj
            .edit(&pr.change)
            .await
            .with_context(|| format!("edit {}", pr.change))?;
        self.select_stack_pr(workspace, number).await
    }

    /// Edits the change of the workspace's stack PR, which `@` left, putting it back on it.
    pub async fn edit_back_to_stack_pr(
        &self,
        workspace: Option<&str>,
    ) -> anyhow::Result<Option<PrDto>> {
        let key = workspace.unwrap_or("");
        let pr = self.store.lock().unwrap().pr(key)?;
        let Some(pr) = pr.filter(|p| p.stack) else {
            bail!("this workspace has no PR picked from its stack");
        };
        let ws = self.workspace(workspace).await?;
        ws.jj
            .edit(&format!("latest({})", pr.head_revset()))
            .await
            .with_context(|| format!("edit {}", pr.head_ref))?;
        self.sync_stack_pr(workspace).await
    }

    /// Deselects the workspace's stack PR. It isn't picked again until another PR is nearest.
    pub fn clear_stack_pr(&self, workspace: Option<&str>) -> anyhow::Result<()> {
        let key = workspace.unwrap_or("");
        let store = self.store.lock().unwrap();
        let Some(pr) = store.pr(key)? else {
            return Ok(());
        };
        if !pr.stack {
            bail!("this workspace was checked out for its own PR");
        }
        store.delete_pr(key)?;
        self.set_dismissed(key, Some(&pr.head_ref));
        Ok(())
    }

    async fn save_stack_pr(&self, workspace: &str, pr: &NearbyPrDto) -> anyhow::Result<PrDto> {
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
            off: false,
        };
        self.store.lock().unwrap().save_pr(&record)?;
        Ok(PrDto::from(&record))
    }
}
