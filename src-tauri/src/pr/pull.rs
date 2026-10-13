//! Pulling a PR's review threads into the review as comment blocks, and its conversation's
//! comments and review summaries as threads on the PR itself.

use std::collections::HashMap;

use crate::anchor::Side;
use crate::github::threads::{Comment, Review};
use crate::github::{self, threads::Thread};
use crate::review::{PulledThread, RepoCtx, StateDto};
use crate::store::{GhLink, GhPost};

pub(super) fn pulled(t: Thread, repo: &str, pr: u32, viewer: &str) -> PulledThread {
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
        .collect::<Vec<GhPost>>();
    let answered = posts.last().is_some_and(|p| p.author == viewer);
    let line = if t.is_outdated { None } else { t.line };
    PulledThread {
        link: GhLink {
            repo: repo.to_string(),
            pr,
            thread: t.id,
            url,
            resolved: t.is_resolved,
            outdated: t.is_outdated,
            posts,
            line,
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
        line,
        hunk,
        answered,
    }
}

/// Conversation comments and review summaries from others, each a thread on the PR. One the
/// viewer posted to the conversation after counts as answered.
pub(super) fn pr_level(
    comments: Vec<Comment>,
    reviews: Vec<Review>,
    repo: &str,
    pr: u32,
    viewer: &str,
) -> Vec<PulledThread> {
    let comments = comments
        .into_iter()
        .map(|c| (c.id, github::login(&c.author), c.body, c.created_at, c.url));
    let reviews = reviews
        .into_iter()
        .filter(|r| r.state != "PENDING" && !r.body.trim().is_empty())
        .map(|r| {
            let at = r.submitted_at.unwrap_or_default();
            (r.id, github::login(&r.author), r.body, at, r.url)
        });
    let all: Vec<_> = comments.chain(reviews).collect();
    let last_word = all
        .iter()
        .filter(|p| p.1 == viewer)
        .map(|p| p.3.clone())
        .max();
    all.into_iter()
        .filter(|p| p.1 != viewer)
        .map(|(id, author, body, created_at, url)| PulledThread {
            // RFC 3339 times in UTC compare as text.
            answered: last_word.as_ref().is_some_and(|w| *w > created_at),
            link: GhLink {
                repo: repo.to_string(),
                pr,
                thread: id,
                url,
                resolved: false,
                outdated: false,
                posts: vec![GhPost {
                    author,
                    body,
                    created_at,
                    pending: false,
                }],
                line: None,
            },
            path: String::new(),
            side: Side::New,
            start: None,
            line: None,
            hunk: String::new(),
        })
        .collect()
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
        let viewer = discussion.viewer;
        let mut threads: Vec<PulledThread> = discussion
            .threads
            .into_iter()
            .map(|t| pulled(t, &repo, pr.number, &viewer))
            .map(|mut t| {
                if !fetched && t.side == Side::New {
                    t.line = None;
                }
                t
            })
            .collect();
        threads.extend(pr_level(
            discussion.comments,
            discussion.reviews,
            &repo,
            pr.number,
            &viewer,
        ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::pr::Login;

    fn login(name: &str) -> Option<Login> {
        Some(Login { login: name.into() })
    }

    fn comment(author: &str, at: &str) -> Comment {
        Comment {
            id: format!("C-{author}-{at}"),
            author: login(author),
            body: "hi".into(),
            created_at: at.into(),
            url: String::new(),
        }
    }

    fn review(author: &str, state: &str, body: &str) -> Review {
        Review {
            id: format!("R-{author}"),
            author: login(author),
            body: body.into(),
            state: state.into(),
            submitted_at: Some("2026-01-02T00:00:00Z".into()),
            url: String::new(),
            viewer_did_author: author == "me",
        }
    }

    #[test]
    fn pr_comments_from_others_become_threads_answered_by_a_later_one_of_yours() {
        let comments = vec![
            comment("bob", "2026-01-01T00:00:00Z"),
            comment("me", "2026-01-03T00:00:00Z"),
            comment("ann", "2026-01-04T00:00:00Z"),
        ];
        let reviews = vec![
            review("bob", "CHANGES_REQUESTED", "Bigger question"),
            review("cat", "COMMENTED", " "),
            review("me", "PENDING", "draft"),
        ];
        let got: Vec<(String, bool)> = pr_level(comments, reviews, "o/n", 1, "me")
            .into_iter()
            .map(|t| (t.link.thread, t.answered))
            .collect();
        assert_eq!(
            got,
            [
                ("C-bob-2026-01-01T00:00:00Z".to_string(), true),
                ("C-ann-2026-01-04T00:00:00Z".to_string(), false),
                ("R-bob".to_string(), true),
            ]
        );
    }
}
