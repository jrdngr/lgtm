//! Threads pulled from a PR on GitHub, kept as comments in the store. The desktop and terminal
//! apps share them, so a thread has the same id in both.

use std::collections::HashMap;

use super::login;
use super::threads::Thread;
use crate::anchor::Side;
use crate::review::place::{Placer, ThreadLines};
use crate::store::{Comment, Conversation, GhLink, GhPost, Stage, Store, ThreadState};

/// A thread pulled from GitHub, to become a comment block.
pub struct PulledThread {
    pub link: GhLink,
    pub path: String,
    pub side: Side,
    pub start: Option<u32>,
    /// `None` once outdated.
    pub line: Option<u32>,
    /// GitHub's diff hunk, the snippet when the code can't be found.
    pub hunk: String,
    /// The viewer had the last word.
    pub answered: bool,
}

impl PulledThread {
    pub fn lines(&self) -> ThreadLines<'_> {
        ThreadLines {
            path: &self.path,
            side: self.side,
            start: self.start,
            line: self.line,
        }
    }
}

pub fn pulled(t: Thread, repo: &str, pr: u32, viewer: &str) -> PulledThread {
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
            author: login(&c.author),
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

/// A pulled thread's stage. `old` is the thread as last pulled. `answered` means the viewer
/// had the last word. A thread someone posted to since comes back to decide, and one the viewer
/// replied to since, from anywhere, is done.
pub fn pulled_stage(old: Option<(&ThreadState, &GhLink)>, new: &GhLink, answered: bool) -> Stage {
    let Some((state, link)) = old else {
        return if new.resolved || answered {
            Stage::Done
        } else {
            Stage::Decide
        };
    };
    let posted = new.posts.len() > link.posts.len();
    if posted && !answered && !new.resolved {
        Stage::Decide
    } else if (posted && answered) || (new.resolved && !link.resolved) {
        Stage::Done
    } else {
        state.stage
    }
}

/// Adds pulled threads as comments, and updates the ones already pulled. `head_texts` are
/// the threads' files at the PR head; `label` names where they came from.
pub fn add_threads(
    store: &Store,
    placer: &Placer,
    threads: Vec<PulledThread>,
    head_texts: &HashMap<String, String>,
    label: &str,
    now: i64,
) -> anyhow::Result<()> {
    // Each known thread's comment.
    let mut known: HashMap<String, Comment> = HashMap::new();
    if let Some(first) = threads.first() {
        for c in store.pr_threads(&first.link.repo, first.link.pr)? {
            if let Some(g) = &c.github {
                known.insert(g.thread.clone(), c);
            }
        }
    }
    for t in threads {
        let rows = placer.thread_rows(&t.lines(), head_texts.get(&t.path).map(String::as_str));
        if let Some(c) = known.get(&t.link.thread) {
            let (Some(old), Some(link)) = (&c.thread, &c.github) else {
                continue;
            };
            let mut state = old.clone();
            state.stage = pulled_stage(Some((old, link)), &t.link, t.answered);
            // A reply posted from elsewhere, like a terminal agent, isn't left to send.
            let sent = t.link.posts.last().is_some_and(|p| {
                t.answered && !c.body.trim().is_empty() && p.body.trim() == c.body.trim()
            });
            if sent {
                store.set_comment_body(c.id, "", now)?;
            }
            store.set_thread(c.id, &Some(state))?;
            store.set_github(c.id, &Some(t.link))?;
            // A thread pulled while its code wasn't in the review gets placed now.
            if let (true, false, Some(rows)) = (c.pins.is_empty(), t.path.is_empty(), &rows) {
                let (path, pins) = placer.pins(store, rows)?;
                store.set_comment_pins(c.id, &path, &pins, &placer.snippet(rows))?;
            }
            continue;
        }
        let (path, pins) = match &rows {
            Some(rows) => placer.pins(store, rows)?,
            None => (t.path.clone(), vec![]),
        };
        let snippet = rows.map_or(t.hunk.clone(), |r| placer.snippet(&r));
        let stage = pulled_stage(None, &t.link, t.answered);
        let thread = ThreadState::new(&t.link.repo, t.link.pr, stage);
        store.add_comment(&Comment {
            id: 0,
            path,
            pins,
            body: String::new(),
            snippet,
            review: label.to_string(),
            created_at: now,
            updated_at: now,
            conversation: Conversation::default(),
            github: Some(t.link),
            thread: Some(thread),
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(posts: usize, resolved: bool) -> GhLink {
        let post = GhPost {
            author: "bob".into(),
            body: "why?".into(),
            created_at: String::new(),
            pending: false,
        };
        GhLink {
            repo: "o/n".into(),
            pr: 1,
            thread: "T".into(),
            url: String::new(),
            resolved,
            outdated: false,
            posts: vec![post; posts],
            line: None,
        }
    }

    #[test]
    fn pulling_keeps_or_reopens_a_threads_stage() {
        let ready = ThreadState::new("o/n", 1, Stage::Ready);
        let done = ThreadState::new("o/n", 1, Stage::Done);
        assert_eq!(pulled_stage(None, &link(1, false), false), Stage::Decide);
        assert_eq!(pulled_stage(None, &link(1, true), false), Stage::Done);
        assert_eq!(pulled_stage(None, &link(2, false), true), Stage::Done);
        let old = link(1, false);
        assert_eq!(
            pulled_stage(Some((&ready, &old)), &link(1, false), false),
            Stage::Ready
        );
        assert_eq!(
            pulled_stage(Some((&done, &old)), &link(2, false), false),
            Stage::Decide
        );
        assert_eq!(
            pulled_stage(Some((&done, &old)), &link(2, false), true),
            Stage::Done
        );
        assert_eq!(
            pulled_stage(Some((&ready, &old)), &link(1, true), false),
            Stage::Done
        );
        // Replied to outside lgtm, like from a terminal agent.
        let reply = ThreadState::new("o/n", 1, Stage::Reply);
        assert_eq!(
            pulled_stage(Some((&reply, &old)), &link(2, false), true),
            Stage::Done
        );
    }
}
