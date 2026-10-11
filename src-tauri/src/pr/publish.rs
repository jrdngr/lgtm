//! Publishing comment blocks into the viewer's pending review, and submitting it.

use anyhow::Context;

use crate::anchor::Side;
use crate::github::{self, Verdict, publish::Spot};
use crate::review::{RepoCtx, StateDto, now_ms};
use crate::store::{GhLink, GhPost};

fn side(s: Side) -> &'static str {
    match s {
        Side::Old => "LEFT",
        Side::New => "RIGHT",
    }
}

impl RepoCtx {
    /// Publishes a comment block's text: as a reply when it's on a GitHub thread, else as a new
    /// thread on its code. It goes into the pending review, then the PR's threads are pulled.
    pub async fn publish_comment(&self, id: u32, comment: u32) -> anyhow::Result<StateDto> {
        let ws = self.review_workspace(id)?;
        let pr = self.whole_pr(id)?;
        let cid = comment as i64;
        let c = self
            .store
            .lock()
            .unwrap()
            .comment(cid)?
            .context("the comment is gone")?;
        let body = c.body.trim();
        anyhow::ensure!(!body.is_empty(), "write something to publish first");
        let link = match &c.github {
            Some(link) => {
                github::publish::reply(&pr.id, &pr.head, &link.thread, body).await?;
                link.clone()
            }
            None => {
                let text = ws.jj.file(Some(&pr.head), &c.path).await.ok();
                let text = text.and_then(|b| String::from_utf8(b).ok());
                let spot = self.comment_spot(id, comment, text.as_deref())?;
                let at = Spot {
                    path: &spot.path,
                    side: side(spot.side),
                    line: spot.line,
                    start: spot.start.map(|(s, l)| (side(s), l)),
                };
                let thread = github::publish::add_thread(&pr.id, &pr.head, &at, body).await?;
                GhLink {
                    repo: pr.repo.to_string(),
                    pr: pr.number,
                    thread,
                    url: pr.url.clone(),
                    resolved: false,
                    outdated: false,
                    posts: vec![],
                }
            }
        };
        let mut link = link;
        // Shown until the pull below brings GitHub's copy.
        link.posts.push(GhPost {
            author: "you".into(),
            body: body.to_string(),
            created_at: String::new(),
            pending: true,
        });
        {
            let store = self.store.lock().unwrap();
            store.set_github(cid, &Some(link))?;
            store.clear_body_if(cid, &c.body, now_ms())?;
        }
        match self.pull_pr_comments(id).await {
            Ok(state) => Ok(state),
            // It's published either way, so show that.
            Err(_) => self.comment_edit(id, |_, _| Ok(())),
        }
    }

    /// Submits the viewer's pending review with a verdict and summary.
    pub async fn submit_review(
        &self,
        workspace: Option<&str>,
        verdict: Verdict,
        body: &str,
    ) -> anyhow::Result<()> {
        let pr = self.need_pr(workspace)?;
        github::publish::submit(&pr.id, &pr.head, verdict, body).await
    }
}
