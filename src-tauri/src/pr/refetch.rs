//! Seeing whether a PR moved on GitHub, and fetching its new head into the workspace.

use serde::Serialize;
use specta::Type;

use super::checkout::fetch;
use crate::github;
use crate::jj::Jj;
use crate::review::RepoCtx;

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HeadDto {
    /// The head on GitHub isn't the fetched one.
    pub moved: bool,
    /// Commits the new head has that the fetched one doesn't.
    pub ahead: u32,
    /// The fetched head isn't under the new one: a force-push.
    pub rewritten: bool,
    /// OPEN, CLOSED or MERGED.
    pub state: String,
}

/// Moves the workspace's own commits, the ones above `old`, onto `new`. A workspace editing
/// `old` itself gets a new commit on `new`. One that moved off the PR, or that `new` already
/// includes (its own commit was pushed), is left alone.
pub(super) async fn move_onto(jj: &Jj, old: &str, new: &str) -> anyhow::Result<()> {
    let has = |revset: String| async move { jj.commit_ids(&revset).await.map(|c| !c.is_empty()) };
    if jj.commit_ids("@").await? == [old] {
        return jj.new_on(new).await;
    }
    if !has(format!("{old} & ::@")).await? || has(format!("{new} & ::@")).await? {
        return Ok(());
    }
    let ours = format!("({old}..@) ~ ::{new}");
    if has(ours.clone()).await? {
        jj.rebase(&format!("roots({ours})"), new).await?;
    }
    Ok(())
}

impl RepoCtx {
    /// How the PR's head on GitHub compares with the fetched one.
    pub async fn pr_head(&self, workspace: Option<&str>) -> anyhow::Result<HeadDto> {
        let pr = self.need_pr(workspace)?;
        let info = github::pr::info(&pr.repo, pr.number).await?;
        let moved = info.head_ref_oid != pr.head;
        let (ahead, kept) = if moved {
            github::pr::compare(&pr.repo, &pr.head, &info.head_ref_oid)
                .await
                .unwrap_or((0, true))
        } else {
            (0, true)
        };
        Ok(HeadDto {
            moved,
            ahead,
            rewritten: moved && !kept,
            state: info.state,
        })
    }

    /// Fetches the PR's head and base again and moves the workspace's own commits onto the
    /// new head. With `review`, that review's session ends first, so it can show what changed.
    pub async fn refetch_pr(
        &self,
        workspace: Option<&str>,
        review: Option<u32>,
    ) -> anyhow::Result<()> {
        let mut pr = self.need_pr(workspace)?;
        let ws = self.workspace(workspace).await?;
        let _busy = ws.claim()?;
        let info = github::pr::info(&pr.repo, pr.number).await?;
        let base = self.base_of(&pr).await?;
        let fetched = fetch(&self.main.jj, &info, &base).await?;
        let (old, new) = (pr.head.clone(), info.head_ref_oid.clone());
        if old != new {
            // Only now, so a failed or empty refetch keeps the review's previous session.
            if let Some(id) = review {
                self.seal_session(id)?;
            }
            ws.snapshot().await?;
            move_onto(&ws.jj, &old, &new).await?;
        }
        pr.head = new;
        pr.base_remote = base.remote;
        pr.head_remote = fetched.head_remote;
        pr.added_remote = pr.added_remote.or(fetched.added_remote);
        pr.title = info.title;
        pr.base_ref = info.base_ref_name;
        self.store.lock().unwrap().save_pr(&pr)?;
        Ok(())
    }
}
