//! A PR workspace's threads: every GitHub thread and draft on its PR, whether or not the
//! review shows its code, and the workflow stages they move through.

use anyhow::Context;
use serde::Serialize;
use specta::Type;

use super::{RepoCtx, Review, StateDto, now_ms};
use crate::store::{Author, Comment, GhLink, Message, Stage, Store, ThreadState, Workshop};

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ThreadDto {
    /// Its comment's id.
    pub id: u32,
    /// Empty for a comment on the PR itself.
    pub path: String,
    /// Its last line, as GitHub numbers it or as drafted.
    pub line: Option<u32>,
    /// The reply not yet sent, or a draft's text.
    pub body: String,
    pub state: ThreadState,
    pub github: Option<GhLink>,
    /// The code it's on as it was, or GitHub's diff hunk.
    pub snippet: String,
    /// The agent conversation about it.
    pub messages: Vec<Message>,
    /// Whether the review shows its code.
    pub placed: bool,
    pub created_at: f64,
}

/// The stage after a thread's reply text changed to `body`. A draft is ready once it says
/// something; a reply being written, or rewritten after it was final, is being worked out.
pub fn after_edit(stage: Stage, draft: bool, body: &str) -> Stage {
    match (body.trim().is_empty(), draft) {
        (true, true) => Stage::Reply,
        (true, false) => stage,
        (false, true) => Stage::Ready,
        (false, false) => Stage::Reply,
    }
}

/// A pulled thread's stage. `old` is the thread as last pulled. `answered` means the viewer
/// had the last word. A thread someone posted to since comes back as new, and one the viewer
/// replied to since, from anywhere, is done.
pub fn pulled_stage(old: Option<(&ThreadState, &GhLink)>, new: &GhLink, answered: bool) -> Stage {
    let Some((state, link)) = old else {
        return if new.resolved || answered {
            Stage::Done
        } else {
            Stage::New
        };
    };
    let posted = new.posts.len() > link.posts.len();
    if posted && !answered && !new.resolved {
        Stage::New
    } else if (posted && answered) || (new.resolved && !link.resolved) {
        Stage::Done
    } else {
        state.stage
    }
}

/// Inline threads in file and line order, then comments on the PR by time.
fn order(t: &ThreadDto) -> (bool, String, u32, String) {
    let first = t.github.as_ref().and_then(|g| g.posts.first());
    let at = first.map_or(String::new(), |p| p.created_at.clone());
    (t.path.is_empty(), t.path.clone(), t.line.unwrap_or(0), at)
}

impl Review {
    /// The PR of the review's workspace, as `(repo, number)`.
    pub(super) fn pr(&self, store: &Store) -> anyhow::Result<Option<(String, u32)>> {
        Ok(store.pr(self.ws.key())?.map(|p| (p.repo.to_string(), p.number)))
    }

    pub(super) fn rematch_threads(&mut self, store: &Store) -> anyhow::Result<()> {
        let Some((repo, pr)) = self.pr(store)? else {
            self.threads.clear();
            return Ok(());
        };
        let placed = |id: i64| {
            self.comments
                .iter()
                .any(|c| c.id as i64 == id && c.rows.is_some())
        };
        let mut threads: Vec<ThreadDto> = store
            .pr_threads(&repo, pr)?
            .into_iter()
            .map(|c| ThreadDto {
                id: c.id as u32,
                placed: placed(c.id),
                line: match &c.github {
                    Some(g) => g.line,
                    None => c.pins.last().map(|p| p.line),
                },
                path: c.path,
                body: c.body,
                snippet: c.snippet,
                messages: c.workshop.messages,
                state: c.thread.expect("pr threads have a state"),
                github: c.github,
                created_at: c.created_at as f64,
            })
            .collect();
        threads.sort_by_key(order);
        self.threads = threads;
        Ok(())
    }

    /// A new draft's state, on the workspace's PR.
    pub(super) fn draft_state(&self, store: &Store, body: &str) -> anyhow::Result<ThreadState> {
        let (repo, pr) = self
            .pr(store)?
            .context("drafts for GitHub need a PR workspace")?;
        Ok(ThreadState::new(
            &repo,
            pr,
            after_edit(Stage::Reply, true, body),
        ))
    }
}

