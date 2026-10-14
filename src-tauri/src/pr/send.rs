//! Sending a PR's ready threads to GitHub. Replies on code and new threads go into one review,
//! so reviewers get one notification; replies to comments on the PR itself post as PR comments
//! that quote what they answer.

use anyhow::Context;
use serde::Deserialize;
use specta::Type;

use super::PrRecord;
use crate::anchor::Side;
use crate::github::{self, Verdict, publish::Spot};
use crate::review::{RepoCtx, StateDto, now_ms};
use crate::store::{Comment, GhLink, GhPost, Stage};

/// What happens to a ready thread with no reply: one that was fixed as asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Fixed {
    Leave,
    Reply,
    Resolve,
    ReplyResolve,
}

impl Fixed {
    fn reply(self) -> bool {
        matches!(self, Fixed::Reply | Fixed::ReplyResolve)
    }

    fn resolve(self) -> bool {
        matches!(self, Fixed::Resolve | Fixed::ReplyResolve)
    }
}

/// The reply `Fixed::Reply` sends.
const DONE: &str = "Done";

fn side(s: Side) -> &'static str {
    match s {
        Side::Old => "LEFT",
        Side::New => "RIGHT",
    }
}

/// A reply to a comment on the PR, opening with a short quote of it.
fn quoted(author: &str, body: &str, reply: &str) -> String {
    let first = body
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let short: String = first.chars().take(100).collect();
    let more = if short.len() < first.len() { "…" } else { "" };
    format!("> @{author}: {short}{more}\n\n{reply}")
}

impl RepoCtx {
    /// Sends every ready thread on the review workspace's PR, then submits the review with
    /// `verdict` and `summary`, and pulls the PR's threads again.
    pub async fn send_threads(
        &self,
        id: u32,
        fixed: Fixed,
        verdict: Verdict,
        summary: &str,
    ) -> anyhow::Result<StateDto> {
        let ws = self.review_workspace(id)?;
        let pr = self.need_pr(ws.name.as_deref())?;
        let ready: Vec<Comment> = self
            .store
            .lock()
            .unwrap()
            .pr_threads(&pr.repo.to_string(), pr.number)?
            .into_iter()
            .filter(|c| c.thread.as_ref().is_some_and(|t| t.stage == Stage::Ready))
            .collect();
        self.check_lines(id, &pr, &ready)?;
        let mut reviewed = false;
        for c in &ready {
            match self.send_one(id, &pr, c, fixed).await {
                Ok(in_review) => reviewed |= in_review,
                Err(e) if reviewed => anyhow::bail!(
                    "{e:#}. What went out before it is in your pending review on GitHub: fix \
                     this thread or move it out of Ready, then send again to submit the review"
                ),
                Err(e) => return Err(e),
            }
        }
        let say = verdict != Verdict::Comment || !summary.trim().is_empty();
        if reviewed || say || github::publish::has_pending(&pr.id).await? {
            github::publish::submit(&pr.id, &pr.head, verdict, summary).await?;
        }
        self.pull_sent(id).await
    }

    /// Sends one ready thread, submitting the review its reply goes into, and pulls the PR's
    /// threads again.
    pub async fn send_thread(&self, id: u32, comment: u32) -> anyhow::Result<StateDto> {
        let ws = self.review_workspace(id)?;
        let pr = self.need_pr(ws.name.as_deref())?;
        let c = self
            .store
            .lock()
            .unwrap()
            .comment(comment as i64)?
            .context("the thread is gone")?;
        anyhow::ensure!(
            c.thread.as_ref().is_some_and(|t| t.stage == Stage::Ready),
            "make the reply final first"
        );
        self.check_lines(id, &pr, std::slice::from_ref(&c))?;
        if self.send_one(id, &pr, &c, Fixed::Leave).await? {
            github::publish::submit(&pr.id, &pr.head, Verdict::Comment, "").await?;
        }
        self.pull_sent(id).await
    }

