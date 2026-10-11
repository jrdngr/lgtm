//! Pulling a PR's review threads into the review as comment blocks.

use std::collections::HashMap;

use crate::anchor::Side;
use crate::github::{self, threads::Thread};
use crate::review::{PulledThread, RepoCtx, StateDto};
use crate::store::{GhLink, GhPost};

fn pulled(t: Thread, repo: &str, pr: u32) -> PulledThread {
    let url = t
        .comments
        .nodes
        .first()
        .map(|c| c.url.clone())
        .unwrap_or_default();
    let hunk = t
        .comments
        .nodes
        .first()
        .map(|c| c.diff_hunk.clone())
        .unwrap_or_default();
    let posts = t
        .comments
        .nodes
        .into_iter()
        .map(|c| GhPost {
            author: github::login(&c.author),
            body: c.body,
            created_at: c.created_at,
            pending: c.state == "PENDING",
        })
        .collect();
    PulledThread {
        link: GhLink {
            repo: repo.to_string(),
            pr,
            thread: t.id,
            url,
            resolved: t.is_resolved,
            outdated: t.is_outdated,
            posts,
        },
        path: t.path,
        side: if t.diff_side == "LEFT" {
            Side::Old
        } else {
            Side::New
        },
        // A range from a removed line to an added one keeps just its end.
        start: t
            .start_line
            .filter(|_| t.start_diff_side.as_deref() == Some(&t.diff_side)),
        line: if t.is_outdated { None } else { t.line },
        hunk,
    }
}

impl RepoCtx {
    /// Pulls the review threads of the PR the review's workspace is for into the review.
    pub async fn pull_pr_comments(&self, id: u32) -> anyhow::Result<StateDto> {
        let ws = self.review_workspace(id)?;
        let pr = self.whole_pr(id)?;
        let discussion = github::threads::discussion(&pr.repo, pr.number).await?;
        let repo = pr.repo.to_string();
        // GitHub numbers lines on its head. Until that's fetched, threads on it wait unplaced,
        // and the pull after the refetch places them.
        let fetched = discussion.head == pr.head;
        let threads: Vec<PulledThread> = discussion
            .threads
            .into_iter()
            .map(|t| pulled(t, &repo, pr.number))
            .map(|mut t| {
                if !fetched && t.side == Side::New {
                    t.line = None;
                }
                t
            })
            .collect();
        let mut head_texts = HashMap::new();
        for t in threads
            .iter()
            .filter(|t| t.line.is_some() && t.side == Side::New)
        {
            if head_texts.contains_key(&t.path) {
                continue;
            }
            if let Ok(bytes) = ws.jj.file(Some(&pr.head), &t.path).await
                && let Ok(text) = String::from_utf8(bytes)
            {
                head_texts.insert(t.path.clone(), text);
            }
        }
        self.add_threads(id, threads, &head_texts, &format!("PR #{}", pr.number))
    }
}