impl RepoCtx {
    /// Moves a thread to `stage` by hand.
    pub fn set_stage(&self, id: u32, comment: u32, stage: Stage) -> anyhow::Result<StateDto> {
        self.comment_edit(id, |_, store| {
            let cid = comment as i64;
            let c = store.comment(cid)?.context("the thread is gone")?;
            let mut t = c.thread.context("notes have no stage")?;
            t.stage = stage;
            Ok(store.set_thread(cid, &Some(t))?)
        })
    }

    /// Clears a GitHub thread's take, reply and agent conversation, and puts it back where a
    /// freshly pulled thread starts.
    pub fn reset_thread(&self, id: u32, comment: u32) -> anyhow::Result<StateDto> {
        self.comment_edit(id, |_, store| {
            let cid = comment as i64;
            let c = store.comment(cid)?.context("the thread is gone")?;
            let link = c.github.context("only GitHub threads reset")?;
            store.set_comment_body(cid, "", now_ms())?;
            store.set_workshop(cid, &Workshop::default())?;
            Ok(store.set_thread(cid, &Some(ThreadState::from_link(&link)))?)
        })
    }

    /// Makes a thread's suggested reply its reply with `apply`, or drops it.
    pub fn settle_suggestion(
        &self,
        id: u32,
        comment: u32,
        apply: bool,
    ) -> anyhow::Result<StateDto> {
        self.comment_edit(id, |_, store| {
            let cid = comment as i64;
            let mut c = store.comment(cid)?.context("the thread is gone")?;
            let draft = c.is_draft();
            let t = c.thread.as_mut().context("notes have no suggestion")?;
            let Some(text) = t.suggestion.take() else {
                return Ok(());
            };
            if apply {
                let before = std::mem::replace(&mut c.body, text);
                t.stage = after_edit(t.stage, draft, &c.body);
                store.set_comment_body(cid, &c.body, now_ms())?;
                // The review that suggested it made the reply's new version.
                let m = &mut c.workshop.messages;
                let made = (1..m.len())
                    .rev()
                    .find(|&i| m[i].author == Author::Agent && m[i - 1].compose);
                if let Some(i) = made {
                    m[i].before = Some(before);
                    store.set_workshop(cid, &c.workshop)?;
                }
            }
            Ok(store.set_thread(cid, &c.thread)?)
        })
    }

    /// Marks a note done, or open again.
    pub fn set_note_done(&self, id: u32, comment: u32, done: bool) -> anyhow::Result<StateDto> {
        self.comment_edit(id, |_, store| {
            Ok(store.set_comment_done(comment as i64, done)?)
        })
    }

    /// Starts a GitHub draft on a note's code, with the note's text.
    pub fn draft_from_note(&self, id: u32, comment: u32) -> anyhow::Result<StateDto> {
        self.comment_edit(id, |review, store| {
            let note = store.comment(comment as i64)?.context("the note is gone")?;
            anyhow::ensure!(note.thread.is_none(), "that's already a thread");
            let now = now_ms();
            store.add_comment(&Comment {
                id: 0,
                thread: Some(review.draft_state(store, &note.body)?),
                review: review.label.clone(),
                created_at: now,
                updated_at: now,
                workshop: Workshop::default(),
                github: None,
                done: false,
                ..note
            })?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::GhPost;

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
    fn editing_a_reply_moves_its_stage() {
        assert_eq!(after_edit(Stage::Reply, true, "nit"), Stage::Ready);
        assert_eq!(after_edit(Stage::Ready, true, " "), Stage::Reply);
        assert_eq!(after_edit(Stage::Ready, false, "rewritten"), Stage::Reply);
        assert_eq!(after_edit(Stage::New, false, "because"), Stage::Reply);
        // Clearing a reply leaves a fixed thread ready to go without one.
        assert_eq!(after_edit(Stage::Ready, false, ""), Stage::Ready);
    }

    #[test]
    fn pulling_keeps_or_reopens_a_threads_stage() {
        let ready = ThreadState::new("o/n", 1, Stage::Ready);
        let done = ThreadState::new("o/n", 1, Stage::Done);
        assert_eq!(pulled_stage(None, &link(1, false), false), Stage::New);
        assert_eq!(pulled_stage(None, &link(1, true), false), Stage::Done);
        assert_eq!(pulled_stage(None, &link(2, false), true), Stage::Done);
        let old = link(1, false);
        assert_eq!(
            pulled_stage(Some((&ready, &old)), &link(1, false), false),
            Stage::Ready
        );
        assert_eq!(
            pulled_stage(Some((&done, &old)), &link(2, false), false),
            Stage::New
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