    /// New comments go on GitHub's lines, so they need the whole PR in view.
    fn check_lines(&self, id: u32, pr: &PrRecord, threads: &[Comment]) -> anyhow::Result<()> {
        let drafts = threads
            .iter()
            .any(|c| c.github.is_none() && !c.body.trim().is_empty());
        anyhow::ensure!(
            !drafts || self.shows_whole(id, &pr.revset())?,
            "open the whole PR ({}) in the Combined view to send new comments, where GitHub's \
             lines match",
            pr.revset()
        );
        Ok(())
    }

    /// The state after sending, with the PR's threads pulled again.
    async fn pull_sent(&self, id: u32) -> anyhow::Result<StateDto> {
        match self.pull_pr_comments(id).await {
            Ok(state) => Ok(state),
            // It's sent either way, so show that.
            Err(_) => self.comment_edit(id, |_, _| Ok(())),
        }
    }

    /// Sends one ready thread and marks it done. Returns whether it went into the pending
    /// review.
    async fn send_one(
        &self,
        id: u32,
        pr: &PrRecord,
        c: &Comment,
        fixed: Fixed,
    ) -> anyhow::Result<bool> {
        let text = c.body.trim();
        let mut in_review = false;
        let link = match &c.github {
            None if text.is_empty() => return Ok(false),
            None => {
                in_review = true;
                Some(self.add_draft(id, pr, c, text).await?)
            }
            Some(l) => {
                let reply = match (text.is_empty(), fixed.reply()) {
                    (false, _) => Some(text),
                    (true, true) => Some(DONE),
                    (true, false) => None,
                };
                match (reply, l.posts.first()) {
                    (Some(reply), Some(post)) if c.path.is_empty() => {
                        let body = quoted(&post.author, &post.body, reply);
                        github::publish::comment(&pr.id, &body).await?;
                    }
                    (Some(reply), _) => {
                        github::publish::reply(&pr.id, &pr.head, &l.thread, reply).await?;
                        in_review = true;
                    }
                    (None, _) => {}
                }
                if text.is_empty() && fixed.resolve() && !c.path.is_empty() && !l.resolved {
                    github::publish::resolve(&l.thread).await?;
                }
                c.github.clone()
            }
        };
        let store = self.store.lock().unwrap();
        let mut thread = c.thread.clone();
        if let Some(t) = &mut thread {
            t.stage = Stage::Done;
        }
        store.set_thread(c.id, &thread)?;
        store.set_github(c.id, &link)?;
        store.clear_body_if(c.id, &c.body, now_ms())?;
        Ok(in_review)
    }

    /// Starts a draft's thread on its code in the pending review.
    async fn add_draft(
        &self,
        id: u32,
        pr: &PrRecord,
        c: &Comment,
        body: &str,
    ) -> anyhow::Result<GhLink> {
        let ws = self.review_workspace(id)?;
        let text = ws.jj.file(Some(&pr.head), &c.path).await.ok();
        let text = text.and_then(|b| String::from_utf8(b).ok());
        let spot = self.comment_spot(id, c.id as u32, text.as_deref())?;
        let at = Spot {
            path: &spot.path,
            side: side(spot.side),
            line: spot.line,
            start: spot.start.map(|(s, l)| (side(s), l)),
        };
        let thread = github::publish::add_thread(&pr.id, &pr.head, &at, body).await?;
        Ok(GhLink {
            repo: pr.repo.to_string(),
            pr: pr.number,
            thread,
            url: pr.url.clone(),
            resolved: false,
            outdated: false,
            // Shown until the pull after sending brings GitHub's copy.
            posts: vec![GhPost {
                author: "you".into(),
                body: body.to_string(),
                created_at: String::new(),
                pending: true,
            }],
            line: Some(spot.line),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_to_pr_comments_quote_their_first_line() {
        assert_eq!(
            quoted(
                "bob",
                "\nShould this be configurable?\nMonorepos…",
                "It is."
            ),
            "> @bob: Should this be configurable?\n\nIt is."
        );
        let long = "x".repeat(150);
        let got = quoted("bob", &long, "ok");
        assert!(got.starts_with(&format!("> @bob: {}…\n\n", "x".repeat(100))));
    }
}
