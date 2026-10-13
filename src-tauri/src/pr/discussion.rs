//! A PR's conversation, outside its code: comments and review summaries, read live from
//! GitHub. Replies post right away rather than going into the pending review.

use serde::Serialize;
use specta::Type;

use super::PrDto;
use crate::github;
use crate::github::threads::{Comment, Review};
use crate::review::RepoCtx;

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PostDto {
    pub author: String,
    pub body: String,
    /// For a review: APPROVED, CHANGES_REQUESTED, COMMENTED or DISMISSED.
    pub verdict: Option<String>,
    pub created_at: String,
    pub url: String,
}

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DiscussionDto {
    pub pr: PrDto,
    /// Oldest first.
    pub posts: Vec<PostDto>,
    /// Whether the viewer has a review that isn't submitted yet.
    pub pending_review: bool,
}

/// Conversation comments and submitted reviews that say something, oldest first.
pub(super) fn posts(comments: Vec<Comment>, reviews: Vec<Review>) -> Vec<PostDto> {
    let comments = comments.into_iter().map(|c| PostDto {
        author: github::login(&c.author),
        body: c.body,
        verdict: None,
        created_at: c.created_at,
        url: c.url,
    });
    // A COMMENTED review with no text only holds code comments, which show on the code.
    let reviews = reviews
        .into_iter()
        .filter(|r| r.state != "PENDING" && (r.state != "COMMENTED" || !r.body.trim().is_empty()))
        .map(|r| PostDto {
            author: github::login(&r.author),
            body: r.body,
            verdict: Some(r.state),
            created_at: r.submitted_at.unwrap_or_default(),
            url: r.url,
        });
    let mut posts: Vec<PostDto> = comments.chain(reviews).collect();
    // RFC 3339 times in UTC sort as text.
    posts.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    posts
}

impl RepoCtx {
    pub async fn pr_discussion(&self, workspace: Option<&str>) -> anyhow::Result<DiscussionDto> {
        let pr = self.need_pr(workspace)?;
        let d = github::threads::discussion(&pr.repo, pr.number).await?;
        let pending_review = d
            .reviews
            .iter()
            .any(|r| r.state == "PENDING" && r.viewer_did_author);
        let posts = posts(d.comments, d.reviews);
        Ok(DiscussionDto {
            pr: PrDto::from(&pr),
            posts,
            pending_review,
        })
    }

    /// Opens the PR's page on GitHub in the browser.
    pub async fn open_pr_page(&self, workspace: Option<&str>) -> anyhow::Result<()> {
        let pr = self.need_pr(workspace)?;
        github::gh::browse_pr(&pr.repo, pr.number).await
    }

    /// Posts to the PR's conversation.
    pub async fn reply_pr(&self, workspace: Option<&str>, body: &str) -> anyhow::Result<()> {
        let pr = self.need_pr(workspace)?;
        anyhow::ensure!(!body.trim().is_empty(), "write something to post first");
        github::publish::comment(&pr.id, body).await
    }
}
